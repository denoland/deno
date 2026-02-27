// Copyright 2018-2025 the Deno authors. MIT license.

use self::ops_worker::WorkerCloseWatcher;
use self::ops_worker::WorkerHostSide;
use self::ops_worker::worker_create;
use self::ts_module_loader::maybe_transpile_source;
use deno_core::CrossIsolateStore;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::channel;
use std::time::Duration;

mod extensions;
mod ops;
mod ops_async;
mod ops_buffer;
mod ops_error;
mod ops_io;
mod ops_worker;
pub mod snapshot;
#[cfg(test)]
pub mod testing;
mod ts_module_loader;

#[derive(Clone, Default)]
pub struct Output {
  pub lines: Arc<Mutex<Vec<String>>>,
}
impl Output {
  pub fn line(&self, line: String) {
    self.lines.lock().unwrap().push(line)
  }

  #[cfg(test)]
  pub fn take(&self) -> Vec<String> {
    std::mem::take(&mut self.lines.lock().unwrap())
  }
}

#[derive(Default)]
pub struct TestData {
  pub data: HashMap<(String, TypeId), Box<dyn Any>>,
}

impl TestData {
  pub fn insert<T: 'static + Any>(&mut self, name: String, data: T) {
    self.data.insert((name, TypeId::of::<T>()), Box::new(data));
  }

  pub fn get<T: 'static + Any>(&self, name: String) -> &T {
    let key = (name, TypeId::of::<T>());
    self
      .data
      .get(&key)
      .unwrap_or_else(|| {
        panic!(
          "Unable to locate '{}' of type {}",
          key.0,
          std::any::type_name::<T>()
        )
      })
      .downcast_ref()
      .unwrap()
  }

  pub fn take<T: 'static + Any>(&mut self, name: String) -> T {
    let key = (name, TypeId::of::<T>());
    let Some(res) = self.data.remove(&key) else {
      panic!(
        "Failed to remove '{}' of type {}",
        key.0,
        std::any::type_name::<T>()
      );
    };
    *res.downcast().unwrap()
  }
}

pub fn create_runtime_from_snapshot(
  snapshot: &'static [u8],
  inspector: bool,
  parent: Option<WorkerCloseWatcher>,
  additional_extensions: Vec<Extension>,
) -> (JsRuntime, WorkerHostSide) {
  create_runtime_from_snapshot_with_options(
    snapshot,
    inspector,
    parent,
    additional_extensions,
    RuntimeOptions::default(),
  )
}

pub struct Snapshot(&'static [u8]);

pub fn create_runtime_from_snapshot_with_options(
  snapshot: &'static [u8],
  inspector: bool,
  parent: Option<WorkerCloseWatcher>,
  additional_extensions: Vec<Extension>,
  options: RuntimeOptions,
) -> (JsRuntime, WorkerHostSide) {
  let (worker, worker_host_side) = worker_create(parent);

  let mut extensions = vec![extensions::checkin_runtime::init::<()>()];
  extensions.extend(additional_extensions);
  let module_loader =
    Rc::new(ts_module_loader::TypescriptModuleLoader::default());
  let runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    startup_snapshot: Some(snapshot),
    module_loader: Some(module_loader.clone()),
    extension_transpiler: Some(Rc::new(|specifier, source| {
      maybe_transpile_source(specifier, source)
    })),
    shared_array_buffer_store: Some(CrossIsolateStore::default()),
    inspector,
    ..options
  });

  let stats = runtime.runtime_activity_stats_factory();
  runtime.op_state().borrow_mut().put(stats);
  runtime.op_state().borrow_mut().put(worker);
  runtime.op_state().borrow_mut().put(Snapshot(snapshot));

  (runtime, worker_host_side)
}

fn run_async(f: impl Future<Output = Result<(), anyhow::Error>>) {
  let tokio = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("Failed to build a runtime");
  tokio.block_on(f).expect("Failed to run the given task");

  // We don't have a good way to wait for tokio to go idle here, but we'd like tokio
  // to poll any remaining tasks to shake out any errors.
  let handle = tokio.spawn(async {
    tokio::task::yield_now().await;
  });
  _ = tokio.block_on(handle);

  let (tx, rx) = channel::<()>();
  let timeout = std::thread::spawn(move || {
    if rx.recv_timeout(Duration::from_secs(10))
      == Err(RecvTimeoutError::Timeout)
    {
      panic!("Failed to shut down the runtime in time");
    }
  });
  drop(tokio);
  drop(tx);
  _ = timeout.join();
}
