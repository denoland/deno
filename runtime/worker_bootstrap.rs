// Copyright 2018-2026 the Deno authors. MIT license.

use std::thread;

use deno_core::ModuleSpecifier;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::v8;
use deno_node::ops::ipc::ChildIpcSerialization;
use deno_telemetry::OtelConfig;
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
  ServeMain {
    worker_count: usize,
  },
  ServeWorker {
    worker_index: usize,
  },
  /// `deno jupyter`
  Jupyter,
  /// `deno deploy`
  Deploy,
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
      WorkerExecutionMode::ServeMain { .. }
      | WorkerExecutionMode::ServeWorker { .. } => 7,
      WorkerExecutionMode::Jupyter => 8,
      WorkerExecutionMode::Deploy => 9,
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
  pub enable_testing_features: bool,
  pub locale: String,
  pub location: Option<ModuleSpecifier>,
  pub color_level: deno_terminal::colors::ColorLevel,
  // --unstable-* flags
  pub unstable_features: Vec<i32>,
  pub user_agent: String,
  pub inspect: bool,
  /// If this is a `deno compile`-ed executable.
  pub is_standalone: bool,
  pub has_node_modules_dir: bool,
  pub argv0: Option<String>,
  pub node_debug: Option<String>,
  pub node_cluster_unique_id: Option<String>,
  pub node_cluster_sched_policy: Option<String>,
  pub node_ipc_init: Option<(i64, ChildIpcSerialization)>,
  pub mode: WorkerExecutionMode,
  pub no_legacy_abort: bool,
  // Used by `deno serve`
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
  pub auto_serve: bool,
  pub otel_config: OtelConfig,
  pub close_on_idle: bool,
  /// When true, the `OffscreenCanvas` global is removed at bootstrap.
  pub disable_offscreen_canvas: bool,
}

impl Default for BootstrapOptions {
  fn default() -> Self {
    let cpu_count = thread::available_parallelism()
      .map(|p| p.get())
      .unwrap_or(1);

    // this version is not correct as its the version of deno_runtime
    // and the implementor should supply a user agent that makes sense
    let runtime_version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("Deno/{runtime_version}");

    Self {
      deno_version: runtime_version.to_string(),
      user_agent,
      cpu_count,
      color_level: colors::get_color_level(),
      enable_testing_features: false,
      log_level: Default::default(),
      locale: "en".to_string(),
      location: Default::default(),
      unstable_features: Default::default(),
      inspect: false,
      args: Default::default(),
      is_standalone: false,
      auto_serve: false,
      has_node_modules_dir: false,
      argv0: None,
      node_debug: None,
      node_cluster_unique_id: None,
      node_cluster_sched_policy: None,
      node_ipc_init: None,
      mode: WorkerExecutionMode::None,
      no_legacy_abort: false,
      serve_port: Default::default(),
      serve_host: Default::default(),
      otel_config: Default::default(),
      close_on_idle: false,
      disable_offscreen_canvas: false,
    }
  }
}

/// Serialized form of `BootstrapOptions` passed to JS as a positional array.
///
/// Each element maps to the corresponding index destructured in `99_main.js`.
/// Keep in sync with `99_main.js`.
#[derive(ToV8)]
struct BootstrapV8<'a>(
  // 0: deno version
  &'a str,
  // 1: location
  Option<&'a str>,
  // 2: granular unstable flags
  &'a [i32],
  // 3: inspect
  bool,
  // 4: enable_testing_features
  bool,
  // 5: has_node_modules_dir
  bool,
  // 6: argv0
  Option<&'a str>,
  // 7: node_debug
  Option<&'a str>,
  // 8: mode
  i32,
  // 9: serve port
  u16,
  // 10: serve host
  Option<&'a str>,
  // 11: serve is main
  bool,
  // 12: serve worker count
  Option<usize>,
  // 13: OTEL config
  Uint8Array,
  // 14: close on idle
  bool,
  // 15: is_standalone
  bool,
  // 16: auto serve
  bool,
  // 17: node cluster unique id (NODE_UNIQUE_ID)
  Option<&'a str>,
  // 18: node cluster scheduling policy (NODE_CLUSTER_SCHED_POLICY)
  Option<&'a str>,
  // disable offscreen canvas
  bool,
);

impl BootstrapOptions {
  /// Return the v8 equivalent of this structure.
  pub fn as_v8<'s>(
    &'s self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    BootstrapV8(
      &self.deno_version,
      self.location.as_ref().map(|l| l.as_str()),
      &self.unstable_features,
      self.inspect,
      self.enable_testing_features,
      self.has_node_modules_dir,
      self.argv0.as_deref(),
      self.node_debug.as_deref(),
      self.mode.discriminant() as i32,
      self.serve_port.unwrap_or_default(),
      self.serve_host.as_deref(),
      matches!(self.mode, WorkerExecutionMode::ServeMain { .. }),
      match self.mode {
        WorkerExecutionMode::ServeMain { worker_count } => Some(worker_count),
        WorkerExecutionMode::ServeWorker { worker_index } => Some(worker_index),
        _ => None,
      },
      self.otel_config.as_v8().into(),
      self.close_on_idle,
      self.is_standalone,
      self.auto_serve,
      self.node_cluster_unique_id.as_deref(),
      self.node_cluster_sched_policy.as_deref(),
      self.disable_offscreen_canvas,
    )
    .to_v8(scope)
    .expect("BootstrapV8::to_v8 failed")
  }
}
