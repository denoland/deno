// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow;
use deno_core::anyhow::anyhow;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::stream;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use opentelemetry::logs::AnyValue;
use opentelemetry::logs::LogRecord as LogRecordTrait;
use opentelemetry::logs::Severity;
use opentelemetry::otel_error;
use opentelemetry::trace::SpanContext;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status as SpanStatus;
use opentelemetry::trace::TraceFlags;
use opentelemetry::trace::TraceId;
use opentelemetry::Key;
use opentelemetry::KeyValue;
use opentelemetry::StringValue;
use opentelemetry::Value;
use opentelemetry_otlp::HttpExporterBuilder;
use opentelemetry_otlp::MetricExporter;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::logs::BatchLogProcessor;
use opentelemetry_sdk::logs::LogProcessor;
use opentelemetry_sdk::logs::LogRecord;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SpanProcessor;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::PROCESS_RUNTIME_NAME;
use opentelemetry_semantic_conventions::resource::PROCESS_RUNTIME_VERSION;
use opentelemetry_semantic_conventions::resource::TELEMETRY_SDK_LANGUAGE;
use opentelemetry_semantic_conventions::resource::TELEMETRY_SDK_NAME;
use opentelemetry_semantic_conventions::resource::TELEMETRY_SDK_VERSION;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::env;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

deno_core::extension!(
  deno_telemetry,
  ops = [
    op_otel_log,
    op_otel_instrumentation_scope_create_and_enter,
    op_otel_instrumentation_scope_enter,
    op_otel_instrumentation_scope_enter_builtin,
    op_otel_span_start,
    op_otel_span_continue,
    op_otel_span_attribute,
    op_otel_span_attribute2,
    op_otel_span_attribute3,
    op_otel_span_set_dropped,
    op_otel_span_flush,
    op_otel_metrics_resource_attribute,
    op_otel_metrics_resource_attribute2,
    op_otel_metrics_resource_attribute3,
    op_otel_metrics_scope,
    op_otel_metrics_sum,
    op_otel_metrics_gauge,
    op_otel_metrics_sum_or_gauge_data_point,
    op_otel_metrics_histogram,
    op_otel_metrics_histogram_data_point,
    op_otel_metrics_histogram_data_point_entry_final,
    op_otel_metrics_histogram_data_point_entry1,
    op_otel_metrics_histogram_data_point_entry2,
    op_otel_metrics_histogram_data_point_entry3,
    op_otel_metrics_data_point_attribute,
    op_otel_metrics_data_point_attribute2,
    op_otel_metrics_data_point_attribute3,
    op_otel_metrics_submit,
  ],
  esm = ["telemetry.ts", "util.ts"],
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelRuntimeConfig {
  pub runtime_name: Cow<'static, str>,
  pub runtime_version: Cow<'static, str>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct OtelConfig {
  pub tracing_enabled: bool,
  pub console: OtelConsoleConfig,
  pub deterministic: bool,
}

impl OtelConfig {
  pub fn as_v8(&self) -> Box<[u8]> {
    Box::new([
      self.tracing_enabled as u8,
      self.console as u8,
      self.deterministic as u8,
    ])
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum OtelConsoleConfig {
  Ignore = 0,
  Capture = 1,
  Replace = 2,
}

impl Default for OtelConsoleConfig {
  fn default() -> Self {
    Self::Ignore
  }
}

static OTEL_SHARED_RUNTIME_SPAWN_TASK_TX: Lazy<
  UnboundedSender<BoxFuture<'static, ()>>,
> = Lazy::new(otel_create_shared_runtime);

fn otel_create_shared_runtime() -> UnboundedSender<BoxFuture<'static, ()>> {
  let (spawn_task_tx, mut spawn_task_rx) =
    mpsc::unbounded::<BoxFuture<'static, ()>>();

  thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
      .enable_io()
      .enable_time()
      // This limits the number of threads for blocking operations (like for
      // synchronous fs ops) or CPU bound tasks like when we run dprint in
      // parallel for deno fmt.
      // The default value is 512, which is an unhelpfully large thread pool. We
      // don't ever want to have more than a couple dozen threads.
      .max_blocking_threads(if cfg!(windows) {
        // on windows, tokio uses blocking tasks for child process IO, make sure
        // we have enough available threads for other tasks to run
        4 * std::thread::available_parallelism()
          .map(|n| n.get())
          .unwrap_or(8)
      } else {
        32
      })
      .build()
      .unwrap();

    rt.block_on(async move {
      while let Some(task) = spawn_task_rx.next().await {
        tokio::spawn(task);
      }
    });
  });

  spawn_task_tx
}

#[derive(Clone, Copy)]
struct OtelSharedRuntime;

impl hyper::rt::Executor<BoxFuture<'static, ()>> for OtelSharedRuntime {
  fn execute(&self, fut: BoxFuture<'static, ()>) {
    (*OTEL_SHARED_RUNTIME_SPAWN_TASK_TX)
      .unbounded_send(fut)
      .expect("failed to send task to shared OpenTelemetry runtime");
  }
}

impl opentelemetry_sdk::runtime::Runtime for OtelSharedRuntime {
  type Interval = Pin<Box<dyn Stream<Item = ()> + Send + 'static>>;
  type Delay = Pin<Box<tokio::time::Sleep>>;

  fn interval(&self, period: Duration) -> Self::Interval {
    stream::repeat(())
      .then(move |_| tokio::time::sleep(period))
      .boxed()
  }

  fn spawn(&self, future: BoxFuture<'static, ()>) {
    (*OTEL_SHARED_RUNTIME_SPAWN_TASK_TX)
      .unbounded_send(future)
      .expect("failed to send task to shared OpenTelemetry runtime");
  }

