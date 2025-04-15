// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;

use deno_runtime::deno_telemetry;
use deno_runtime::deno_telemetry::OtelConfig;
use deno_runtime::deno_telemetry::OtelConsoleConfig;

struct CliLogger<FnOnLogStart: Fn(), FnOnLogEnd: Fn()> {
  otel_console_config: OtelConsoleConfig,
  logger: env_logger::Logger,
  on_log_start: FnOnLogStart,
  on_log_end: FnOnLogEnd,
}

impl<FnOnLogStart: Fn(), FnOnLogEnd: Fn()> CliLogger<FnOnLogStart, FnOnLogEnd> {
  pub fn filter(&self) -> log::LevelFilter {
    self.logger.filter()
  }
}

impl<FnOnLogStart: Fn() + Send + Sync, FnOnLogEnd: Fn() + Send + Sync> log::Log
  for CliLogger<FnOnLogStart, FnOnLogEnd>
{
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    self.logger.enabled(metadata)
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      (self.on_log_start)();

      match self.otel_console_config {
        OtelConsoleConfig::Ignore => {
          self.logger.log(record);
        }
        OtelConsoleConfig::Capture => {
          self.logger.log(record);
          deno_telemetry::handle_log(record);
        }
        OtelConsoleConfig::Replace => {
          deno_telemetry::handle_log(record);
        }
      }

      (self.on_log_end)();
    }
  }

  fn flush(&self) {
    self.logger.flush();
  }
}

pub struct InitLoggingOptions<FnOnLogStart: Fn(), FnOnLogEnd: Fn()> {
  pub on_log_start: FnOnLogStart,
  pub on_log_end: FnOnLogEnd,
  pub maybe_level: Option<log::Level>,
  pub otel_config: Option<OtelConfig>,
}

pub fn init<
  FOnLogStart: Fn() + Send + Sync + 'static,
  FnOnLogEnd: Fn() + Send + Sync + 'static,
>(
  options: InitLoggingOptions<FOnLogStart, FnOnLogEnd>,
) {
  let log_level = options.maybe_level.unwrap_or(log::Level::Info);
  let logger = env_logger::Builder::from_env(
    env_logger::Env::new()
      // Use `DENO_LOG` and `DENO_LOG_STYLE` instead of `RUST_` prefix
      .filter_or("DENO_LOG", log_level.to_level_filter().to_string())
      .write_style("DENO_LOG_STYLE"),
  )
  // https://github.com/denoland/deno/issues/6641
  .filter_module("rustyline", log::LevelFilter::Off)
  // wgpu crates (gfx_backend), have a lot of useless INFO and WARN logs
  .filter_module("wgpu", log::LevelFilter::Error)
  .filter_module("gfx", log::LevelFilter::Error)
  .filter_module("globset", log::LevelFilter::Error)
  // used to make available the lsp_debug which is then filtered out at runtime
  // in the cli logger
  .filter_module("deno::lsp::performance", log::LevelFilter::Debug)
  .filter_module("rustls", log::LevelFilter::Off)
  // swc_ecma_codegen's `srcmap!` macro emits error-level spans only on debug
  // build:
  // https://github.com/swc-project/swc/blob/74d6478be1eb8cdf1df096c360c159db64b64d8a/crates/swc_ecma_codegen/src/macros.rs#L112
  // We suppress them here to avoid flooding our CI logs in integration tests.
  .filter_module("swc_ecma_codegen", log::LevelFilter::Off)
  .filter_module("swc_ecma_transforms_optimization", log::LevelFilter::Off)
  .filter_module("swc_ecma_parser", log::LevelFilter::Error)
  // Suppress span lifecycle logs since they are too verbose
  .filter_module("tracing::span", log::LevelFilter::Off)
  .filter_module("tower_lsp", log::LevelFilter::Trace)
  .filter_module("opentelemetry_sdk", log::LevelFilter::Off)
  // for deno_compile, this is too verbose
  .filter_module("editpe", log::LevelFilter::Error)
  // too verbose
  .filter_module("cranelift_codegen", log::LevelFilter::Off)
  .format(|buf, record| {
    let mut target = record.target().to_string();
    if let Some(line_no) = record.line() {
      target.push(':');
      target.push_str(&line_no.to_string());
    }
    if record.level() <= log::Level::Info
      || (record.target() == "deno::lsp::performance"
        && record.level() == log::Level::Debug)
    {
      // Print ERROR, WARN, INFO and lsp_debug logs as they are
      writeln!(buf, "{}", record.args())
    } else {
      // Add prefix to DEBUG or TRACE logs
      writeln!(
        buf,
        "{} RS - {} - {}",
        record.level(),
        target,
        record.args()
      )
    }
  })
  .build();

  let cli_logger = CliLogger {
    on_log_start: options.on_log_start,
    on_log_end: options.on_log_end,
    logger,
    otel_console_config: options
      .otel_config
      .map(|c| c.console)
      .unwrap_or(OtelConsoleConfig::Ignore),
  };
  let max_level = cli_logger.filter();
  let r = log::set_boxed_logger(Box::new(cli_logger));
  if r.is_ok() {
    log::set_max_level(max_level);
  }
  r.expect("Could not install logger.");
}
