// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::compilers::TargetLib;
use crate::deno_error::permission_denied;
use crate::global_state::ThreadSafeGlobalState;
use crate::global_timer::GlobalTimer;
use crate::import_map::ImportMap;
use crate::metrics::Metrics;
use crate::ops::JsonOp;
use crate::ops::MinimalOp;
use crate::permissions::DenoPermissions;
use crate::web_worker::WebWorker;
use crate::worker::WorkerChannels;
use deno_core::Buf;
use deno_core::CoreOp;
use deno_core::ErrBox;
use deno_core::Loader;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::Value;
use std;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::pin::Pin;
use std::str;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Instant;
use tokio::sync::Mutex as AsyncMutex;

/// Isolate cannot be passed between threads but ThreadSafeState can.
/// ThreadSafeState satisfies Send and Sync. So any state that needs to be
/// accessed outside the main V8 thread should be inside ThreadSafeState.
pub struct ThreadSafeState(Arc<State>);

#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct State {
  pub global_state: ThreadSafeGlobalState,
  pub permissions: Arc<Mutex<DenoPermissions>>,
  pub main_module: ModuleSpecifier,
  // TODO(ry) rename to worker_channels_internal
  pub worker_channels: WorkerChannels,
  /// When flags contains a `.import_map_path` option, the content of the
  /// import map file will be resolved and set.
  pub import_map: Option<ImportMap>,
  pub metrics: Metrics,
  pub global_timer: Mutex<GlobalTimer>,
  pub workers: Mutex<HashMap<u32, WorkerChannels>>,
  pub loading_workers: Mutex<HashMap<u32, mpsc::Receiver<Result<(), ErrBox>>>>,
  pub next_worker_id: AtomicUsize,
  pub start_time: Instant,
  pub seeded_rng: Option<Mutex<StdRng>>,
  pub resource_table: Mutex<ResourceTable>,
  pub target_lib: TargetLib,
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
  pub fn lock_resource_table(&self) -> MutexGuard<ResourceTable> {
    self.resource_table.lock().unwrap()
  }

  /// Wrap core `OpDispatcher` to collect metrics.
  pub fn core_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp
  where
    D: Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp,
  {
    let state = self.clone();

    move |control: &[u8], zero_copy: Option<ZeroCopyBuf>| -> CoreOp {
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
          let result_fut = fut.map_ok(move |buf: Buf| {
            state.metrics_op_completed(buf.len());
            buf
          });
          Op::Async(result_fut.boxed_local())
        }
        Op::AsyncUnref(fut) => {
          let state = state.clone();
          let result_fut = fut.map_ok(move |buf: Buf| {
            state.metrics_op_completed(buf.len());
            buf
          });
          Op::AsyncUnref(result_fut.boxed_local())
        }
      }
    }
  }

  /// This is a special function that provides `state` argument to dispatcher.
  pub fn stateful_minimal_op<D>(
    &self,
    dispatcher: D,
  ) -> impl Fn(i32, Option<ZeroCopyBuf>) -> Pin<Box<MinimalOp>>
  where
    D: Fn(&ThreadSafeState, i32, Option<ZeroCopyBuf>) -> Pin<Box<MinimalOp>>,
  {
    let state = self.clone();

    move |rid: i32, zero_copy: Option<ZeroCopyBuf>| -> Pin<Box<MinimalOp>> {
      dispatcher(&state, rid, zero_copy)
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
  ) -> impl Fn(Value, Option<ZeroCopyBuf>) -> Result<JsonOp, ErrBox>
  where
    D: Fn(
      &ThreadSafeState,
      Value,
      Option<ZeroCopyBuf>,
    ) -> Result<JsonOp, ErrBox>,
  {
    let state = self.clone();

    move |args: Value,
          zero_copy: Option<ZeroCopyBuf>|
          -> Result<JsonOp, ErrBox> { dispatcher(&state, args, zero_copy) }
  }
}

impl Loader for ThreadSafeState {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
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

    Ok(module_specifier)
  }

  /// Given an absolute url, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    is_dyn_import: bool,
  ) -> Pin<Box<deno_core::SourceCodeInfoFuture>> {
    let module_specifier = module_specifier.clone();
    if is_dyn_import {
      if let Err(e) = self.check_dyn_import(&module_specifier) {
        return async move { Err(e) }.boxed_local();
      }
    }

    // TODO(bartlomieju): incrementing resolve_count here has no sense...
    self.metrics.resolve_count.fetch_add(1, Ordering::SeqCst);
    let module_url_specified = module_specifier.to_string();
    let global_state = self.global_state.clone();
    let target_lib = self.target_lib.clone();
    let fut = async move {
      let compiled_module = global_state
        .fetch_compiled_module(module_specifier, maybe_referrer, target_lib)
        .await?;
      Ok(deno_core::SourceCodeInfo {
        // Real module name, might be different from initial specifier
        // due to redirections.
        code: compiled_module.code,
        module_url_specified,
        module_url_found: compiled_module.name,
      })
    };

    fut.boxed_local()
  }
}