  fn delay(&self, duration: Duration) -> Self::Delay {
    Box::pin(tokio::time::sleep(duration))
  }
}

impl opentelemetry_sdk::runtime::RuntimeChannel for OtelSharedRuntime {
  type Receiver<T: Debug + Send> = BatchMessageChannelReceiver<T>;
  type Sender<T: Debug + Send> = BatchMessageChannelSender<T>;

  fn batch_message_channel<T: Debug + Send>(
    &self,
    capacity: usize,
  ) -> (Self::Sender<T>, Self::Receiver<T>) {
    let (batch_tx, batch_rx) = tokio::sync::mpsc::channel::<T>(capacity);
    (batch_tx.into(), batch_rx.into())
  }
}

#[derive(Debug)]
pub struct BatchMessageChannelSender<T: Send> {
  sender: tokio::sync::mpsc::Sender<T>,
}

impl<T: Send> From<tokio::sync::mpsc::Sender<T>>
  for BatchMessageChannelSender<T>
{
  fn from(sender: tokio::sync::mpsc::Sender<T>) -> Self {
    Self { sender }
  }
}

impl<T: Send> opentelemetry_sdk::runtime::TrySend
  for BatchMessageChannelSender<T>
{
  type Message = T;

  fn try_send(
    &self,
    item: Self::Message,
  ) -> Result<(), opentelemetry_sdk::runtime::TrySendError> {
    self.sender.try_send(item).map_err(|err| match err {
      tokio::sync::mpsc::error::TrySendError::Full(_) => {
        opentelemetry_sdk::runtime::TrySendError::ChannelFull
      }
      tokio::sync::mpsc::error::TrySendError::Closed(_) => {
        opentelemetry_sdk::runtime::TrySendError::ChannelClosed
      }
    })
  }
}

pub struct BatchMessageChannelReceiver<T> {
  receiver: tokio::sync::mpsc::Receiver<T>,
}

impl<T> From<tokio::sync::mpsc::Receiver<T>>
  for BatchMessageChannelReceiver<T>
{
  fn from(receiver: tokio::sync::mpsc::Receiver<T>) -> Self {
    Self { receiver }
  }
}

impl<T> Stream for BatchMessageChannelReceiver<T> {
  type Item = T;

  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    self.receiver.poll_recv(cx)
  }
}

mod hyper_client {
  use http_body_util::BodyExt;
  use http_body_util::Full;
  use hyper::body::Body as HttpBody;
  use hyper::body::Frame;
  use hyper_util::client::legacy::connect::HttpConnector;
  use hyper_util::client::legacy::Client;
  use opentelemetry_http::Bytes;
  use opentelemetry_http::HttpError;
  use opentelemetry_http::Request;
  use opentelemetry_http::Response;
  use opentelemetry_http::ResponseExt;
  use std::fmt::Debug;
  use std::pin::Pin;
  use std::task::Poll;
  use std::task::{self};

  use super::OtelSharedRuntime;

  // same as opentelemetry_http::HyperClient except it uses OtelSharedRuntime
  #[derive(Debug, Clone)]
  pub struct HyperClient {
    inner: Client<HttpConnector, Body>,
  }

  impl HyperClient {
    pub fn new() -> Self {
      Self {
        inner: Client::builder(OtelSharedRuntime).build(HttpConnector::new()),
      }
    }
  }

  #[async_trait::async_trait]
  impl opentelemetry_http::HttpClient for HyperClient {
    async fn send(
      &self,
      request: Request<Vec<u8>>,
    ) -> Result<Response<Bytes>, HttpError> {
      let (parts, body) = request.into_parts();
      let request = Request::from_parts(parts, Body(Full::from(body)));
      let mut response = self.inner.request(request).await?;
      let headers = std::mem::take(response.headers_mut());

      let mut http_response = Response::builder()
        .status(response.status())
        .body(response.into_body().collect().await?.to_bytes())?;
      *http_response.headers_mut() = headers;

      Ok(http_response.error_for_status()?)
    }
  }

  #[pin_project::pin_project]
  pub struct Body(#[pin] Full<Bytes>);

  impl HttpBody for Body {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

    #[inline]
    fn poll_frame(
      self: Pin<&mut Self>,
      cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
      self.project().0.poll_frame(cx).map_err(Into::into)
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
      self.0.is_end_stream()
    }

    #[inline]
    fn size_hint(&self) -> hyper::body::SizeHint {
      self.0.size_hint()
    }
  }
}

enum MetricProcessorMessage {
  ResourceMetrics(ResourceMetrics),
  Flush(tokio::sync::oneshot::Sender<()>),
}

struct MetricProcessor {
  tx: tokio::sync::mpsc::Sender<MetricProcessorMessage>,
}

impl MetricProcessor {
  fn new(exporter: MetricExporter) -> Self {
    let (tx, mut rx) = tokio::sync::mpsc::channel(2048);
    let future = async move {
      while let Some(message) = rx.recv().await {
        match message {
          MetricProcessorMessage::ResourceMetrics(mut rm) => {
            if let Err(err) = exporter.export(&mut rm).await {
              otel_error!(
                name: "MetricProcessor.Export.Error",
                error = format!("{}", err)
              );
            }
          }
          MetricProcessorMessage::Flush(tx) => {
            if let Err(()) = tx.send(()) {
              otel_error!(
                name: "MetricProcessor.Flush.SendResultError",
                error = "()",
              );
            }
          }
        }
      }
    };

    (*OTEL_SHARED_RUNTIME_SPAWN_TASK_TX)
      .unbounded_send(Box::pin(future))
      .expect("failed to send task to shared OpenTelemetry runtime");

    Self { tx }
  }

