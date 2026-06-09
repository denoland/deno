// Copyright 2018-2026 the Deno authors. MIT license.

use std::thread;

use deno_core::ModuleSpecifier;
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
    }
  }
}

impl BootstrapOptions {
  /// Return the v8 equivalent of this structure.
  ///
  /// Produces a JS array whose indices match what `99_main.js` destructures.
  /// Keep this in sync with `99_main.js`.
  pub fn as_v8<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Value> {
    use deno_core::convert::ToV8 as _;

    // 0: denoVersion
    let v0: v8::Local<v8::Value> =
      v8::String::new(scope, &self.deno_version).unwrap().into();
    // 1: location
    let v1: v8::Local<v8::Value> =
      match self.location.as_ref().map(|l| l.as_str()) {
        Some(s) => v8::String::new(scope, s).unwrap().into(),
        None => v8::null(scope).into(),
      };
    // 2: unstableFeatures — array of i32
    let mut unstable_elems: Vec<v8::Local<v8::Value>> =
      Vec::with_capacity(self.unstable_features.len());
    for &n in &self.unstable_features {
      unstable_elems.push(v8::Integer::new(scope, n).into());
    }
    let v2: v8::Local<v8::Value> =
      v8::Array::new_with_elements(scope, &unstable_elems).into();
    // 3: inspect
    let v3: v8::Local<v8::Value> = v8::Boolean::new(scope, self.inspect).into();
    // 4: enableTestingFeatures
    let v4: v8::Local<v8::Value> =
      v8::Boolean::new(scope, self.enable_testing_features).into();
    // 5: hasNodeModulesDir
    let v5: v8::Local<v8::Value> =
      v8::Boolean::new(scope, self.has_node_modules_dir).into();
    // 6: argv0
    let v6: v8::Local<v8::Value> = match self.argv0.as_deref() {
      Some(s) => v8::String::new(scope, s).unwrap().into(),
      None => v8::null(scope).into(),
    };
    // 7: nodeDebug
    let v7: v8::Local<v8::Value> = match self.node_debug.as_deref() {
      Some(s) => v8::String::new(scope, s).unwrap().into(),
      None => v8::null(scope).into(),
    };
    // 8: mode (discriminant as integer)
    let v8_val: v8::Local<v8::Value> =
      v8::Integer::new(scope, self.mode.discriminant() as i32).into();
    // 9: servePort (u16 → unsigned integer)
    let v9: v8::Local<v8::Value> = v8::Integer::new_from_unsigned(
      scope,
      self.serve_port.unwrap_or_default() as u32,
    )
    .into();
    // 10: serveHost
    let v10: v8::Local<v8::Value> = match self.serve_host.as_deref() {
      Some(s) => v8::String::new(scope, s).unwrap().into(),
      None => v8::null(scope).into(),
    };
    // 11: serveIsMain
    let v11: v8::Local<v8::Value> = v8::Boolean::new(
      scope,
      matches!(self.mode, WorkerExecutionMode::ServeMain { .. }),
    )
    .into();
    // 12: serveWorkerCountOrIndex (null when not in serve mode)
    let v12: v8::Local<v8::Value> = match self.mode {
      WorkerExecutionMode::ServeMain { worker_count } => {
        v8::Number::new(scope, worker_count as f64).into()
      }
      WorkerExecutionMode::ServeWorker { worker_index } => {
        v8::Number::new(scope, worker_index as f64).into()
      }
      _ => v8::null(scope).into(),
    };
    // 13: otelConfig as Uint8Array
    let v13 = deno_core::convert::Uint8Array(self.otel_config.as_v8())
      .to_v8(scope)
      .unwrap();
    // 14: closeOnIdle
    let v14: v8::Local<v8::Value> =
      v8::Boolean::new(scope, self.close_on_idle).into();
    // 15: standalone
    let v15: v8::Local<v8::Value> =
      v8::Boolean::new(scope, self.is_standalone).into();
    // 16: autoServe
    let v16: v8::Local<v8::Value> =
      v8::Boolean::new(scope, self.auto_serve).into();
    // 17: nodeClusterUniqueId
    let v17: v8::Local<v8::Value> = match self.node_cluster_unique_id.as_deref()
    {
      Some(s) => v8::String::new(scope, s).unwrap().into(),
      None => v8::null(scope).into(),
    };
    // 18: nodeClusterSchedPolicy
    let v18: v8::Local<v8::Value> =
      match self.node_cluster_sched_policy.as_deref() {
        Some(s) => v8::String::new(scope, s).unwrap().into(),
        None => v8::null(scope).into(),
      };

    let elements: [v8::Local<v8::Value>; 19] = [
      v0, v1, v2, v3, v4, v5, v6, v7, v8_val, v9, v10, v11, v12, v13, v14, v15,
      v16, v17, v18,
    ];
    v8::Array::new_with_elements(scope, &elements).into()
  }
}
