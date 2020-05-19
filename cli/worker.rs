// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::inspector::DenoInspector;
use crate::ops;
use crate::state::State;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleId;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use futures::task::AtomicWaker;
use std::cell::RefMut;
use std::env;
use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

/// Events that are sent to host from child
/// worker.
pub enum WorkerEvent {
  Message(Buf),
  Error(ErrBox),
  TerminalError(ErrBox),
}

pub struct WorkerChannelsInternal {
  pub sender: mpsc::Sender<WorkerEvent>,
  pub receiver: mpsc::Receiver<Buf>,
}

#[derive(Clone)]
pub struct WorkerHandle {
  pub sender: mpsc::Sender<Buf>,
  pub receiver: Arc<AsyncMutex<mpsc::Receiver<WorkerEvent>>>,
}

impl WorkerHandle {
  /// Post message to worker as a host.
  pub fn post_message(&self, buf: Buf) -> Result<(), ErrBox> {
    let mut sender = self.sender.clone();
    sender.try_send(buf).map_err(ErrBox::from)
  }

  /// Get the event with lock.
  /// Return error if more than one listener tries to get event
  pub async fn get_event(&self) -> Result<Option<WorkerEvent>, ErrBox> {
    let mut receiver = self.receiver.try_lock()?;
    Ok(receiver.next().await)
  }
}

fn create_channels() -> (WorkerChannelsInternal, WorkerHandle) {
  let (in_tx, in_rx) = mpsc::channel::<Buf>(1);
  let (out_tx, out_rx) = mpsc::channel::<WorkerEvent>(1);
  let internal_channels = WorkerChannelsInternal {
    sender: out_tx,
    receiver: in_rx,
  };
  let external_channels = WorkerHandle {
    sender: in_tx,
    receiver: Arc::new(AsyncMutex::new(out_rx)),
  };
  (internal_channels, external_channels)
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
pub struct Worker {
  pub name: String,
  pub isolate: Box<deno_core::EsIsolate>,
  pub state: State,
  pub waker: AtomicWaker,
  pub(crate) internal_channels: WorkerChannelsInternal,
  external_channels: WorkerHandle,
}

impl Worker {
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let loader = Rc::new(state.clone());
    let mut isolate = deno_core::EsIsolate::new(loader, startup_data, false);

    state.maybe_init_inspector(&mut isolate);

    let global_state = state.borrow().global_state.clone();
    isolate.set_js_error_create_fn(move |core_js_error| {
      JSError::create(core_js_error, &global_state.ts_compiler)
    });

    let (internal_channels, external_channels) = create_channels();

    Self {
      name,
      isolate,
      state,
      waker: AtomicWaker::new(),
      internal_channels,
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
    self.isolate.execute(js_filename, js_source)
  }

  /// Loads and instantiates specified JavaScript module.
  pub async fn preload_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, ErrBox> {
    self.isolate.load_module(module_specifier, None).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    let id = self.preload_module(module_specifier).await?;
    self.wait_for_inspector_session();
    self.isolate.mod_evaluate(id)
  }

  /// Loads, instantiates and executes provided source code
  /// as module.
  pub async fn execute_module_from_code(
    &mut self,
    module_specifier: &ModuleSpecifier,
    code: String,
  ) -> Result<(), ErrBox> {
    let id = self
      .isolate
      .load_module(module_specifier, Some(code))
      .await?;
    self.wait_for_inspector_session();
    self.isolate.mod_evaluate(id)
  }

  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WorkerHandle {
    self.external_channels.clone()
  }

  #[inline(always)]
  fn inspector(&self) -> RefMut<Option<Box<DenoInspector>>> {
    let state = self.state.borrow_mut();
    RefMut::map(state, |s| &mut s.inspector)
  }

  fn wait_for_inspector_session(&self) {
    if self.state.should_inspector_break_on_first_statement() {
      self
        .inspector()
        .as_mut()
        .unwrap()
        .wait_for_session_and_break_on_next_statement()
    }
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    // The Isolate object must outlive the Inspector object, but this is
    // currently not enforced by the type system.
    self.inspector().take();
  }
}

impl Future for Worker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();

    // We always poll the inspector if it exists.
    let _ = inner.inspector().as_mut().map(|i| i.poll_unpin(cx));
    inner.waker.register(cx.waker());
    inner.isolate.poll_unpin(cx)
  }
}

