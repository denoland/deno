// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt;

use deno_core::anyhow;
use opentelemetry::trace::TracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use serde::Deserialize;
use serde::Serialize;
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;

use crate::lsp::logging::lsp_debug;

pub(crate) fn make_tracer(
  endpoint: Option<&str>,
) -> Result<opentelemetry_sdk::trace::Tracer, anyhow::Error> {
  let endpoint = endpoint.unwrap_or("http://localhost:4317");
  let exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_tonic()
    .with_endpoint(endpoint)
    .build()?;
  let provider = opentelemetry_sdk::trace::Builder::default()
    .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
    .with_resource(Resource::new(vec![KeyValue::new(SERVICE_NAME, "deno-lsp")]))
    .build();
  opentelemetry::global::set_tracer_provider(provider.clone());
  Ok(provider.tracer("deno-lsp-tracer"))
}

pub(crate) struct TracingGuard(
  // TODO(nathanwhit): use default guard here so we can change tracing after init
  // but needs wiring through the subscriber to the TSC thread, as it can't be a global default
  // #[allow(dead_code)] tracing::dispatcher::DefaultGuard,
  #[allow(dead_code)] (),
);

impl fmt::Debug for TracingGuard {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("TracingGuard").finish()
  }
}

impl Drop for TracingGuard {
  fn drop(&mut self) {
    lsp_debug!("Shutting down tracing");
    tokio::task::spawn_blocking(|| {
      opentelemetry::global::shutdown_tracer_provider()
    });
  }
}

#[derive(
  Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Copy, Default,
)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TracingCollector {
  #[default]
  OpenTelemetry,
  Logging,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct TracingConfig {
  /// Enable tracing.
  pub(crate) enable: bool,

  /// The collector to use. Defaults to `OpenTelemetry`.
  /// If `Logging` is used, the collected traces will be written to stderr.
  pub(crate) collector: TracingCollector,

  /// The filter to use. Defaults to `INFO`.
  pub(crate) filter: Option<String>,

  /// The endpoint to use for the OpenTelemetry collector.
  pub(crate) collector_endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum TracingConfigOrEnabled {
  Config(TracingConfig),
  Enabled(bool),
}

impl From<TracingConfig> for TracingConfigOrEnabled {
  fn from(value: TracingConfig) -> Self {
    TracingConfigOrEnabled::Config(value)
  }
}

impl From<TracingConfigOrEnabled> for TracingConfig {
  fn from(value: TracingConfigOrEnabled) -> Self {
    match value {
      TracingConfigOrEnabled::Config(config) => config,
      TracingConfigOrEnabled::Enabled(enabled) => TracingConfig {
        enable: enabled,
        ..Default::default()
      },
    }
  }
}

impl TracingConfigOrEnabled {
  pub(crate) fn enabled(&self) -> bool {
    match self {
      TracingConfigOrEnabled::Config(config) => config.enable,
      TracingConfigOrEnabled::Enabled(enabled) => *enabled,
    }
  }
}

pub(crate) fn init_tracing_subscriber(
  config: &TracingConfig,
) -> Result<TracingGuard, anyhow::Error> {
  if !config.enable {
    return Err(anyhow::anyhow!("Tracing is not enabled"));
  }
  let filter = tracing_subscriber::EnvFilter::builder()
    .with_default_directive(LevelFilter::INFO.into());
  let filter = if let Some(directive) = config.filter.as_ref() {
    filter.parse(directive)?
  } else {
    filter.with_env_var("DENO_LSP_TRACE").from_env()?
  };
  let open_telemetry_layer = match config.collector {
    TracingCollector::OpenTelemetry => Some(OpenTelemetryLayer::new(
      make_tracer(config.collector_endpoint.as_deref())?,
    )),
    _ => None,
  };
  let logging_layer = match config.collector {
    TracingCollector::Logging => Some(
      tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        // Include span events in the log output.
        // Without this, only events get logged (and at the moment we have none).
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
    ),
    _ => None,
  };

  tracing::subscriber::set_global_default(
    tracing_subscriber::registry()
      .with(filter)
      .with(logging_layer)
      .with(open_telemetry_layer),
  )
  .unwrap();

  let guard = ();
  Ok(TracingGuard(guard))
}
