// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::fmt;

use crate::lsp::logging::lsp_debug;
use deno_core::anyhow;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::BatchConfigBuilder;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use serde::Deserialize;
use serde::Serialize;
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub(crate) fn make_tracer(
  endpoint: Option<&str>,
) -> Result<opentelemetry_sdk::trace::Tracer, anyhow::Error> {
  Ok(
    opentelemetry_otlp::new_pipeline()
      .tracing()
      .with_exporter(
        opentelemetry_otlp::new_exporter()
          .tonic()
          .with_endpoint(endpoint.unwrap_or("http://localhost:4317")),
      )
      .with_trace_config(
        opentelemetry_sdk::trace::config()
          .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn)
          .with_resource(Resource::new(vec![KeyValue::new(
            SERVICE_NAME,
            "deno-lsp",
          )])),
      )
      .with_batch_config(
        BatchConfigBuilder::default()
          .with_max_queue_size(8192)
          .build(),
      )
      .install_batch(opentelemetry_sdk::runtime::Tokio)?,
  )
}

pub(crate) struct TracingGuard(tracing::dispatcher::DefaultGuard);

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

  let guard = tracing_subscriber::registry()
    .with(filter)
    .with(logging_layer)
    .with(open_telemetry_layer)
    .set_default();
  Ok(TracingGuard(guard))
}