  fn submit(&self, rm: ResourceMetrics) {
    let _ = self
      .tx
      .try_send(MetricProcessorMessage::ResourceMetrics(rm));
  }

  fn force_flush(&self) -> Result<(), anyhow::Error> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    self.tx.try_send(MetricProcessorMessage::Flush(tx))?;
    deno_core::futures::executor::block_on(rx)?;
    Ok(())
  }
}

struct Processors {
  spans: BatchSpanProcessor<OtelSharedRuntime>,
  logs: BatchLogProcessor<OtelSharedRuntime>,
  metrics: MetricProcessor,
}

static OTEL_PROCESSORS: OnceCell<Processors> = OnceCell::new();

static BUILT_IN_INSTRUMENTATION_SCOPE: OnceCell<
  opentelemetry::InstrumentationScope,
> = OnceCell::new();

pub fn init(config: OtelRuntimeConfig) -> anyhow::Result<()> {
  // Parse the `OTEL_EXPORTER_OTLP_PROTOCOL` variable. The opentelemetry_*
  // crates don't do this automatically.
  // TODO(piscisaureus): enable GRPC support.
  let protocol = match env::var("OTEL_EXPORTER_OTLP_PROTOCOL").as_deref() {
    Ok("http/protobuf") => Protocol::HttpBinary,
    Ok("http/json") => Protocol::HttpJson,
    Ok("") | Err(env::VarError::NotPresent) => Protocol::HttpBinary,
    Ok(protocol) => {
      return Err(anyhow!(
        "Env var OTEL_EXPORTER_OTLP_PROTOCOL specifies an unsupported protocol: {}",
        protocol
      ));
    }
    Err(err) => {
      return Err(anyhow!(
        "Failed to read env var OTEL_EXPORTER_OTLP_PROTOCOL: {}",
        err
      ));
    }
  };

  // Define the resource attributes that will be attached to all log records.
  // These attributes are sourced as follows (in order of precedence):
  //   * The `service.name` attribute from the `OTEL_SERVICE_NAME` env var.
  //   * Additional attributes from the `OTEL_RESOURCE_ATTRIBUTES` env var.
  //   * Default attribute values defined here.
  // TODO(piscisaureus): add more default attributes (e.g. script path).
  let mut resource = Resource::default();

  // Add the runtime name and version to the resource attributes. Also override
  // the `telemetry.sdk` attributes to include the Deno runtime.
  resource = resource.merge(&Resource::new(vec![
    KeyValue::new(PROCESS_RUNTIME_NAME, config.runtime_name),
    KeyValue::new(PROCESS_RUNTIME_VERSION, config.runtime_version.clone()),
    KeyValue::new(
      TELEMETRY_SDK_LANGUAGE,
      format!(
        "deno-{}",
        resource.get(Key::new(TELEMETRY_SDK_LANGUAGE)).unwrap()
      ),
    ),
    KeyValue::new(
      TELEMETRY_SDK_NAME,
      format!(
        "deno-{}",
        resource.get(Key::new(TELEMETRY_SDK_NAME)).unwrap()
      ),
    ),
    KeyValue::new(
      TELEMETRY_SDK_VERSION,
      format!(
        "{}-{}",
        config.runtime_version,
        resource.get(Key::new(TELEMETRY_SDK_VERSION)).unwrap()
      ),
    ),
  ]));

  // The OTLP endpoint is automatically picked up from the
  // `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable. Additional headers can
  // be specified using `OTEL_EXPORTER_OTLP_HEADERS`.

  let client = hyper_client::HyperClient::new();

  let span_exporter = HttpExporterBuilder::default()
    .with_http_client(client.clone())
    .with_protocol(protocol)
    .build_span_exporter()?;
  let mut span_processor =
    BatchSpanProcessor::builder(span_exporter, OtelSharedRuntime).build();
  span_processor.set_resource(&resource);

  let metric_exporter = HttpExporterBuilder::default()
    .with_http_client(client.clone())
    .with_protocol(protocol)
    .build_metrics_exporter(Temporality::Cumulative)?;
  let metric_processor = MetricProcessor::new(metric_exporter);

  let log_exporter = HttpExporterBuilder::default()
    .with_http_client(client)
    .with_protocol(protocol)
    .build_log_exporter()?;
  let log_processor =
    BatchLogProcessor::builder(log_exporter, OtelSharedRuntime).build();
  log_processor.set_resource(&resource);

  OTEL_PROCESSORS
    .set(Processors {
      spans: span_processor,
      logs: log_processor,
      metrics: metric_processor,
    })
    .map_err(|_| anyhow!("failed to init otel"))?;

  let builtin_instrumentation_scope =
    opentelemetry::InstrumentationScope::builder("deno")
      .with_version(config.runtime_version.clone())
      .build();
  BUILT_IN_INSTRUMENTATION_SCOPE
    .set(builtin_instrumentation_scope)
    .map_err(|_| anyhow!("failed to init otel"))?;

  Ok(())
}

/// This function is called by the runtime whenever it is about to call
/// `process::exit()`, to ensure that all OpenTelemetry logs are properly
/// flushed before the process terminates.
pub fn flush() {
  if let Some(Processors {
    spans,
    logs,
    metrics,
  }) = OTEL_PROCESSORS.get()
  {
    let _ = spans.force_flush();
    let _ = logs.force_flush();
    let _ = metrics.force_flush();
  }
}

