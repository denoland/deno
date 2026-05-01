// Copyright 2018-2026 the Deno authors. MIT license.

use async_trait::async_trait;
use deno_core::futures::future::BoxFuture;
use hyper::header;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::export::logs::LogBatch;
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use prost::Message;

use crate::hyper_client::HyperClient;

const DEFAULT_GRPC_ENDPOINT: &str = "http://localhost:4317";

const TRACES_PATH: &str =
  "/opentelemetry.proto.collector.trace.v1.TraceService/Export";
const METRICS_PATH: &str =
  "/opentelemetry.proto.collector.metrics.v1.MetricsService/Export";
const LOGS_PATH: &str =
  "/opentelemetry.proto.collector.logs.v1.LogsService/Export";

#[derive(Clone, Copy)]
enum Signal {
  Traces,
  Metrics,
  Logs,
}

impl Signal {
  fn endpoint_env_var(self) -> &'static str {
    match self {
      Signal::Traces => "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
      Signal::Metrics => "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
      Signal::Logs => "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
    }
  }

  fn headers_env_var(self) -> &'static str {
    match self {
      Signal::Traces => "OTEL_EXPORTER_OTLP_TRACES_HEADERS",
      Signal::Metrics => "OTEL_EXPORTER_OTLP_METRICS_HEADERS",
      Signal::Logs => "OTEL_EXPORTER_OTLP_LOGS_HEADERS",
    }
  }
}

/// Per-signal endpoint takes precedence over the generic one per the OTLP spec.
fn grpc_endpoint(signal: Signal) -> String {
  std::env::var(signal.endpoint_env_var())
    .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
    .unwrap_or_else(|_| DEFAULT_GRPC_ENDPOINT.to_string())
}

/// Parse OTLP headers. Per-signal headers are merged with (and override)
/// the generic headers, per the OTLP spec.
fn grpc_headers(
  signal: Signal,
) -> impl Iterator<Item = (header::HeaderName, header::HeaderValue)> {
  fn parse_headers(
    val: &str,
  ) -> Vec<(header::HeaderName, header::HeaderValue)> {
    val
      .split(',')
      .filter_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        Some((
          header::HeaderName::from_bytes(k.trim().as_bytes()).ok()?,
          header::HeaderValue::from_str(v.trim()).ok()?,
        ))
      })
      .collect()
  }

  let mut headers = Vec::new();
  if let Ok(val) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
    headers.extend(parse_headers(&val));
  }
  // Per-signal headers override generic ones
  if let Ok(val) = std::env::var(signal.headers_env_var()) {
    headers.extend(parse_headers(&val));
  }
  headers.into_iter()
}

/// Encode a protobuf message into a gRPC frame: 1 byte compression flag
/// (always 0) + 4 byte big-endian message length + message bytes.
fn grpc_frame(msg: &impl Message) -> Vec<u8> {
  let len = msg.encoded_len();
  let mut buf = Vec::with_capacity(5 + len);
  buf.push(0u8); // no compression
  buf.extend_from_slice(&(len as u32).to_be_bytes());
  msg.encode(&mut buf).unwrap();
  buf
}

/// Check grpc-status in a header map, returning an error if non-zero.
fn check_grpc_status(
  headers: &hyper::HeaderMap,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
  if let Some(grpc_status) = headers.get("grpc-status") {
    let code: i32 = grpc_status.to_str()?.parse()?;
    if code != 0 {
      let message = headers
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown error");
      return Err(format!("gRPC error {}: {}", code, message).into());
    }
    return Ok(true); // found and was 0
  }
  Ok(false) // not found
}

/// Send a gRPC unary request and check the response status.
///
/// Checks grpc-status in both initial response headers (Trailers-Only
/// form for early errors) and HTTP/2 trailers (normal success/error
/// form), per the gRPC spec.
async fn grpc_send(
  client: &HyperClient,
  signal: Signal,
  path: &str,
  body: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let endpoint = grpc_endpoint(signal);
  let url = format!("{}{}", endpoint.trim_end_matches('/'), path);

  let mut request = hyper::Request::builder()
    .method(hyper::Method::POST)
    .uri(&url)
    .header("content-type", "application/grpc")
    .header("te", "trailers")
    .body(body)?;

  for (k, v) in grpc_headers(signal) {
    request.headers_mut().insert(k, v);
  }

  let (parts, trailers) = client.grpc_request(request).await?;
  if !parts.status.is_success() {
    return Err(format!("gRPC transport error: HTTP {}", parts.status).into());
  }

  // Check grpc-status in initial headers (Trailers-Only form)
  if let Err(e) = check_grpc_status(&parts.headers) {
    log::error!("OTLP gRPC export failed: {e}");
    return Err(e);
  }

  // Check grpc-status in HTTP/2 trailers (normal form)
  if let Some(ref trailers) = trailers
    && let Err(e) = check_grpc_status(trailers)
  {
    log::error!("OTLP gRPC export failed: {e}");
    return Err(e);
  }

  Ok(())
}