impl ThreadSafeState {
  pub fn create_channels() -> (WorkerChannels, WorkerChannels) {
    let (in_tx, in_rx) = mpsc::channel::<Buf>(1);
    let (out_tx, out_rx) = mpsc::channel::<Buf>(1);
    let internal_channels = WorkerChannels {
      sender: out_tx,
      receiver: Arc::new(AsyncMutex::new(in_rx)),
    };
    let external_channels = WorkerChannels {
      sender: in_tx,
      receiver: Arc::new(AsyncMutex::new(out_rx)),
    };
    (internal_channels, external_channels)
  }

  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new(
    global_state: ThreadSafeGlobalState,
    shared_permissions: Option<Arc<Mutex<DenoPermissions>>>,
    main_module: ModuleSpecifier,
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

    let permissions = if let Some(perm) = shared_permissions {
      perm
    } else {
      Arc::new(Mutex::new(global_state.permissions.clone()))
    };

    let state = State {
      global_state,
      main_module,
      permissions,
      import_map,
      worker_channels: internal_channels,
      metrics: Metrics::default(),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(HashMap::new()),
      loading_workers: Mutex::new(HashMap::new()),
      next_worker_id: AtomicUsize::new(0),
      start_time: Instant::now(),
      seeded_rng,

      resource_table: Mutex::new(ResourceTable::default()),
      target_lib: TargetLib::Main,
    };

    Ok(ThreadSafeState(Arc::new(state)))
  }

  /// If `shared_permission` is None then permissions from globa state are used.
  pub fn new_for_worker(
    global_state: ThreadSafeGlobalState,
    shared_permissions: Option<Arc<Mutex<DenoPermissions>>>,
    main_module: ModuleSpecifier,
    internal_channels: WorkerChannels,
  ) -> Result<Self, ErrBox> {
    let seeded_rng = match global_state.flags.seed {
      Some(seed) => Some(Mutex::new(StdRng::seed_from_u64(seed))),
      None => None,
    };

    let permissions = if let Some(perm) = shared_permissions {
      perm
    } else {
      Arc::new(Mutex::new(global_state.permissions.clone()))
    };

    let state = State {
      global_state,
      main_module,
      permissions,
      import_map: None,
      worker_channels: internal_channels,
      metrics: Metrics::default(),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(HashMap::new()),
      loading_workers: Mutex::new(HashMap::new()),
      next_worker_id: AtomicUsize::new(0),
      start_time: Instant::now(),
      seeded_rng,

      resource_table: Mutex::new(ResourceTable::default()),
      target_lib: TargetLib::Worker,
    };

    Ok(ThreadSafeState(Arc::new(state)))
  }

  pub fn add_child_worker(&self, worker: &WebWorker) -> u32 {
    let worker_id = self.next_worker_id.fetch_add(1, Ordering::Relaxed) as u32;
    let handle = worker.thread_safe_handle();
    let mut workers_tl = self.workers.lock().unwrap();
    workers_tl.insert(worker_id, handle);
    worker_id
  }

  #[inline]
  pub fn check_read(&self, path: &Path) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_read(path)
  }

  #[inline]
  pub fn check_write(&self, path: &Path) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_write(path)
  }

  #[inline]
  pub fn check_env(&self) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_env()
  }

  #[inline]
  pub fn check_net(&self, hostname: &str, port: u16) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_net(hostname, port)
  }

  #[inline]
  pub fn check_net_url(&self, url: &url::Url) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_net_url(url)
  }

  #[inline]
  pub fn check_run(&self) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_run()
  }

  #[inline]
  pub fn check_plugin(&self, filename: &Path) -> Result<(), ErrBox> {
    self.permissions.lock().unwrap().check_plugin(filename)
  }

  pub fn check_dyn_import(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    let u = module_specifier.as_url();
    match u.scheme() {
      "http" | "https" => {
        self.check_net_url(u)?;
        Ok(())
      }
      "file" => {
        let path = u
          .to_file_path()
          .unwrap()
          .into_os_string()
          .into_string()
          .unwrap();
        self.check_read(Path::new(&path))?;
        Ok(())
      }
      _ => Err(permission_denied()),
    }
  }

  #[cfg(test)]
  pub fn mock(
    main_module: &str,
    internal_channels: WorkerChannels,
  ) -> ThreadSafeState {
    let module_specifier = ModuleSpecifier::resolve_url_or_path(main_module)
      .expect("Invalid entry module");
    ThreadSafeState::new(
      ThreadSafeGlobalState::mock(vec!["deno".to_string()]),
      None,
      module_specifier,
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
  f(ThreadSafeState::mock("./hello.js", int));
}