pub fn handle_log(record: &log::Record) {
  use log::Level;

  let Some(Processors { logs, .. }) = OTEL_PROCESSORS.get() else {
    return;
  };

  let mut log_record = LogRecord::default();

  log_record.set_observed_timestamp(SystemTime::now());
  log_record.set_severity_number(match record.level() {
    Level::Error => Severity::Error,
    Level::Warn => Severity::Warn,
    Level::Info => Severity::Info,
    Level::Debug => Severity::Debug,
    Level::Trace => Severity::Trace,
  });
  log_record.set_severity_text(record.level().as_str());
  log_record.set_body(record.args().to_string().into());
  log_record.set_target(record.metadata().target().to_string());

  struct Visitor<'s>(&'s mut LogRecord);

  impl<'s, 'kvs> log::kv::VisitSource<'kvs> for Visitor<'s> {
    fn visit_pair(
      &mut self,
      key: log::kv::Key<'kvs>,
      value: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
      #[allow(clippy::manual_map)]
      let value = if let Some(v) = value.to_bool() {
        Some(AnyValue::Boolean(v))
      } else if let Some(v) = value.to_borrowed_str() {
        Some(AnyValue::String(v.to_owned().into()))
      } else if let Some(v) = value.to_f64() {
        Some(AnyValue::Double(v))
      } else if let Some(v) = value.to_i64() {
        Some(AnyValue::Int(v))
      } else {
        None
      };

      if let Some(value) = value {
        let key = Key::from(key.as_str().to_owned());
        self.0.add_attribute(key, value);
      }

      Ok(())
    }
  }

  let _ = record.key_values().visit(&mut Visitor(&mut log_record));

  logs.emit(
    &mut log_record,
    BUILT_IN_INSTRUMENTATION_SCOPE.get().unwrap(),
  );
}

fn parse_trace_id(
  scope: &mut v8::HandleScope<'_>,
  trace_id: v8::Local<'_, v8::Value>,
) -> TraceId {
  if let Ok(string) = trace_id.try_cast() {
    let value_view = v8::ValueView::new(scope, string);
    match value_view.data() {
      v8::ValueViewData::OneByte(bytes) => {
        TraceId::from_hex(&String::from_utf8_lossy(bytes))
          .unwrap_or(TraceId::INVALID)
      }

      _ => TraceId::INVALID,
    }
  } else if let Ok(uint8array) = trace_id.try_cast::<v8::Uint8Array>() {
    let data = uint8array.data();
    let byte_length = uint8array.byte_length();
    if byte_length != 16 {
      return TraceId::INVALID;
    }
    // SAFETY: We have ensured that the byte length is 16, so it is safe to
    // cast the data to an array of 16 bytes.
    let bytes = unsafe { &*(data as *const u8 as *const [u8; 16]) };
    TraceId::from_bytes(*bytes)
  } else {
    TraceId::INVALID
  }
}

fn parse_span_id(
  scope: &mut v8::HandleScope<'_>,
  span_id: v8::Local<'_, v8::Value>,
) -> SpanId {
  if let Ok(string) = span_id.try_cast() {
    let value_view = v8::ValueView::new(scope, string);
    match value_view.data() {
      v8::ValueViewData::OneByte(bytes) => {
        SpanId::from_hex(&String::from_utf8_lossy(bytes))
          .unwrap_or(SpanId::INVALID)
      }
      _ => SpanId::INVALID,
    }
  } else if let Ok(uint8array) = span_id.try_cast::<v8::Uint8Array>() {
    let data = uint8array.data();
    let byte_length = uint8array.byte_length();
    if byte_length != 8 {
      return SpanId::INVALID;
    }
    // SAFETY: We have ensured that the byte length is 8, so it is safe to
    // cast the data to an array of 8 bytes.
    let bytes = unsafe { &*(data as *const u8 as *const [u8; 8]) };
    SpanId::from_bytes(*bytes)
  } else {
    SpanId::INVALID
  }
}

macro_rules! attr {
  ($scope:ident, $attributes:expr $(=> $dropped_attributes_count:expr)?, $name:expr, $value:expr) => {
    let name = if let Ok(name) = $name.try_cast() {
      let view = v8::ValueView::new($scope, name);
      match view.data() {
        v8::ValueViewData::OneByte(bytes) => {
          Some(String::from_utf8_lossy(bytes).into_owned())
        }
        v8::ValueViewData::TwoByte(bytes) => {
          Some(String::from_utf16_lossy(bytes))
        }
      }
    } else {
      None
    };
    let value = if let Ok(string) = $value.try_cast::<v8::String>() {
      Some(Value::String(StringValue::from({
        let x = v8::ValueView::new($scope, string);
        match x.data() {
          v8::ValueViewData::OneByte(bytes) => {
            String::from_utf8_lossy(bytes).into_owned()
          }
          v8::ValueViewData::TwoByte(bytes) => String::from_utf16_lossy(bytes),
        }
      })))
    } else if let Ok(number) = $value.try_cast::<v8::Number>() {
      Some(Value::F64(number.value()))
    } else if let Ok(boolean) = $value.try_cast::<v8::Boolean>() {
      Some(Value::Bool(boolean.is_true()))
    } else if let Ok(bigint) = $value.try_cast::<v8::BigInt>() {
      let (i64_value, _lossless) = bigint.i64_value();
      Some(Value::I64(i64_value))
    } else {
      None
    };
    if let (Some(name), Some(value)) = (name, value) {
      $attributes.push(KeyValue::new(name, value));
    }
    $(
      else {
        $dropped_attributes_count += 1;
      }
    )?
  };
}

#[derive(Debug, Clone)]
struct InstrumentationScope(opentelemetry::InstrumentationScope);

impl deno_core::GarbageCollected for InstrumentationScope {}

#[op2]
#[cppgc]
fn op_otel_instrumentation_scope_create_and_enter(
  state: &mut OpState,
  #[string] name: String,
  #[string] version: Option<String>,
  #[string] schema_url: Option<String>,
) -> InstrumentationScope {
  let mut builder = opentelemetry::InstrumentationScope::builder(name);
  if let Some(version) = version {
    builder = builder.with_version(version);
  }
  if let Some(schema_url) = schema_url {
    builder = builder.with_schema_url(schema_url);
  }
  let scope = InstrumentationScope(builder.build());
  state.put(scope.clone());
  scope
}