impl Deref for Worker {
  type Target = deno_core::EsIsolate;
  fn deref(&self) -> &Self::Target {
    &self.isolate
  }
}

impl DerefMut for Worker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.isolate
  }
}

/// This worker is created and used by Deno executable.
///
/// It provides ops available in the `Deno` namespace.
///
/// All WebWorkers created during program execution are descendants of
/// this worker.
pub struct MainWorker(Worker);

impl MainWorker {
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_);
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::runtime_compiler::init(isolate, &state);
      ops::errors::init(isolate, &state);
      ops::fetch::init(isolate, &state);
      ops::fs::init(isolate, &state);
      ops::fs_events::init(isolate, &state);
      ops::io::init(isolate, &state);
      ops::plugin::init(isolate, &state);
      ops::net::init(isolate, &state);
      ops::tls::init(isolate, &state);
      ops::os::init(isolate, &state);
      ops::permissions::init(isolate, &state);
      ops::process::init(isolate, &state);
      ops::random::init(isolate, &state);
      ops::repl::init(isolate, &state);
      ops::resources::init(isolate, &state);
      ops::signal::init(isolate, &state);
      ops::timers::init(isolate, &state);
      ops::tty::init(isolate, &state);
      ops::worker_host::init(isolate, &state);
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags;
  use crate::global_state::GlobalState;
  use crate::startup_data;
  use crate::state::State;
  use crate::tokio_util;
  use std::sync::atomic::Ordering;

  #[test]
  fn execute_mod_esm_imports_a() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/esm_imports_a.js");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let global_state = GlobalState::new(flags::Flags::default()).unwrap();
    let state =
      State::new(global_state, None, module_specifier.clone(), false).unwrap();
    let state_ = state.clone();
    tokio_util::run_basic(async move {
      let mut worker =
        MainWorker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_module(&module_specifier).await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      if let Err(e) = (&mut *worker).await {
        panic!("Future got unexpected error: {:?}", e);
      }
    });
    let state = state_.borrow();
    assert_eq!(state.metrics.resolve_count, 2);
    // Check that we didn't start the compiler.
    assert_eq!(state.global_state.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let global_state = GlobalState::new(flags::Flags::default()).unwrap();
    let state =
      State::new(global_state, None, module_specifier.clone(), false).unwrap();
    let state_ = state.clone();
    tokio_util::run_basic(async move {
      let mut worker =
        MainWorker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_module(&module_specifier).await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      if let Err(e) = (&mut *worker).await {
        panic!("Future got unexpected error: {:?}", e);
      }
    });

    let state = state_.borrow();
    assert_eq!(state.metrics.resolve_count, 1);
    // Check that we didn't start the compiler.
    assert_eq!(state.global_state.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[tokio::test]
  async fn execute_006_url_imports() {
    let http_server_guard = crate::test_util::http_server();
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/006_url_imports.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let flags = flags::Flags {
      subcommand: flags::DenoSubcommand::Run {
        script: module_specifier.to_string(),
      },
      reload: true,
      ..flags::Flags::default()
    };
    let global_state = GlobalState::new(flags).unwrap();
    let state =
      State::new(global_state.clone(), None, module_specifier.clone(), false)
        .unwrap();
    let mut worker = MainWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      state.clone(),
    );
    worker.execute("bootstrap.mainRuntime()").unwrap();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = (&mut *worker).await {
      panic!("Future got unexpected error: {:?}", e);
    }
    let state = state.borrow();
    assert_eq!(state.metrics.resolve_count, 3);
    // Check that we've only invoked the compiler once.
    assert_eq!(state.global_state.compiler_starts.load(Ordering::SeqCst), 1);
    drop(http_server_guard);
  }

  fn create_test_worker() -> MainWorker {
    let state = State::mock("./hello.js");
    let mut worker = MainWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      state,
    );
    worker.execute("bootstrap.mainRuntime()").unwrap();
    worker
  }

  #[tokio::test]
  async fn execute_mod_resolve_error() {
    // "foo" is not a valid module specifier so this should return an error.
    let mut worker = create_test_worker();
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
    let result = worker.execute_module(&module_specifier).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let mut worker = create_test_worker();
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/002_hello.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let result = worker.execute_module(&module_specifier).await;
    assert!(result.is_ok());
  }
}
