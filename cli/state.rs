// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::permission_denied;
use crate::global_state::ThreadSafeGlobalState;
use crate::global_timer::GlobalTimer;
use crate::import_map::ImportMap;
use crate::metrics::Metrics;
use crate::ops::JsonOp;
use crate::permissions::DenoPermissions;
use crate::worker::Worker;
use crate::worker::WorkerChannels;
use deno::Buf;
use deno::CoreOp;
use deno::ErrBox;
use deno::Loader;
use deno::ModuleSpecifier;
use deno::Op;
use deno::PinnedBuf;
use futures::Future;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::Value;
use std;
use std::collections::HashMap;
use std::ops::Deref;
use std::str;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::mpsc;

/// Isolate cannot be passed between threads but ThreadSafeState can.
/// ThreadSafeState satisfies Send and Sync. So any state that needs to be
/// accessed outside the main V8 thread should be inside ThreadSafeState.
pub struct ThreadSafeState(Arc<State>);

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct State {
  pub global_state: ThreadSafeGlobalState,
  pub modules: Arc<Mutex<deno::Modules>>,
  pub permissions: DenoPermissions,
  pub main_module: Option<ModuleSpecifier>,
  pub worker_channels: Mutex<WorkerChannels>,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub metrics: Metrics,
  pub global_timer: Mutex<GlobalTimer>,
  pub workers: Mutex<HashMap<u32, Worker>>,
  pub next_worker_id: AtomicUsize,
  pub start_time: Instant,
  pub seeded_rng: Option<Mutex<StdRng>>,
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
    let fut = self
      .global_state
      .fetch_compiled_module(module_specifier)
      .map(|compiled_module| deno::SourceCodeInfo {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      });

    Box::new(fut)
  }
}

impl ThreadSafeState {
  pub fn create_channels() -> (WorkerChannels, WorkerChannels) {
    let (in_tx, in_rx) = mpsc::channel::<Buf>(1);
    let (out_tx, out_rx) = mpsc::channel::<Buf>(1);
    let internal_channels = WorkerChannels {
      sender: out_tx,
      receiver: in_rx,
    };
    let external_channels = WorkerChannels {
      sender: in_tx,
      receiver: out_rx,
    };
    (internal_channels, external_channels)
  }

  pub fn new(
    global_state: ThreadSafeGlobalState,
    main_module: Option<ModuleSpecifier>,
    include_deno_namespace: bool,
    internal_channels: WorkerChannels,
  ) -> Result<Self, ErrBox> {
    let import_map: Option<ImportMap> =
      match global_state.flags.import_map_path.as_ref() {
        None => None,
        Some(file_path) => Some(ImportMap::load(file_path)?),
      };

    let seeded_rng = match global_state.flags.seed {
      Some(seed) => Some(Mutex::new(StdRng::seed_from_u64(seed))),
      None => None,
    };

    let modules = Arc::new(Mutex::new(deno::Modules::new()));
    let permissions = global_state.permissions.clone();

    let state = State {
      global_state,
      modules,
      main_module,
      permissions,
      import_map,
      worker_channels: Mutex::new(internal_channels),
      metrics: Metrics::default(),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(HashMap::new()),
      next_worker_id: AtomicUsize::new(0),
      start_time: Instant::now(),
      seeded_rng,
      include_deno_namespace,
    };

    Ok(ThreadSafeState(Arc::new(state)))
  }

  pub fn add_child_worker(&self, worker: Worker) -> u32 {
    let worker_id = self.next_worker_id.fetch_add(1, Ordering::Relaxed) as u32;
    let mut workers_tl = self.workers.lock().unwrap();
    workers_tl.insert(worker_id, worker);
    worker_id
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
  pub fn mock(
    argv: Vec<String>,
    internal_channels: WorkerChannels,
  ) -> ThreadSafeState {
    let module_specifier = if argv.is_empty() {
      None
    } else {
      let module_specifier = ModuleSpecifier::resolve_url_or_path(&argv[0])
        .expect("Invalid entry module");
      Some(module_specifier)
    };

    ThreadSafeState::new(
      ThreadSafeGlobalState::mock(argv),
      module_specifier,
      true,
      internal_channels,
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
  let (int, _) = ThreadSafeState::create_channels();
  f(ThreadSafeState::mock(
    vec![String::from("./deno"), String::from("hello.js")],
    int,
  ));
}