#[op2(fast)]
fn op_otel_instrumentation_scope_enter(
  state: &mut OpState,
  #[cppgc] scope: &InstrumentationScope,
) {
  state.put(scope.clone());
}

#[op2(fast)]
fn op_otel_instrumentation_scope_enter_builtin(state: &mut OpState) {
  if let Some(scope) = BUILT_IN_INSTRUMENTATION_SCOPE.get() {
    state.put(InstrumentationScope(scope.clone()));
  }
}

#[op2(fast)]
fn op_otel_log(
  scope: &mut v8::HandleScope<'_>,
  #[string] message: String,
  #[smi] level: i32,
  trace_id: v8::Local<'_, v8::Value>,
  span_id: v8::Local<'_, v8::Value>,
  #[smi] trace_flags: u8,
) {
  let Some(Processors { logs, .. }) = OTEL_PROCESSORS.get() else {
    return;
  };
  let Some(instrumentation_scope) = BUILT_IN_INSTRUMENTATION_SCOPE.get() else {
    return;
  };

  // Convert the integer log level that ext/console uses to the corresponding
  // OpenTelemetry log severity.
  let severity = match level {
    ..=0 => Severity::Debug,
    1 => Severity::Info,
    2 => Severity::Warn,
    3.. => Severity::Error,
  };

  let trace_id = parse_trace_id(scope, trace_id);
  let span_id = parse_span_id(scope, span_id);

  let mut log_record = LogRecord::default();

  log_record.set_observed_timestamp(SystemTime::now());
  log_record.set_body(message.into());
  log_record.set_severity_number(severity);
  log_record.set_severity_text(severity.name());
  if trace_id != TraceId::INVALID && span_id != SpanId::INVALID {
    log_record.set_trace_context(
      trace_id,
      span_id,
      Some(TraceFlags::new(trace_flags)),
    );
  }

  logs.emit(&mut log_record, instrumentation_scope);
}

fn owned_string<'s>(
  scope: &mut v8::HandleScope<'s>,
  string: v8::Local<'s, v8::String>,
) -> String {
  let x = v8::ValueView::new(scope, string);
  match x.data() {
    v8::ValueViewData::OneByte(bytes) => {
      String::from_utf8_lossy(bytes).into_owned()
    }
    v8::ValueViewData::TwoByte(bytes) => String::from_utf16_lossy(bytes),
  }
}

struct TemporarySpan(SpanData);

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_otel_span_start<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  trace_id: v8::Local<'s, v8::Value>,
  span_id: v8::Local<'s, v8::Value>,
  parent_span_id: v8::Local<'s, v8::Value>,
  #[smi] span_kind: u8,
  name: v8::Local<'s, v8::Value>,
  start_time: f64,
  end_time: f64,
) -> Result<(), anyhow::Error> {
  if let Some(temporary_span) = state.try_take::<TemporarySpan>() {
    let Some(Processors { spans, .. }) = OTEL_PROCESSORS.get() else {
      return Ok(());
    };
    spans.on_end(temporary_span.0);
  };

  let Some(InstrumentationScope(instrumentation_scope)) =
    state.try_borrow::<InstrumentationScope>()
  else {
    return Err(anyhow!("instrumentation scope not available"));
  };

  let trace_id = parse_trace_id(scope, trace_id);
  if trace_id == TraceId::INVALID {
    return Err(anyhow!("invalid trace_id"));
  }

  let span_id = parse_span_id(scope, span_id);
  if span_id == SpanId::INVALID {
    return Err(anyhow!("invalid span_id"));
  }

  let parent_span_id = parse_span_id(scope, parent_span_id);

  let name = owned_string(scope, name.try_cast()?);

  let temporary_span = TemporarySpan(SpanData {
    span_context: SpanContext::new(
      trace_id,
      span_id,
      TraceFlags::SAMPLED,
      false,
      Default::default(),
    ),
    parent_span_id,
    span_kind: match span_kind {
      0 => SpanKind::Internal,
      1 => SpanKind::Server,
      2 => SpanKind::Client,
      3 => SpanKind::Producer,
      4 => SpanKind::Consumer,
      _ => return Err(anyhow!("invalid span kind")),
    },
    name: Cow::Owned(name),
    start_time: SystemTime::UNIX_EPOCH
      .checked_add(std::time::Duration::from_secs_f64(start_time))
      .ok_or_else(|| anyhow!("invalid start time"))?,
    end_time: SystemTime::UNIX_EPOCH
      .checked_add(std::time::Duration::from_secs_f64(end_time))
      .ok_or_else(|| anyhow!("invalid start time"))?,
    attributes: Vec::new(),
    dropped_attributes_count: 0,
    events: Default::default(),
    links: Default::default(),
    status: SpanStatus::Unset,
    instrumentation_scope: instrumentation_scope.clone(),
  });
  state.put(temporary_span);

  Ok(())
}

#[op2(fast)]
fn op_otel_span_continue(
  state: &mut OpState,
  #[smi] status: u8,
  #[string] error_description: Cow<'_, str>,
) {
  if let Some(temporary_span) = state.try_borrow_mut::<TemporarySpan>() {
    temporary_span.0.status = match status {
      0 => SpanStatus::Unset,
      1 => SpanStatus::Ok,
      2 => SpanStatus::Error {
        description: Cow::Owned(error_description.into_owned()),
      },
      _ => return,
    };
  }
}

