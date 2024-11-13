// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::tokio_util::create_basic_runtime;
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
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::logs::BatchLogProcessor;
use opentelemetry_sdk::logs::LogProcessor as LogProcessorTrait;
use opentelemetry_sdk::logs::LogRecord;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::SpanProcessor as SpanProcessorTrait;
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

type SpanProcessor = BatchSpanProcessor<OtelSharedRuntime>;
type LogProcessor = BatchLogProcessor<OtelSharedRuntime>;

deno_core::extension!(
  deno_otel,
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
  ],
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelConfig {
  pub runtime_name: Cow<'static, str>,
  pub runtime_version: Cow<'static, str>,
  pub console: OtelConsoleConfig,
  pub deterministic: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum OtelConsoleConfig {
  Ignore = 0,
  Capture = 1,
  Replace = 2,
}

impl Default for OtelConfig {
  fn default() -> Self {
    Self {
      runtime_name: Cow::Borrowed(env!("CARGO_PKG_NAME")),
      runtime_version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
      console: OtelConsoleConfig::Capture,
      deterministic: false,
    }
  }
}

static OTEL_SHARED_RUNTIME_SPAWN_TASK_TX: Lazy<
  UnboundedSender<BoxFuture<'static, ()>>,
> = Lazy::new(otel_create_shared_runtime);

fn otel_create_shared_runtime() -> UnboundedSender<BoxFuture<'static, ()>> {
  let (spawn_task_tx, mut spawn_task_rx) =
    mpsc::unbounded::<BoxFuture<'static, ()>>();

  thread::spawn(move || {
    let rt = create_basic_runtime();
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

static OTEL_PROCESSORS: OnceCell<(SpanProcessor, LogProcessor)> =
  OnceCell::new();

static BUILT_IN_INSTRUMENTATION_SCOPE: OnceCell<
  opentelemetry::InstrumentationScope,
> = OnceCell::new();

pub fn init(config: OtelConfig) -> anyhow::Result<()> {
  // Parse the `OTEL_EXPORTER_OTLP_PROTOCOL` variable. The opentelemetry_*
  // crates don't do this automatically.
  // TODO(piscisaureus): enable GRPC support.
  let protocol = match env::var("OTEL_EXPORTER_OTLP_PROTOCOL").as_deref() {
    Ok("http/protobuf") => Protocol::HttpBinary,
    Ok("http/json") => Protocol::HttpJson,
    Ok("") | Err(env::VarError::NotPresent) => {
      return Ok(());
    }
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

  let log_exporter = HttpExporterBuilder::default()
    .with_http_client(client)
    .with_protocol(protocol)
    .build_log_exporter()?;
  let log_processor =
    BatchLogProcessor::builder(log_exporter, OtelSharedRuntime).build();
  log_processor.set_resource(&resource);

  OTEL_PROCESSORS
    .set((span_processor, log_processor))
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
  if let Some((span_processor, log_processor)) = OTEL_PROCESSORS.get() {
    let _ = span_processor.force_flush();
    let _ = log_processor.force_flush();
  }
}

pub fn handle_log(record: &log::Record) {
  use log::Level;

  let Some((_, log_processor)) = OTEL_PROCESSORS.get() else {
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

  log_processor.emit(
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
  state.put(InstrumentationScope(
    BUILT_IN_INSTRUMENTATION_SCOPE.get().unwrap().clone(),
  ));
}

#[op2(fast)]
fn op_otel_log(
  state: &mut OpState,
  scope: &mut v8::HandleScope<'_>,
  #[string] message: String,
  #[smi] level: i32,
  trace_id: v8::Local<'_, v8::Value>,
  span_id: v8::Local<'_, v8::Value>,
  #[smi] trace_flags: u8,
) {
  let Some((_, log_processor)) = OTEL_PROCESSORS.get() else {
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

  log_processor.emit(
    &mut log_record,
    BUILT_IN_INSTRUMENTATION_SCOPE.get().unwrap(),
  );
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
    let Some((span_processor, _)) = OTEL_PROCESSORS.get() else {
      return Ok(());
    };
    span_processor.on_end(temporary_span.0);
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

  let name = {
    let x = v8::ValueView::new(scope, name.try_cast()?);
    match x.data() {
      v8::ValueViewData::OneByte(bytes) => {
        String::from_utf8_lossy(bytes).into_owned()
      }
      v8::ValueViewData::TwoByte(bytes) => String::from_utf16_lossy(bytes),
    }
  };

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
    temporary_span.0.dropped_attributes_count = dropped_attributes_count;
    temporary_span.0.links.dropped_count = dropped_links_count;
    temporary_span.0.events.dropped_count = dropped_events_count;
  }
}

#[op2(fast)]
fn op_otel_span_flush(state: &mut OpState) {
  let Some(temporary_span) = state.try_take::<TemporarySpan>() else {
    return;
  };

  let Some((span_processor, _)) = OTEL_PROCESSORS.get() else {
    return;
  };

  span_processor.on_end(temporary_span.0);
}