// ---- Span Exporter ----

#[derive(Debug)]
pub struct GrpcSpanExporter {
  client: HyperClient,
  resource: Resource,
}

impl GrpcSpanExporter {
  pub fn new(client: HyperClient) -> Self {
    Self {
      client,
      resource: Resource::default(),
    }
  }
}

impl opentelemetry_sdk::export::trace::SpanExporter for GrpcSpanExporter {
  fn export(
    &mut self,
    batch: Vec<SpanData>,
  ) -> BoxFuture<'static, opentelemetry_sdk::export::trace::ExportResult> {
    let resource: ResourceAttributesWithSchema = (&self.resource).into();
    let client = self.client.clone();
    Box::pin(async move {
      let resource_spans =
        opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope(
          batch, &resource,
        );
      let request = ExportTraceServiceRequest { resource_spans };
      let body = grpc_frame(&request);
      grpc_send(&client, Signal::Traces, TRACES_PATH, body)
        .await
        .map_err(opentelemetry::trace::TraceError::Other)
    })
  }

  fn shutdown(&mut self) {}

  fn set_resource(&mut self, resource: &Resource) {
    self.resource = resource.clone();
  }
}

// ---- Log Exporter ----

#[derive(Debug)]
pub struct GrpcLogExporter {
  client: HyperClient,
  resource: Resource,
}

impl GrpcLogExporter {
  pub fn new(client: HyperClient) -> Self {
    Self {
      client,
      resource: Resource::default(),
    }
  }
}

#[async_trait]
impl opentelemetry_sdk::export::logs::LogExporter for GrpcLogExporter {
  async fn export(
    &mut self,
    batch: LogBatch<'_>,
  ) -> opentelemetry_sdk::logs::LogResult<()> {
    let resource: ResourceAttributesWithSchema = (&self.resource).into();
    let resource_logs =
      opentelemetry_proto::transform::logs::tonic::group_logs_by_resource_and_scope(
        batch, &resource,
      );
    let request = ExportLogsServiceRequest { resource_logs };
    let body = grpc_frame(&request);
    grpc_send(&self.client, Signal::Logs, LOGS_PATH, body)
      .await
      .map_err(opentelemetry_sdk::logs::LogError::Other)
  }

  fn shutdown(&mut self) {}

  fn set_resource(&mut self, resource: &Resource) {
    self.resource = resource.clone();
  }
}

// ---- Metric Exporter ----

#[derive(Debug)]
pub struct GrpcMetricExporter {
  client: HyperClient,
  temporality: Temporality,
}

impl GrpcMetricExporter {
  pub fn new(client: HyperClient, temporality: Temporality) -> Self {
    Self {
      client,
      temporality,
    }
  }
}

#[async_trait]
impl opentelemetry_sdk::metrics::exporter::PushMetricExporter
  for GrpcMetricExporter
{
  async fn export(
    &self,
    metrics: &mut ResourceMetrics,
  ) -> opentelemetry_sdk::metrics::MetricResult<()> {
    let request = ExportMetricsServiceRequest::from(&*metrics);
    let body = grpc_frame(&request);
    grpc_send(&self.client, Signal::Metrics, METRICS_PATH, body)
      .await
      .map_err(|e| {
        opentelemetry_sdk::metrics::MetricError::Other(e.to_string())
      })
  }

  async fn force_flush(&self) -> opentelemetry_sdk::metrics::MetricResult<()> {
    Ok(())
  }

  fn shutdown(&self) -> opentelemetry_sdk::metrics::MetricResult<()> {
    Ok(())
  }

  fn temporality(&self) -> Temporality {
    self.temporality
  }
}