#[op2(fast)]
fn op_otel_span_attribute<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
) {
  if let Some(temporary_span) = state.try_borrow_mut::<TemporarySpan>() {
    temporary_span.0.attributes.reserve_exact(
      (capacity as usize) - temporary_span.0.attributes.capacity(),
    );
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key, value);
  }
}

#[op2(fast)]
fn op_otel_span_attribute2<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
) {
  if let Some(temporary_span) = state.try_borrow_mut::<TemporarySpan>() {
    temporary_span.0.attributes.reserve_exact(
      (capacity as usize) - temporary_span.0.attributes.capacity(),
    );
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key1, value1);
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key2, value2);
  }
}

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_otel_span_attribute3<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
  key3: v8::Local<'s, v8::Value>,
  value3: v8::Local<'s, v8::Value>,
) {
  if let Some(temporary_span) = state.try_borrow_mut::<TemporarySpan>() {
    temporary_span.0.attributes.reserve_exact(
      (capacity as usize) - temporary_span.0.attributes.capacity(),
    );
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key1, value1);
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key2, value2);
    attr!(scope, temporary_span.0.attributes => temporary_span.0.dropped_attributes_count, key3, value3);
  }
}

#[op2(fast)]
fn op_otel_span_set_dropped(
  state: &mut OpState,
  #[smi] dropped_attributes_count: u32,
  #[smi] dropped_links_count: u32,
  #[smi] dropped_events_count: u32,
) {
  if let Some(temporary_span) = state.try_borrow_mut::<TemporarySpan>() {
    temporary_span.0.dropped_attributes_count += dropped_attributes_count;
    temporary_span.0.links.dropped_count += dropped_links_count;
    temporary_span.0.events.dropped_count += dropped_events_count;
  }
}

#[op2(fast)]
fn op_otel_span_flush(state: &mut OpState) {
  let Some(temporary_span) = state.try_take::<TemporarySpan>() else {
    return;
  };

  let Some(Processors { spans, .. }) = OTEL_PROCESSORS.get() else {
    return;
  };

  spans.on_end(temporary_span.0);
}

// Holds data being built from JS before
// it is submitted to the rust processor.
struct TemporaryMetricsExport {
  resource_attributes: Vec<KeyValue>,
  scope_metrics: Vec<ScopeMetrics>,
  metric: Option<TemporaryMetric>,
}

struct TemporaryMetric {
  name: String,
  description: String,
  unit: String,
  data: TemporaryMetricData,
}

enum TemporaryMetricData {
  Sum(opentelemetry_sdk::metrics::data::Sum<f64>),
  Gauge(opentelemetry_sdk::metrics::data::Gauge<f64>),
  Histogram(opentelemetry_sdk::metrics::data::Histogram<f64>),
}

impl From<TemporaryMetric> for Metric {
  fn from(value: TemporaryMetric) -> Self {
    Metric {
      name: Cow::Owned(value.name),
      description: Cow::Owned(value.description),
      unit: Cow::Owned(value.unit),
      data: match value.data {
        TemporaryMetricData::Sum(sum) => Box::new(sum),
        TemporaryMetricData::Gauge(gauge) => Box::new(gauge),
        TemporaryMetricData::Histogram(histogram) => Box::new(histogram),
      },
    }
  }
}

#[op2(fast)]
fn op_otel_metrics_resource_attribute<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
) {
  let metrics_export = if let Some(metrics_export) =
    state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    metrics_export.resource_attributes.reserve_exact(
      (capacity as usize) - metrics_export.resource_attributes.capacity(),
    );
    metrics_export
  } else {
    state.put(TemporaryMetricsExport {
      resource_attributes: Vec::with_capacity(capacity as usize),
      scope_metrics: vec![],
      metric: None,
    });
    state.borrow_mut()
  };
  attr!(scope, metrics_export.resource_attributes, key, value);
}

#[op2(fast)]
fn op_otel_metrics_resource_attribute2<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
) {
  let metrics_export = if let Some(metrics_export) =
    state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    metrics_export.resource_attributes.reserve_exact(
      (capacity as usize) - metrics_export.resource_attributes.capacity(),
    );
    metrics_export
  } else {
    state.put(TemporaryMetricsExport {
      resource_attributes: Vec::with_capacity(capacity as usize),
      scope_metrics: vec![],
      metric: None,
    });
    state.borrow_mut()
  };
  attr!(scope, metrics_export.resource_attributes, key1, value1);
  attr!(scope, metrics_export.resource_attributes, key2, value2);
}

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_otel_metrics_resource_attribute3<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
  key3: v8::Local<'s, v8::Value>,
  value3: v8::Local<'s, v8::Value>,
) {
  let metrics_export = if let Some(metrics_export) =
    state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    metrics_export.resource_attributes.reserve_exact(
      (capacity as usize) - metrics_export.resource_attributes.capacity(),
    );
    metrics_export
  } else {
    state.put(TemporaryMetricsExport {
      resource_attributes: Vec::with_capacity(capacity as usize),
      scope_metrics: vec![],
      metric: None,
    });
    state.borrow_mut()
  };
  attr!(scope, metrics_export.resource_attributes, key1, value1);
  attr!(scope, metrics_export.resource_attributes, key2, value2);
  attr!(scope, metrics_export.resource_attributes, key3, value3);
}

