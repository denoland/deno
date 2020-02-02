// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::ops;
use crate::state::ThreadSafeState;
use deno_core;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::task::AtomicWaker;
use std::env;
use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

/// Wraps mpsc channels so they can be referenced
/// from ops and used to facilitate parent-child communication
/// for workers.
#[derive(Clone)]
pub struct WorkerChannels {
  pub sender: mpsc::Sender<Buf>,
  pub receiver: Arc<AsyncMutex<mpsc::Receiver<Buf>>>,
}

/// Worker is a CLI wrapper for `deno_core::Isolate`.
///
/// It provides infrastructure to communicate with a worker and
/// consequently between workers.
///
/// This struct is meant to be used as a base struct for concrete
/// type of worker that registers set of ops.
///
/// Currently there are three types of workers:
///  - `MainWorker`
///  - `CompilerWorker`
///  - `WebWorker`
#[derive(Clone)]
pub struct Worker {
  pub name: String,
  pub isolate: Arc<AsyncMutex<Box<deno_core::EsIsolate>>>,
  pub state: ThreadSafeState,
  external_channels: WorkerChannels,
}

impl Worker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
    external_channels: WorkerChannels,
  ) -> Self {
    let mut isolate =
      deno_core::EsIsolate::new(Box::new(state.clone()), startup_data, false);

    let global_state_ = state.global_state.clone();
    isolate.set_js_error_create(move |v8_exception| {
      JSError::from_v8_exception(v8_exception, &global_state_.ts_compiler)
    });

    Self {
      name,
      isolate: Arc::new(AsyncMutex::new(isolate)),
      state,
      external_channels,
    }
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), ErrBox> {
    let path = env::current_dir().unwrap().join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.execute2(url.as_str(), js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    let mut isolate = self.isolate.try_lock().unwrap();
    isolate.execute(js_filename, js_source)
  }

  /// Executes the provided JavaScript module.
  ///
  /// Takes ownership of the isolate behind mutex.
  pub async fn execute_mod_async(
    &mut self,
    module_specifier: &ModuleSpecifier,
    maybe_code: Option<String>,
    is_prefetch: bool,
  ) -> Result<(), ErrBox> {
    let specifier = module_specifier.to_string();
    let worker = self.clone();

    let mut isolate = self.isolate.lock().await;
    let id = isolate.load_module(&specifier, maybe_code).await?;
    worker.state.global_state.progress.done();

    if !is_prefetch {
      return isolate.mod_evaluate(id);
    }

    Ok(())
  }

  /// Post message to worker as a host.
  ///
  /// This method blocks current thread.
  pub async fn post_message(&self, buf: Buf) -> Result<(), ErrBox> {
    let mut sender = self.external_channels.sender.clone();
    let result = sender.send(buf).map_err(ErrBox::from).await;
    drop(sender);
    result
  }

  /// Get message from worker as a host.
  pub fn get_message(
    &self,
  ) -> Pin<Box<dyn Future<Output = Option<Buf>> + Send>> {
    let receiver_mutex = self.external_channels.receiver.clone();

    async move {
      let mut receiver = receiver_mutex.lock().await;
      receiver.next().await
    }
    .boxed()
  }
}

impl Future for Worker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let waker = AtomicWaker::new();
    waker.register(cx.waker());
    match inner.isolate.try_lock() {
      Ok(mut isolate) => isolate.poll_unpin(cx),
      Err(_) => {
        waker.wake();
        Poll::Pending
      }
    }
  }
}

/// This worker is created and used by Deno executable.
///
/// It provides ops available in the `Deno` namespace.
///
/// All WebWorkers created during program execution are decendants of
/// this worker.
#[derive(Clone)]
pub struct MainWorker(Worker);

impl MainWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
    external_channels: WorkerChannels,
  ) -> Self {
    let state_ = state.clone();
    let worker = Worker::new(name, startup_data, state_, external_channels);
    {
      let mut isolate = worker.isolate.try_lock().unwrap();
      let op_registry = isolate.op_registry.clone();

      ops::runtime::init(&mut isolate, &state);
      ops::runtime_compiler::init(&mut isolate, &state);
      ops::errors::init(&mut isolate, &state);
      ops::fetch::init(&mut isolate, &state);
      ops::files::init(&mut isolate, &state);
      ops::fs::init(&mut isolate, &state);
      ops::io::init(&mut isolate, &state);
      ops::plugins::init(&mut isolate, &state, op_registry);
      ops::net::init(&mut isolate, &state);
      ops::tls::init(&mut isolate, &state);
      ops::os::init(&mut isolate, &state);
      ops::permissions::init(&mut isolate, &state);
      ops::process::init(&mut isolate, &state);
      ops::random::init(&mut isolate, &state);
      ops::repl::init(&mut isolate, &state);
      ops::resources::init(&mut isolate, &state);
      ops::signal::init(&mut isolate, &state);
      ops::timers::init(&mut isolate, &state);
      ops::worker_host::init(&mut isolate, &state);
      ops::web_worker::init(&mut isolate, &state);
    }

    Self(worker)
  }
}

