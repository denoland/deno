// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compilers::CompiledModule;
use crate::compilers::JsCompiler;
use crate::compilers::JsonCompiler;
use crate::compilers::TsCompiler;
use crate::deno_dir;
use crate::deno_error::permission_denied;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::global_timer::GlobalTimer;
use crate::import_map::ImportMap;
use crate::msg;
use crate::ops::JsonOp;
use crate::permissions::DenoPermissions;
use crate::progress::Progress;
use crate::resources;
use crate::resources::ResourceId;
use crate::worker::Worker;
use deno::Buf;
use deno::CoreOp;
use deno::ErrBox;
use deno::Loader;
use deno::ModuleSpecifier;
use deno::Op;
use deno::PinnedBuf;
use futures::future::Shared;
use futures::Future;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::Value;
use std;
use std::collections::HashMap;
use std::env;
use std::ops::Deref;
use std::str;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::mpsc as async_mpsc;

pub type WorkerSender = async_mpsc::Sender<Buf>;
pub type WorkerReceiver = async_mpsc::Receiver<Buf>;
pub type WorkerChannels = (WorkerSender, WorkerReceiver);
pub type UserWorkerTable = HashMap<ResourceId, Shared<Worker>>;

#[derive(Default)]
pub struct Metrics {
  pub ops_dispatched: AtomicUsize,
  pub ops_completed: AtomicUsize,
  pub bytes_sent_control: AtomicUsize,
  pub bytes_sent_data: AtomicUsize,
  pub bytes_received: AtomicUsize,
  pub resolve_count: AtomicUsize,
  pub compiler_starts: AtomicUsize,
}

/// Isolate cannot be passed between threads but ThreadSafeState can.
/// ThreadSafeState satisfies Send and Sync. So any state that needs to be
/// accessed outside the main V8 thread should be inside ThreadSafeState.
pub struct ThreadSafeState(Arc<State>);

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct State {
  pub modules: Arc<Mutex<deno::Modules>>,
  pub main_module: Option<ModuleSpecifier>,
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub permissions: DenoPermissions,
  pub flags: flags::DenoFlags,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub metrics: Metrics,
  pub worker_channels: Mutex<WorkerChannels>,
  pub global_timer: Mutex<GlobalTimer>,
  pub workers: Mutex<UserWorkerTable>,
  pub start_time: Instant,
  /// A reference to this worker's resource.
  pub resource: resources::Resource,
  /// Reference to global progress bar.
  pub progress: Progress,
  pub seeded_rng: Option<Mutex<StdRng>>,

  pub file_fetcher: SourceFileFetcher,
  pub js_compiler: JsCompiler,
  pub json_compiler: JsonCompiler,
  pub ts_compiler: TsCompiler,

  pub include_deno_namespace: bool,
}

impl Clone for ThreadSafeState {
  fn clone(&self) -> Self {
    ThreadSafeState(self.0.clone())
  }
}

impl Deref for ThreadSafeState {
  type Target = Arc<State>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ThreadSafeState {
  /// Wrap core `OpDispatcher` to collect metrics.
  pub fn core_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&[u8], Option<PinnedBuf>) -> CoreOp
  where
    D: Fn(&[u8], Option<PinnedBuf>) -> CoreOp,
  {
    let state = self.clone();

    move |control: &[u8], zero_copy: Option<PinnedBuf>| -> CoreOp {
      let bytes_sent_control = control.len();
      let bytes_sent_zero_copy =
        zero_copy.as_ref().map(|b| b.len()).unwrap_or(0);

      let op = dispatcher(control, zero_copy);
      state.metrics_op_dispatched(bytes_sent_control, bytes_sent_zero_copy);

      match op {
        Op::Sync(buf) => {
          state.metrics_op_completed(buf.len());
          Op::Sync(buf)
        }
        Op::Async(fut) => {
          let state = state.clone();
          let result_fut = Box::new(fut.map(move |buf: Buf| {
            state.clone().metrics_op_completed(buf.len());
            buf
          }));
          Op::Async(result_fut)
        }
      }
    }
  }

  /// This is a special function that provides `state` argument to dispatcher.
  ///
  /// NOTE: This only works with JSON dispatcher.
  /// This is a band-aid for transition to `Isolate.register_op` API as most of our
  /// ops require `state` argument.
  pub fn stateful_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(Value, Option<PinnedBuf>) -> Result<JsonOp, ErrBox>
  where
    D: Fn(&ThreadSafeState, Value, Option<PinnedBuf>) -> Result<JsonOp, ErrBox>,
  {
    let state = self.clone();

    move |args: Value, zero_copy: Option<PinnedBuf>| -> Result<JsonOp, ErrBox> {
      dispatcher(&state, args, zero_copy)
    }
  }
}

impl Loader for ThreadSafeState {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
    is_dyn_import: bool,
  ) -> Result<ModuleSpecifier, ErrBox> {
    if !is_main {
      if let Some(import_map) = &self.import_map {
        let result = import_map.resolve(specifier, referrer)?;
        if let Some(r) = result {
          return Ok(r);
        }
      }
    }
    let module_specifier =
      ModuleSpecifier::resolve_import(specifier, referrer)?;

    if is_dyn_import {
      self.check_dyn_import(&module_specifier)?;
    }

    Ok(module_specifier)
  }

  /// Given an absolute url, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Box<deno::SourceCodeInfoFuture> {
    self.metrics.resolve_count.fetch_add(1, Ordering::SeqCst);
    let module_url_specified = module_specifier.to_string();
    Box::new(self.fetch_compiled_module(module_specifier).map(
      |compiled_module| deno::SourceCodeInfo {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      },
    ))
  }
}