#[op2(fast)]
fn op_otel_metrics_scope<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  name: v8::Local<'s, v8::Value>,
  schema_url: v8::Local<'s, v8::Value>,
  version: v8::Local<'s, v8::Value>,
) {
  let name = owned_string(scope, name.cast());

  let scope_builder = opentelemetry::InstrumentationScope::builder(name);
  let scope_builder = if schema_url.is_null_or_undefined() {
    scope_builder
  } else {
    scope_builder.with_schema_url(owned_string(scope, schema_url.cast()))
  };
  let scope_builder = if version.is_null_or_undefined() {
    scope_builder
  } else {
    scope_builder.with_version(owned_string(scope, version.cast()))
  };
  let scope = scope_builder.build();
  let scope_metric = ScopeMetrics {
    scope,
    metrics: vec![],
  };

  match state.try_borrow_mut::<TemporaryMetricsExport>() {
    Some(temp) => {
      if let Some(current_metric) = temp.metric.take() {
        let metric = Metric::from(current_metric);
        temp.scope_metrics.last_mut().unwrap().metrics.push(metric);
      }
      temp.scope_metrics.push(scope_metric);
    }
    None => {
      state.put(TemporaryMetricsExport {
        resource_attributes: vec![],
        scope_metrics: vec![scope_metric],
        metric: None,
      });
    }
  }
}

#[op2(fast)]
fn op_otel_metrics_sum<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  name: v8::Local<'s, v8::Value>,
  description: v8::Local<'s, v8::Value>,
  unit: v8::Local<'s, v8::Value>,
  #[smi] temporality: u8,
  is_monotonic: bool,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(current_metric) = temp.metric.take() {
    let metric = Metric::from(current_metric);
    temp.scope_metrics.last_mut().unwrap().metrics.push(metric);
  }

  let name = owned_string(scope, name.cast());
  let description = owned_string(scope, description.cast());
  let unit = owned_string(scope, unit.cast());
  let temporality = match temporality {
    0 => Temporality::Delta,
    1 => Temporality::Cumulative,
    _ => return,
  };
  let sum = opentelemetry_sdk::metrics::data::Sum {
    data_points: vec![],
    temporality,
    is_monotonic,
  };

  temp.metric = Some(TemporaryMetric {
    name,
    description,
    unit,
    data: TemporaryMetricData::Sum(sum),
  });
}

#[op2(fast)]
fn op_otel_metrics_gauge<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  name: v8::Local<'s, v8::Value>,
  description: v8::Local<'s, v8::Value>,
  unit: v8::Local<'s, v8::Value>,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(current_metric) = temp.metric.take() {
    let metric = Metric::from(current_metric);
    temp.scope_metrics.last_mut().unwrap().metrics.push(metric);
  }

  let name = owned_string(scope, name.cast());
  let description = owned_string(scope, description.cast());
  let unit = owned_string(scope, unit.cast());

  let gauge = opentelemetry_sdk::metrics::data::Gauge {
    data_points: vec![],
  };

  temp.metric = Some(TemporaryMetric {
    name,
    description,
    unit,
    data: TemporaryMetricData::Gauge(gauge),
  });
}

#[op2(fast)]
fn op_otel_metrics_sum_or_gauge_data_point(
  state: &mut OpState,
  value: f64,
  start_time: f64,
  time: f64,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  let start_time = SystemTime::UNIX_EPOCH
    .checked_add(std::time::Duration::from_secs_f64(start_time))
    .unwrap();
  let time = SystemTime::UNIX_EPOCH
    .checked_add(std::time::Duration::from_secs_f64(time))
    .unwrap();

  let data_point = opentelemetry_sdk::metrics::data::DataPoint {
    value,
    start_time: Some(start_time),
    time: Some(time),
    attributes: vec![],
    exemplars: vec![],
  };

  match &mut temp.metric {
    Some(TemporaryMetric {
      data: TemporaryMetricData::Sum(sum),
      ..
    }) => sum.data_points.push(data_point),
    Some(TemporaryMetric {
      data: TemporaryMetricData::Gauge(gauge),
      ..
    }) => gauge.data_points.push(data_point),
    _ => {}
  }
}

#[op2(fast)]
fn op_otel_metrics_histogram<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  name: v8::Local<'s, v8::Value>,
  description: v8::Local<'s, v8::Value>,
  unit: v8::Local<'s, v8::Value>,
  #[smi] temporality: u8,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(current_metric) = temp.metric.take() {
    let metric = Metric::from(current_metric);
    temp.scope_metrics.last_mut().unwrap().metrics.push(metric);
  }

  let name = owned_string(scope, name.cast());
  let description = owned_string(scope, description.cast());
  let unit = owned_string(scope, unit.cast());

  let temporality = match temporality {
    0 => Temporality::Delta,
    1 => Temporality::Cumulative,
    _ => return,
  };
  let histogram = opentelemetry_sdk::metrics::data::Histogram {
    data_points: vec![],
    temporality,
  };

  temp.metric = Some(TemporaryMetric {
    name,
    description,
    unit,
    data: TemporaryMetricData::Histogram(histogram),
  });
}

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_otel_metrics_histogram_data_point(
  state: &mut OpState,
  #[number] count: u64,
  min: f64,
  max: f64,
  sum: f64,
  start_time: f64,
  time: f64,
  #[smi] buckets: u32,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  let min = if min.is_nan() { None } else { Some(min) };
  let max = if max.is_nan() { None } else { Some(max) };

  let start_time = SystemTime::UNIX_EPOCH
    .checked_add(std::time::Duration::from_secs_f64(start_time))
    .unwrap();
  let time = SystemTime::UNIX_EPOCH
    .checked_add(std::time::Duration::from_secs_f64(time))
    .unwrap();

  let data_point = opentelemetry_sdk::metrics::data::HistogramDataPoint {
    bounds: Vec::with_capacity(buckets as usize),
    bucket_counts: Vec::with_capacity((buckets as usize) + 1),
    count,
    sum,
    min,
    max,
    start_time,
    time,
    attributes: vec![],
    exemplars: vec![],
  };

  if let Some(TemporaryMetric {
    data: TemporaryMetricData::Histogram(histogram),
    ..
  }) = &mut temp.metric
  {
    histogram.data_points.push(data_point);
  }
}

