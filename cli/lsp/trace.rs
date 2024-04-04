// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::fmt;

use crate::lsp::logging::lsp_debug;
use deno_core::anyhow;
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
use tracing_subscriber::util::SubscriberInitExt;

pub(crate) fn make_tracer(
) -> Result<opentelemetry_sdk::trace::Tracer, anyhow::Error> {
  Ok(
    opentelemetry_otlp::new_pipeline()
      .tracing()
      .with_exporter(
        opentelemetry_otlp::new_exporter()
          .tonic()
          .with_endpoint("http://localhost:4317"),
      )
      .with_trace_config(
        opentelemetry_sdk::trace::config()
          .with_sampler(opentelemetry_sdk::trace::Sampler::AlwaysOn)
          .with_resource(Resource::new(vec![KeyValue::new(
            SERVICE_NAME,
            "deno-lsp",
          )])),
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
#[serde(default)]
pub(crate) struct TracingConfig {
  pub(crate) enable: bool,

  pub(crate) collector: TracingCollector,

  pub(crate) filter: Option<String>,
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
    TracingCollector::OpenTelemetry => {
      Some(OpenTelemetryLayer::new(make_tracer()?))
    }
    _ => None,
  };
  let logging_layer = match config.collector {
    TracingCollector::Logging => Some(
      tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
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
