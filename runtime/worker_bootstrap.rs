// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::v8;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use std::cell::RefCell;
use std::thread;

use deno_terminal::colors;

/// The execution mode for this worker. Some modes may have implicit behaviour.
#[derive(Copy, Clone)]
#[repr(u8)]
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
  Serve,
  /// `deno jupyter`
  Jupyter,
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
  // --unstable flag, deprecated
  pub unstable: bool,
  // --unstable-* flags
  pub unstable_features: Vec<i32>,
  pub user_agent: String,
  pub inspect: bool,
  pub has_node_modules_dir: bool,
  pub argv0: Option<String>,
  pub node_debug: Option<String>,
  pub node_ipc_fd: Option<i64>,
  pub disable_deprecated_api_warning: bool,
  pub verbose_deprecated_api_warning: bool,
  pub future: bool,
  pub mode: WorkerExecutionMode,
  // Used by `deno serve`
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
}

impl Default for BootstrapOptions {
  fn default() -> Self {
    let cpu_count = thread::available_parallelism()
      .map(|p| p.get())
      .unwrap_or(1);

    let runtime_version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("Deno/{runtime_version}");

    Self {
      user_agent,
      cpu_count,
      no_color: !colors::use_color(),
      is_stdout_tty: deno_terminal::is_stdout_tty(),
      is_stderr_tty: deno_terminal::is_stderr_tty(),
      enable_op_summary_metrics: Default::default(),
      enable_testing_features: Default::default(),
      log_level: Default::default(),
      locale: "en".to_string(),
      location: Default::default(),
      unstable: Default::default(),
      unstable_features: Default::default(),
      inspect: Default::default(),
      args: Default::default(),
      has_node_modules_dir: Default::default(),
      argv0: None,
      node_debug: None,
      node_ipc_fd: None,
      disable_deprecated_api_warning: false,
      verbose_deprecated_api_warning: false,
      future: false,
      mode: WorkerExecutionMode::None,
      serve_port: Default::default(),
      serve_host: Default::default(),
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
  // location
  Option<&'a str>,
  // unstable
  bool,
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
  // disable_deprecated_api_warning,
  bool,
  // verbose_deprecated_api_warning
  bool,
  // future
  bool,
  // mode
  i32,
  // serve port
  u16,
  // serve host
  Option<&'a str>,
);

impl BootstrapOptions {
  /// Return the v8 equivalent of this structure.
  pub fn as_v8<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::Local<'s, v8::Value> {
    let scope = RefCell::new(scope);
    let ser = deno_core::serde_v8::Serializer::new(&scope);

    let bootstrap = BootstrapV8(
      self.location.as_ref().map(|l| l.as_str()),
      self.unstable,
      self.unstable_features.as_ref(),
      self.inspect,
      self.enable_testing_features,
      self.has_node_modules_dir,
      self.argv0.as_deref(),
      self.node_debug.as_deref(),
      self.disable_deprecated_api_warning,
      self.verbose_deprecated_api_warning,
      self.future,
      self.mode as u8 as _,
      self.serve_port.unwrap_or_default(),
      self.serve_host.as_deref(),
    );

    bootstrap.serialize(ser).unwrap()
  }
}