#[op2(fast)]
fn op_otel_metrics_histogram_data_point_entry_final(
  state: &mut OpState,
  #[number] count1: u64,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(TemporaryMetric {
    data: TemporaryMetricData::Histogram(histogram),
    ..
  }) = &mut temp.metric
  {
    histogram
      .data_points
      .last_mut()
      .unwrap()
      .bucket_counts
      .push(count1)
  }
}

#[op2(fast)]
fn op_otel_metrics_histogram_data_point_entry1(
  state: &mut OpState,
  #[number] count1: u64,
  bound1: f64,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(TemporaryMetric {
    data: TemporaryMetricData::Histogram(histogram),
    ..
  }) = &mut temp.metric
  {
    let data_point = histogram.data_points.last_mut().unwrap();
    data_point.bucket_counts.push(count1);
    data_point.bounds.push(bound1);
  }
}

#[op2(fast)]
fn op_otel_metrics_histogram_data_point_entry2(
  state: &mut OpState,
  #[number] count1: u64,
  bound1: f64,
  #[number] count2: u64,
  bound2: f64,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(TemporaryMetric {
    data: TemporaryMetricData::Histogram(histogram),
    ..
  }) = &mut temp.metric
  {
    let data_point = histogram.data_points.last_mut().unwrap();
    data_point.bucket_counts.push(count1);
    data_point.bounds.push(bound1);
    data_point.bucket_counts.push(count2);
    data_point.bounds.push(bound2);
  }
}

#[op2(fast)]
fn op_otel_metrics_histogram_data_point_entry3(
  state: &mut OpState,
  #[number] count1: u64,
  bound1: f64,
  #[number] count2: u64,
  bound2: f64,
  #[number] count3: u64,
  bound3: f64,
) {
  let Some(temp) = state.try_borrow_mut::<TemporaryMetricsExport>() else {
    return;
  };

  if let Some(TemporaryMetric {
    data: TemporaryMetricData::Histogram(histogram),
    ..
  }) = &mut temp.metric
  {
    let data_point = histogram.data_points.last_mut().unwrap();
    data_point.bucket_counts.push(count1);
    data_point.bounds.push(bound1);
    data_point.bucket_counts.push(count2);
    data_point.bounds.push(bound2);
    data_point.bucket_counts.push(count3);
    data_point.bounds.push(bound3);
  }
}

#[op2(fast)]
fn op_otel_metrics_data_point_attribute<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key: v8::Local<'s, v8::Value>,
  value: v8::Local<'s, v8::Value>,
) {
  if let Some(TemporaryMetricsExport {
    metric: Some(metric),
    ..
  }) = state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    let attributes = match &mut metric.data {
      TemporaryMetricData::Sum(sum) => {
        &mut sum.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Gauge(gauge) => {
        &mut gauge.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Histogram(histogram) => {
        &mut histogram.data_points.last_mut().unwrap().attributes
      }
    };
    attributes.reserve_exact((capacity as usize) - attributes.capacity());
    attr!(scope, attributes, key, value);
  }
}

#[op2(fast)]
fn op_otel_metrics_data_point_attribute2<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
) {
  if let Some(TemporaryMetricsExport {
    metric: Some(metric),
    ..
  }) = state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    let attributes = match &mut metric.data {
      TemporaryMetricData::Sum(sum) => {
        &mut sum.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Gauge(gauge) => {
        &mut gauge.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Histogram(histogram) => {
        &mut histogram.data_points.last_mut().unwrap().attributes
      }
    };
    attributes.reserve_exact((capacity as usize) - attributes.capacity());
    attr!(scope, attributes, key1, value1);
    attr!(scope, attributes, key2, value2);
  }
}

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_otel_metrics_data_point_attribute3<'s>(
  scope: &mut v8::HandleScope<'s>,
  state: &mut OpState,
  #[smi] capacity: u32,
  key1: v8::Local<'s, v8::Value>,
  value1: v8::Local<'s, v8::Value>,
  key2: v8::Local<'s, v8::Value>,
  value2: v8::Local<'s, v8::Value>,
  key3: v8::Local<'s, v8::Value>,
  value3: v8::Local<'s, v8::Value>,
) {
  if let Some(TemporaryMetricsExport {
    metric: Some(metric),
    ..
  }) = state.try_borrow_mut::<TemporaryMetricsExport>()
  {
    let attributes = match &mut metric.data {
      TemporaryMetricData::Sum(sum) => {
        &mut sum.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Gauge(gauge) => {
        &mut gauge.data_points.last_mut().unwrap().attributes
      }
      TemporaryMetricData::Histogram(histogram) => {
        &mut histogram.data_points.last_mut().unwrap().attributes
      }
    };
    attributes.reserve_exact((capacity as usize) - attributes.capacity());
    attr!(scope, attributes, key1, value1);
    attr!(scope, attributes, key2, value2);
    attr!(scope, attributes, key3, value3);
  }
}

#[op2(fast)]
fn op_otel_metrics_submit(state: &mut OpState) {
  let Some(mut temp) = state.try_take::<TemporaryMetricsExport>() else {
    return;
  };

  let Some(Processors { metrics, .. }) = OTEL_PROCESSORS.get() else {
    return;
  };

  if let Some(current_metric) = temp.metric {
    let metric = Metric::from(current_metric);
    temp.scope_metrics.last_mut().unwrap().metrics.push(metric);
  }

  let resource = Resource::new(temp.resource_attributes);
  let scope_metrics = temp.scope_metrics;

  metrics.submit(ResourceMetrics {
    resource,
    scope_metrics,
  });
}