impl ThreadSafeState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    progress: Progress,
    include_deno_namespace: bool,
  ) -> Result<Self, ErrBox> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();

    let (worker_in_tx, worker_in_rx) = async_mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = async_mpsc::channel::<Buf>(1);
    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);
    let resource = resources::add_worker(external_channels);

    let dir = deno_dir::DenoDir::new(custom_root)?;

    let file_fetcher = SourceFileFetcher::new(
      dir.deps_cache.clone(),
      progress.clone(),
      !flags.reload,
      flags.cache_blacklist.clone(),
      flags.no_fetch,
    )?;

    let ts_compiler = TsCompiler::new(
      file_fetcher.clone(),
      dir.gen_cache.clone(),
      !flags.reload,
      flags.config_path.clone(),
    )?;

    let main_module: Option<ModuleSpecifier> = if argv_rest.len() <= 1 {
      None
    } else {
      let root_specifier = argv_rest[1].clone();
      Some(ModuleSpecifier::resolve_url_or_path(&root_specifier)?)
    };

    let import_map: Option<ImportMap> = match &flags.import_map_path {
      None => None,
      Some(file_path) => Some(ImportMap::load(file_path)?),
    };

    let mut seeded_rng = None;
    if let Some(seed) = flags.seed {
      seeded_rng = Some(Mutex::new(StdRng::seed_from_u64(seed)));
    };

    let modules = Arc::new(Mutex::new(deno::Modules::new()));

    let state = State {
      main_module,
      modules,
      dir,
      argv: argv_rest,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      import_map,
      metrics: Metrics::default(),
      worker_channels: Mutex::new(internal_channels),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(UserWorkerTable::new()),
      start_time: Instant::now(),
      resource,
      progress,
      seeded_rng,
      file_fetcher,
      ts_compiler,
      js_compiler: JsCompiler {},
      json_compiler: JsonCompiler {},
      include_deno_namespace,
    };

    Ok(ThreadSafeState(Arc::new(state)))
  }

  pub fn fetch_compiled_module(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> impl Future<Item = CompiledModule, Error = ErrBox> {
    let state_ = self.clone();

    self
      .file_fetcher
      .fetch_source_file_async(&module_specifier)
      .and_then(move |out| match out.media_type {
        msg::MediaType::Unknown => {
          state_.js_compiler.compile_async(state_.clone(), &out)
        }
        msg::MediaType::Json => {
          state_.json_compiler.compile_async(state_.clone(), &out)
        }
        msg::MediaType::TypeScript
        | msg::MediaType::TSX
        | msg::MediaType::JSX => {
          state_.ts_compiler.compile_async(state_.clone(), &out)
        }
        msg::MediaType::JavaScript => {
          if state_.ts_compiler.compile_js {
            state_.ts_compiler.compile_async(state_.clone(), &out)
          } else {
            state_.js_compiler.compile_async(state_.clone(), &out)
          }
        }
      })
  }

  /// Read main module from argv
  pub fn main_module(&self) -> Option<ModuleSpecifier> {
    match &self.main_module {
      Some(module_specifier) => Some(module_specifier.clone()),
      None => None,
    }
  }

  #[inline]
  pub fn check_read(&self, filename: &str) -> Result<(), ErrBox> {
    self.permissions.check_read(filename)
  }

  #[inline]
  pub fn check_write(&self, filename: &str) -> Result<(), ErrBox> {
    self.permissions.check_write(filename)
  }

  #[inline]
  pub fn check_env(&self) -> Result<(), ErrBox> {
    self.permissions.check_env()
  }

  #[inline]
  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    self.permissions.check_net(hostname, port)
  }

  #[inline]
  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    self.permissions.check_net_url(url)
  }

  #[inline]
  pub fn check_run(&self) -> Result<(), ErrBox> {
    self.permissions.check_run()
  }

  pub fn check_dyn_import(
    self: &Self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    let u = module_specifier.as_url();
    match u.scheme() {
      "http" | "https" => {
        self.check_net_url(u)?;
        Ok(())
      }
      "file" => {
        let filename = u
          .to_file_path()
          .unwrap()
          .into_os_string()
          .into_string()
          .unwrap();
        self.check_read(&filename)?;
        Ok(())
      }
      _ => Err(permission_denied()),
    }
  }

  #[cfg(test)]
  pub fn mock(argv: Vec<String>) -> ThreadSafeState {
    ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      Progress::new(),
      true,
    )
    .unwrap()
  }

  pub fn metrics_op_dispatched(
    &self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.metrics.ops_dispatched.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_control
      .fetch_add(bytes_sent_control, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_data
      .fetch_add(bytes_sent_data, Ordering::SeqCst);
  }

  pub fn metrics_op_completed(&self, bytes_received: usize) {
    self.metrics.ops_completed.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_received
      .fetch_add(bytes_received, Ordering::SeqCst);
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(ThreadSafeState::mock(vec![
    String::from("./deno"),
    String::from("hello.js"),
  ]));
}

#[test]
fn import_map_given_for_repl() {
  let _result = ThreadSafeState::new(
    flags::DenoFlags {
      import_map_path: Some("import_map.json".to_string()),
      ..flags::DenoFlags::default()
    },
    vec![String::from("./deno")],
    Progress::new(),
    true,
  );
}