impl Deref for MainWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for MainWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Future for MainWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.0.poll_unpin(cx)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags;
  use crate::global_state::ThreadSafeGlobalState;
  use crate::progress::Progress;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use futures::executor::block_on;
  use std::sync::atomic::Ordering;

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce() + Send + 'static,
  {
    let fut = futures::future::lazy(move |_cx| f());
    tokio_util::run(fut)
  }

  pub async fn panic_on_error<I, E, F>(f: F) -> I
  where
    F: Future<Output = Result<I, E>>,
    E: std::fmt::Debug,
  {
    match f.await {
      Ok(v) => v,
      Err(e) => panic!("Future got unexpected error: {:?}", e),
    }
  }

  #[test]
  fn execute_mod_esm_imports_a() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/esm_imports_a.js");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let global_state = ThreadSafeGlobalState::new(
      flags::DenoFlags {
        argv: vec![String::from("./deno"), module_specifier.to_string()],
        ..flags::DenoFlags::default()
      },
      Progress::new(),
    )
    .unwrap();
    let (int, ext) = ThreadSafeState::create_channels();
    let state = ThreadSafeState::new(
      global_state,
      None,
      Some(module_specifier.clone()),
      int,
    )
    .unwrap();
    let state_ = state.clone();
    tokio_util::run(async move {
      let mut worker =
        MainWorker::new("TEST".to_string(), StartupData::None, state, ext);
      let result = worker
        .execute_mod_async(&module_specifier, None, false)
        .await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      panic_on_error(worker).await
    });

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let global_state = ThreadSafeGlobalState::new(
      flags::DenoFlags {
        argv: vec![String::from("deno"), module_specifier.to_string()],
        ..flags::DenoFlags::default()
      },
      Progress::new(),
    )
    .unwrap();
    let (int, ext) = ThreadSafeState::create_channels();
    let state = ThreadSafeState::new(
      global_state,
      None,
      Some(module_specifier.clone()),
      int,
    )
    .unwrap();
    let state_ = state.clone();
    tokio_util::run(async move {
      let mut worker =
        MainWorker::new("TEST".to_string(), StartupData::None, state, ext);
      let result = worker
        .execute_mod_async(&module_specifier, None, false)
        .await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      panic_on_error(worker).await
    });

    let metrics = &state_.metrics;
    // TODO  assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_006_url_imports() {
    let http_server_guard = crate::test_util::http_server();

    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/006_url_imports.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut flags = flags::DenoFlags::default();
    flags.argv = vec![String::from("deno"), module_specifier.to_string()];
    flags.reload = true;
    let global_state =
      ThreadSafeGlobalState::new(flags, Progress::new()).unwrap();
    let (int, ext) = ThreadSafeState::create_channels();
    let state = ThreadSafeState::new(
      global_state.clone(),
      None,
      Some(module_specifier.clone()),
      int,
    )
    .unwrap();
    let global_state_ = global_state;
    let state_ = state.clone();
    tokio_util::run(async move {
      let mut worker = MainWorker::new(
        "TEST".to_string(),
        startup_data::deno_isolate_init(),
        state,
        ext,
      );

      worker.execute("bootstrapMainRuntime()").unwrap();
      let result = worker
        .execute_mod_async(&module_specifier, None, false)
        .await;

      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      panic_on_error(worker).await
    });

    assert_eq!(state_.metrics.resolve_count.load(Ordering::SeqCst), 3);
    // Check that we've only invoked the compiler once.
    assert_eq!(
      global_state_.metrics.compiler_starts.load(Ordering::SeqCst),
      1
    );
    drop(http_server_guard);
  }

  fn create_test_worker() -> MainWorker {
    let (int, ext) = ThreadSafeState::create_channels();
    let state = ThreadSafeState::mock(
      vec![String::from("./deno"), String::from("hello.js")],
      int,
    );
    let mut worker = MainWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      state,
      ext,
    );
    worker.execute("bootstrapMainRuntime()").unwrap();
    worker
  }

  #[test]
  fn execute_mod_resolve_error() {
    run_in_task(|| {
      // "foo" is not a valid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
      let result =
        block_on(worker.execute_mod_async(&module_specifier, None, false));
      assert!(result.is_err());
    })
  }

  #[test]
  fn execute_mod_002_hello() {
    run_in_task(|| {
      // This assumes cwd is project root (an assumption made throughout the
      // tests).
      let mut worker = create_test_worker();
      let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cli/tests/002_hello.ts");
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      let result =
        block_on(worker.execute_mod_async(&module_specifier, None, false));
      assert!(result.is_ok());
    })
  }
}
