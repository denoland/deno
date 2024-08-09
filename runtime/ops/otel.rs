// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::tokio_util::create_basic_runtime;
use anyhow::anyhow;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::mpsc::UnboundedSender;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::stream;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::OpState;
use maplit::hashmap;
use once_cell::sync::Lazy;
use opentelemetry::logs::LogRecord;
use opentelemetry::logs::Logger as LoggerTrait;
use opentelemetry::logs::LoggerProvider;
use opentelemetry::logs::Severity;
use opentelemetry::Key;
use opentelemetry::KeyValue;
use opentelemetry_otlp::HttpExporterBuilder;
use opentelemetry_otlp::LogExporterBuilder;
use opentelemetry_sdk::logs::Logger;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use opentelemetry_semantic_conventions::resource::SERVICE_VERSION;
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

deno_core::extension!(
  deno_otel,
  ops = [op_otel_log],
  options = {
    otel_config: Option<OtelConfig>, // `None` means OpenTelemetry is disabled.
  },
  state = |state, options| {
    if let Some(otel_config) = options.otel_config {
      let logger = otel_create_logger(otel_config)
        .expect("Failed to create OpenTelemetry logger");
      state.put::<Logger>(logger);
    }
  }
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelConfig {
  pub default_service_name: Cow<'static, str>,
  pub default_service_version: Cow<'static, str>,
}

impl Default for OtelConfig {
  fn default() -> Self {
    Self {
      default_service_name: Cow::Borrowed(env!("CARGO_PKG_NAME")),
      default_service_version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
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

fn otel_create_logger(config: OtelConfig) -> anyhow::Result<Logger> {
  // Parse the `OTEL_EXPORTER_OTLP_PROTOCOL` variable. The opentelemetry_*
  // crates don't do this automatically. Currently, the only supported protocol
  // is "http/protobuf".
  // TODO(piscisaureus): enable GRPC support.
  let _protocol = match env::var("OTEL_EXPORTER_OTLP_PROTOCOL").as_deref() {
    Ok(protocol @ "http/protobuf") => protocol,
    Ok("") | Err(env::VarError::NotPresent) => {
      return Err(anyhow!("OTEL_EXPORTER_OTLP_PROTOCOL must be set",))
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
      ))
    }
  };

  // Verify that `OTEL_EXPORTER_OTLP_ENDPOINT` is set. If unspecified,
  // `HttpExporterBuilder` will use http://localhost:4317 as the default
  // endpoint, but this seems not very useful.
  match env::var("OTEL_EXPORTER_OTLP_ENDPOINT").as_deref() {
    Ok(endpoint) if !endpoint.is_empty() => {}
    Ok(_) | Err(env::VarError::NotPresent) => {
      return Err(anyhow!("OTEL_EXPORTER_OTLP_ENDPOINT must be set",))
    }
    Err(err) => {
      return Err(anyhow!(
        "Failed to read env var OTEL_EXPORTER_OTLP_ENDPOINT: {}",
        err
      ))
    }
  };

  // The OTLP endpoint is automatically picked up from the
  // `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable. Additional headers can
  // be specified using `OTEL_EXPORTER_OTLP_HEADERS`.
  let exporter = LogExporterBuilder::Http(
    HttpExporterBuilder::default().with_http_client(reqwest::Client::new()),
  );

  // Define the resource attributes that will be attached to all log records.
  // These attributes are sourced as follows (in order of precedence):
  //   * The `service.name` attribute from the `OTEL_SERVICE_NAME` env var.
  //   * Additional attributes from the `OTEL_RESOURCE_ATTRIBUTES` env var.
  //   * Default attribute values defined here.
  // TODO(piscisaureus): add more default attributes (e.g. script path).
  let mut resource = Resource::default();
  // The default service name assigned by `Resource::default()`, if not
  // otherwise specified via environment variables, is "unknown_service".
  // Override this with the current crate name and version.
  if resource
    .get(Key::from_static_str(SERVICE_NAME))
    .filter(|service_name| service_name.as_str() != "unknown_service")
    .is_none()
  {
    resource = resource.merge(&Resource::new(
      hashmap! {
        SERVICE_NAME => config.default_service_name,
        SERVICE_VERSION => config.default_service_version,
      }
      .into_iter()
      .map(|(k, v)| KeyValue::new(k, v)),
    ))
  }

  let logging_provider = opentelemetry_otlp::new_pipeline()
    .logging()
    .with_exporter(exporter)
    .with_resource(resource)
    .install_batch(OtelSharedRuntime)?;

  // Create the `Logger` instance that will be used to emit console logs.
  // The "console" argument is used to specify the `otel.scope.name` attribute,
  // which is a standard attribute to instrumentation scope of log records.
  let logger = logging_provider.logger_builder("console").build();

  Ok(logger)
}

/// This function is called by the runtime whenever it is about to call
/// `os::process::exit()`, to ensure that all OpenTelemetry logs are properly
/// flushed before the process terminates.
pub fn otel_drop_logger(state: &mut OpState) {
  let Some(logger) = state.try_take::<Logger>() else {
    // Since this function is called unconditionaly before `os::process::exit()`,
    // it is not an error if the logger is not available.
    return;
  };

  // When the `Logger` is dropped, the underlying `LoggerProvider` will be
  // dropped as well. The provider's Drop implementation will flush all logs
  // and block until the exporter has successfully sent them.
  drop(logger);
}

#[op2(fast)]
fn op_otel_log(
  state: &mut OpState,
  #[string] message: String,
  #[smi] level: i32,
) {
  let Some(logger) = state.try_borrow::<Logger>() else {
    log::error!("op_otel_log: OpenTelemetry Logger not available");
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

  let mut log_record = logger.create_log_record();
  log_record.set_body(message.into());
  log_record.set_severity_number(severity);
  log_record.set_severity_text(Cow::Borrowed(severity.name()));
  logger.emit(log_record);
}
