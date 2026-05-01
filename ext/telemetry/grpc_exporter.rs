// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::sync::Once;
use std::time::Duration;

use async_trait::async_trait;
use deno_core::futures::future::BoxFuture;
use flate2::Compression;
use flate2::write::GzEncoder;
use hyper::header;
use opentelemetry_http::HttpClient;
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

const USER_AGENT: &str =
  concat!("OTel-OTLP-Exporter-Deno/", env!("CARGO_PKG_VERSION"));

/// Resolve the gRPC endpoint, honoring `OTEL_EXPORTER_OTLP_INSECURE` for
/// bare `host:port` endpoints that lack a scheme.
fn grpc_endpoint() -> String {
  let raw = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
    .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"))
    .unwrap_or_else(|_| DEFAULT_GRPC_ENDPOINT.to_string());

  // If the endpoint already has a scheme, use it as-is.
  if raw.starts_with("http://") || raw.starts_with("https://") {
    return raw;
  }

  // Bare host:port — synthesize scheme from OTEL_EXPORTER_OTLP_INSECURE.
  let insecure = std::env::var("OTEL_EXPORTER_OTLP_INSECURE")
    .map(|v| v.eq_ignore_ascii_case("true"))
    .unwrap_or(false);
  let scheme = if insecure { "http" } else { "https" };
  format!("{}://{}", scheme, raw)
}

#[derive(Clone, Copy, PartialEq)]
enum GrpcCompression {
  None,
  Gzip,
}

fn grpc_compression() -> GrpcCompression {
  match std::env::var("OTEL_EXPORTER_OTLP_COMPRESSION") {
    Ok(val) if val.eq_ignore_ascii_case("gzip") => GrpcCompression::Gzip,
    Ok(val) if val.is_empty() || val.eq_ignore_ascii_case("none") => {
      GrpcCompression::None
    }
    Ok(val) => {
      static WARN_ONCE: Once = Once::new();
      WARN_ONCE.call_once(|| {
        log::warn!(
          "unsupported OTEL_EXPORTER_OTLP_COMPRESSION value {:?}, falling back to no compression",
          val,
        );
      });
      GrpcCompression::None
    }
    Err(_) => GrpcCompression::None,
  }
}

fn grpc_headers()
-> impl Iterator<Item = (header::HeaderName, header::HeaderValue)> {
  ["OTEL_EXPORTER_OTLP_HEADERS"]
    .into_iter()
    .filter_map(|var| std::env::var(var).ok())
    .flat_map(|val| {
      val
        .split(',')
        .filter_map(|pair| {
          let (k, v) = pair.split_once('=')?;
          let name = match header::HeaderName::from_bytes(k.trim().as_bytes()) {
            Ok(n) => n,
            Err(_) => {
              log::warn!("invalid OTEL header name: {:?}", k.trim());
              return None;
            }
          };
          let value = match header::HeaderValue::from_str(v.trim()) {
            Ok(v) => v,
            Err(_) => {
              log::warn!("invalid OTEL header value for {:?}", k.trim());
              return None;
            }
          };
          Some((name, value))
        })
        .collect::<Vec<_>>()
    })
}

/// Encode a protobuf message into a gRPC frame: 1 byte compression flag
/// + 4 byte big-endian message length + message bytes (optionally gzip'd).
fn grpc_frame(msg: &impl Message) -> Vec<u8> {
  let compression = grpc_compression();
  let raw_len = msg.encoded_len();
  let mut proto_buf = Vec::with_capacity(raw_len);
  msg.encode(&mut proto_buf).expect("protobuf encode failed");

  let (compressed_flag, payload) = if compression == GrpcCompression::Gzip {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&proto_buf).expect("gzip encode failed");
    (1u8, encoder.finish().expect("gzip finish failed"))
  } else {
    (0u8, proto_buf)
  };

  let len = payload.len();
  let mut buf = Vec::with_capacity(5 + len);
  buf.push(compressed_flag);
  buf.extend_from_slice(&(len as u32).to_be_bytes());
  buf.extend_from_slice(&payload);
  buf
}

/// Format a `Duration` as a gRPC timeout header value (milliseconds).
fn grpc_timeout_header(timeout: Duration) -> String {
  format!("{}m", timeout.as_millis())
}

/// Send a gRPC unary request and check the response status.
async fn grpc_send(
  client: &HyperClient,
  path: &str,
  body: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let endpoint = grpc_endpoint();
  let url = format!("{}{}", endpoint.trim_end_matches('/'), path);

  let mut builder = hyper::Request::builder()
    .method(hyper::Method::POST)
    .uri(&url)
    .header("content-type", "application/grpc")
    .header("te", "trailers")
    .header("user-agent", USER_AGENT)
    .header("grpc-timeout", grpc_timeout_header(client.timeout()));

  if grpc_compression() == GrpcCompression::Gzip {
    builder = builder.header("grpc-encoding", "gzip");
  }

  let mut request = builder.body(body)?;

  for (k, v) in grpc_headers() {
    request.headers_mut().insert(k, v);
  }

  let response = client.send(request).await?;
  let status = response.status();
  if !status.is_success() {
    return Err(format!("gRPC transport error: HTTP {}", status).into());
  }

  // Check grpc-status header (may be in initial headers for errors)
  if let Some(grpc_status) = response.headers().get("grpc-status") {
    let code: i32 = grpc_status.to_str()?.parse()?;
    if code != 0 {
      let message = response
        .headers()
        .get("grpc-message")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown error");
      return Err(format!("gRPC error {}: {}", code, message).into());
    }
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
      grpc_send(&client, TRACES_PATH, body)
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
    grpc_send(&self.client, LOGS_PATH, body)
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
    grpc_send(&self.client, METRICS_PATH, body)
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
