// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::ops::otel::OtelConfig;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use std::cell::RefCell;
use std::thread;

use deno_terminal::colors;

/// The execution mode for this worker. Some modes may have implicit behaviour.
#[derive(Copy, Clone)]
pub enum WorkerExecutionMode {
  /// No special behaviour.
  None,

  /// Running in a worker.
  Worker,
  /// `deno run`
  Run,
  /// `deno repl`
  Repl,
  /// `deno eval`
  Eval,
  /// `deno test`
  Test,
  /// `deno bench`
  Bench,
  /// `deno serve`
  Serve {
    is_main: bool,
    worker_count: Option<usize>,
  },
  /// `deno jupyter`
  Jupyter,
}

impl WorkerExecutionMode {
  pub fn discriminant(&self) -> u8 {
    match self {
      WorkerExecutionMode::None => 0,
      WorkerExecutionMode::Worker => 1,
      WorkerExecutionMode::Run => 2,
      WorkerExecutionMode::Repl => 3,
      WorkerExecutionMode::Eval => 4,
      WorkerExecutionMode::Test => 5,
      WorkerExecutionMode::Bench => 6,
      WorkerExecutionMode::Serve { .. } => 7,
      WorkerExecutionMode::Jupyter => 8,
    }
  }
  pub fn serve_info(&self) -> (Option<bool>, Option<usize>) {
    match *self {
      WorkerExecutionMode::Serve {
        is_main,
        worker_count,
      } => (Some(is_main), worker_count),
      _ => (None, None),
    }
  }
}

/// The log level to use when printing diagnostic log messages, warnings,
/// or errors in the worker.
///
/// Note: This is disconnected with the log crate's log level and the Rust code
/// in this crate will respect that value instead. To specify that, use
/// `log::set_max_level`.
#[derive(Debug, Default, Clone, Copy)]
pub enum WorkerLogLevel {
  // WARNING: Ensure this is kept in sync with
  // the JS values (search for LogLevel).
  Error = 1,
  Warn = 2,
  #[default]
  Info = 3,
  Debug = 4,
}

impl From<log::Level> for WorkerLogLevel {
  fn from(value: log::Level) -> Self {
    match value {
      log::Level::Error => WorkerLogLevel::Error,
      log::Level::Warn => WorkerLogLevel::Warn,
      log::Level::Info => WorkerLogLevel::Info,
      log::Level::Debug => WorkerLogLevel::Debug,
      log::Level::Trace => WorkerLogLevel::Debug,
    }
  }
}

/// Common bootstrap options for MainWorker & WebWorker
#[derive(Clone)]
pub struct BootstrapOptions {
  pub deno_version: String,
  /// Sets `Deno.args` in JS runtime.
  pub args: Vec<String>,
  pub cpu_count: usize,
  pub log_level: WorkerLogLevel,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub locale: String,
  pub location: Option<ModuleSpecifier>,
  /// Sets `Deno.noColor` in JS runtime.
  pub no_color: bool,
  pub is_stdout_tty: bool,
  pub is_stderr_tty: bool,
  pub color_level: deno_terminal::colors::ColorLevel,
  // --unstable-* flags
  pub unstable_features: Vec<i32>,
  pub user_agent: String,
  pub inspect: bool,
  pub has_node_modules_dir: bool,
  pub argv0: Option<String>,
  pub node_debug: Option<String>,
  pub node_ipc_fd: Option<i64>,
  pub mode: WorkerExecutionMode,
  // Used by `deno serve`
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
  // OpenTelemetry output options. If `None`, OpenTelemetry is disabled.
  pub otel_config: Option<OtelConfig>,
}

impl Default for BootstrapOptions {
  fn default() -> Self {
    let cpu_count = thread::available_parallelism()
      .map(|p| p.get())
      .unwrap_or(1);

    let runtime_version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("Deno/{runtime_version}");

    Self {
      deno_version: runtime_version.to_string(),
      user_agent,
      cpu_count,
      no_color: !colors::use_color(),
      is_stdout_tty: deno_terminal::is_stdout_tty(),
      is_stderr_tty: deno_terminal::is_stderr_tty(),
      color_level: colors::get_color_level(),
      enable_op_summary_metrics: Default::default(),
      enable_testing_features: Default::default(),
      log_level: Default::default(),
      locale: "en".to_string(),
      location: Default::default(),
      unstable_features: Default::default(),
      inspect: Default::default(),
      args: Default::default(),
      has_node_modules_dir: Default::default(),
      argv0: None,
      node_debug: None,
      node_ipc_fd: None,
      mode: WorkerExecutionMode::None,
      serve_port: Default::default(),
      serve_host: Default::default(),
      otel_config: None,
    }
  }
}

/// This is a struct that we use to serialize the contents of the `BootstrapOptions`
/// struct above to a V8 form. While `serde_v8` is not as fast as hand-coding this,
/// it's "fast enough" while serializing a large tuple like this that it doesn't appear
/// on flamegraphs.
///
/// Note that a few fields in here are derived from the process and environment and
/// are not sourced from the underlying `BootstrapOptions`.
///
/// Keep this in sync with `99_main.js`.
#[derive(Serialize)]
struct BootstrapV8<'a>(
  // deno version
  &'a str,
  // location
  Option<&'a str>,
  // granular unstable flags
  &'a [i32],
  // inspect
  bool,
  // enable_testing_features
  bool,
  // has_node_modules_dir
  bool,
  // argv0
  Option<&'a str>,
  // node_debug
  Option<&'a str>,
  // mode
  i32,
  // serve port
  u16,
  // serve host
  Option<&'a str>,
  // serve is main
  Option<bool>,
  // serve worker count
  Option<usize>,
  // OTEL config
  Box<[u8]>,
);

impl BootstrapOptions {
  /// Return the v8 equivalent of this structure.
  pub fn as_v8<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::Local<'s, v8::Value> {
    let scope = RefCell::new(scope);
    let ser = deno_core::serde_v8::Serializer::new(&scope);

    let (serve_is_main, serve_worker_count) = self.mode.serve_info();
    let bootstrap = BootstrapV8(
      &self.deno_version,
      self.location.as_ref().map(|l| l.as_str()),
      self.unstable_features.as_ref(),
      self.inspect,
      self.enable_testing_features,
      self.has_node_modules_dir,
      self.argv0.as_deref(),
      self.node_debug.as_deref(),
      self.mode.discriminant() as _,
      self.serve_port.unwrap_or_default(),
      self.serve_host.as_deref(),
      serve_is_main,
      serve_worker_count,
      if let Some(otel_config) = self.otel_config.as_ref() {
        Box::new([otel_config.console as u8, otel_config.deterministic as u8])
      } else {
        Box::new([])
      },
    );

    bootstrap.serialize(ser).unwrap()
  }
}
