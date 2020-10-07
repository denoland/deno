// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fmt_errors::JsError;
use crate::global_state::GlobalState;
use crate::inspector::DenoInspector;
use crate::js;
use crate::metrics::Metrics;
use crate::ops;
use crate::ops::io::get_stdio;
use crate::permissions::Permissions;
use crate::state::CliModuleLoader;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::FutureExt;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::AtomicWaker;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use std::env;
use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;

/// Events that are sent to host from child
/// worker.
pub enum WorkerEvent {
  Message(Box<[u8]>),
  Error(AnyError),
  TerminalError(AnyError),
}

pub struct WorkerChannelsInternal {
  pub sender: mpsc::Sender<WorkerEvent>,
  pub receiver: mpsc::Receiver<Box<[u8]>>,
}

#[derive(Clone)]
pub struct WorkerHandle {
  pub sender: mpsc::Sender<Box<[u8]>>,
  pub receiver: Arc<AsyncMutex<mpsc::Receiver<WorkerEvent>>>,
}

impl WorkerHandle {
  /// Post message to worker as a host.
  pub fn post_message(&self, buf: Box<[u8]>) -> Result<(), AnyError> {
    let mut sender = self.sender.clone();
    sender.try_send(buf)?;
    Ok(())
  }

  /// Get the event with lock.
  /// Return error if more than one listener tries to get event
  pub async fn get_event(&self) -> Result<Option<WorkerEvent>, AnyError> {
    let mut receiver = self.receiver.try_lock()?;
    Ok(receiver.next().await)
  }
}

fn create_channels() -> (WorkerChannelsInternal, WorkerHandle) {
  let (in_tx, in_rx) = mpsc::channel::<Box<[u8]>>(1);
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
/// Currently there are two types of workers:
///  - `MainWorker`
///  - `WebWorker`
pub struct Worker {
  pub name: String,
  pub isolate: JsRuntime,
  pub inspector: Option<Box<DenoInspector>>,
  pub waker: AtomicWaker,
  pub(crate) internal_channels: WorkerChannelsInternal,
  external_channels: WorkerHandle,
  should_break_on_first_statement: bool,
}

impl Worker {
  pub fn new(
    name: String,
    startup_snapshot: Snapshot,
    global_state: Arc<GlobalState>,
    module_loader: Rc<CliModuleLoader>,
    is_main: bool,
  ) -> Self {
    let global_state_ = global_state.clone();

    let mut isolate = JsRuntime::new(RuntimeOptions {
      module_loader: Some(module_loader),
      startup_snapshot: Some(startup_snapshot),
      js_error_create_fn: Some(Box::new(move |core_js_error| {
        JsError::create(core_js_error, &global_state_.ts_compiler)
      })),
      ..Default::default()
    });
    {
      let op_state = isolate.op_state();
      let mut op_state = op_state.borrow_mut();
      op_state.get_error_class_fn = &crate::errors::get_error_class_name;
    }

    let inspector =
      if let Some(inspector_server) = &global_state.maybe_inspector_server {
        Some(DenoInspector::new(
          &mut isolate,
          Some(inspector_server.clone()),
        ))
      } else if global_state.flags.coverage || global_state.flags.repl {
        Some(DenoInspector::new(&mut isolate, None))
      } else {
        None
      };

    let should_break_on_first_statement = inspector.is_some()
      && is_main
      && global_state.flags.inspect_brk.is_some();

    let (internal_channels, external_channels) = create_channels();

    Self {
      name,
      isolate,
      inspector,
      waker: AtomicWaker::new(),
      internal_channels,
      external_channels,
      should_break_on_first_statement,
    }
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), AnyError> {
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
  ) -> Result<(), AnyError> {
    self.isolate.execute(js_filename, js_source)
  }

  /// Loads and instantiates specified JavaScript module.
  pub async fn preload_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, AnyError> {
    self.isolate.load_module(module_specifier, None).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_module(module_specifier).await?;
    self.wait_for_inspector_session();
    self.isolate.mod_evaluate(id).await
  }

  /// Loads, instantiates and executes provided source code
  /// as module.
  pub async fn execute_module_from_code(
    &mut self,
    module_specifier: &ModuleSpecifier,
    code: String,
  ) -> Result<(), AnyError> {
    let id = self
      .isolate
      .load_module(module_specifier, Some(code))
      .await?;
    self.wait_for_inspector_session();
    self.isolate.mod_evaluate(id).await
  }

  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WorkerHandle {
    self.external_channels.clone()
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .inspector
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
    self.inspector.take();
  }
}

impl Future for Worker {
  type Output = Result<(), AnyError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();

    // We always poll the inspector if it exists.
    let _ = inner.inspector.as_mut().map(|i| i.poll_unpin(cx));
    inner.waker.register(cx.waker());
    inner.isolate.poll_unpin(cx)
  }
}

impl Deref for Worker {
  type Target = JsRuntime;
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
  pub fn new(
    global_state: &Arc<GlobalState>,
    main_module: ModuleSpecifier,
  ) -> Self {
    let loader = CliModuleLoader::new(global_state.maybe_import_map.clone());
    let mut worker = Worker::new(
      "main".to_string(),
      js::deno_isolate_init(),
      global_state.clone(),
      loader,
      true,
    );
    {
      // All ops registered in this function depend on these
      {
        let op_state = worker.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<GlobalState>>(global_state.clone());
        op_state.put::<Permissions>(global_state.permissions.clone());
      }

      ops::runtime::init(&mut worker, main_module);
      ops::fetch::init(&mut worker, global_state.flags.ca_file.as_deref());
      ops::timers::init(&mut worker);
      ops::worker_host::init(&mut worker);
      ops::random::init(&mut worker, global_state.flags.seed);
      ops::reg_json_sync(&mut worker, "op_close", deno_core::op_close);
      ops::reg_json_sync(&mut worker, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        &mut worker,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(&mut worker);
      ops::fs_events::init(&mut worker);
      ops::fs::init(&mut worker);
      ops::io::init(&mut worker);
      ops::net::init(&mut worker);
      ops::os::init(&mut worker);
      ops::permissions::init(&mut worker);
      ops::plugin::init(&mut worker);
      ops::process::init(&mut worker);
      ops::runtime_compiler::init(&mut worker);
      ops::signal::init(&mut worker);
      ops::tls::init(&mut worker);
      ops::tty::init(&mut worker);
      ops::websocket::init(&mut worker);
    }
    {
      let op_state = worker.op_state();
      let mut op_state = op_state.borrow_mut();
      let t = &mut op_state.resource_table;
      let (stdin, stdout, stderr) = get_stdio();
      if let Some(stream) = stdin {
        t.add("stdin", Box::new(stream));
      }
      if let Some(stream) = stdout {
        t.add("stdout", Box::new(stream));
      }
      if let Some(stream) = stderr {
        t.add("stderr", Box::new(stream));
      }
    }
    worker
      .execute("bootstrap.mainRuntime()")
      .expect("Failed to execute bootstrap script");
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

/// Wrapper for `WorkerHandle` that adds functionality
/// for terminating workers.
///
/// This struct is used by host as well as worker itself.
///
/// Host uses it to communicate with worker and terminate it,
/// while worker uses it only to finish execution on `self.close()`.
#[derive(Clone)]
pub struct WebWorkerHandle {
  worker_handle: WorkerHandle,
  terminate_tx: mpsc::Sender<()>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl Deref for WebWorkerHandle {
  type Target = WorkerHandle;
  fn deref(&self) -> &Self::Target {
    &self.worker_handle
  }
}

impl DerefMut for WebWorkerHandle {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.worker_handle
  }
}

impl WebWorkerHandle {
  pub fn terminate(&self) {
    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.terminated.swap(true, Ordering::Relaxed);

    if !already_terminated {
      self.isolate_handle.terminate_execution();
      let mut sender = self.terminate_tx.clone();
      // This call should be infallible hence the `expect`.
      // This might change in the future.
      sender.try_send(()).expect("Failed to terminate");
    }
  }
}

/// This worker is implementation of `Worker` Web API
///
/// At the moment this type of worker supports only
/// communication with parent and creating new workers.
///
/// Each `WebWorker` is either a child of `MainWorker` or other
/// `WebWorker`.
pub struct WebWorker {
  worker: Worker,
  event_loop_idle: bool,
  terminate_rx: mpsc::Receiver<()>,
  handle: WebWorkerHandle,
  pub has_deno_namespace: bool,
}

impl WebWorker {
  pub fn new(
    name: String,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    global_state: Arc<GlobalState>,
    has_deno_namespace: bool,
  ) -> Self {
    let loader = CliModuleLoader::new_for_worker();
    let mut worker = Worker::new(
      name,
      js::deno_isolate_init(),
      global_state.clone(),
      loader,
      false,
    );

    let terminated = Arc::new(AtomicBool::new(false));
    let isolate_handle = worker.isolate.v8_isolate().thread_safe_handle();
    let (terminate_tx, terminate_rx) = mpsc::channel::<()>(1);

    let handle = WebWorkerHandle {
      worker_handle: worker.thread_safe_handle(),
      terminated,
      isolate_handle,
      terminate_tx,
    };

    let mut web_worker = Self {
      worker,
      event_loop_idle: false,
      terminate_rx,
      handle,
      has_deno_namespace,
    };

    {
      let handle = web_worker.thread_safe_handle();
      let sender = web_worker.worker.internal_channels.sender.clone();

      // All ops registered in this function depend on these
      {
        let op_state = web_worker.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<GlobalState>>(global_state.clone());
        op_state.put::<Permissions>(permissions);
      }

      ops::web_worker::init(&mut web_worker, sender, handle);
      ops::runtime::init(&mut web_worker, main_module);
      ops::fetch::init(&mut web_worker, global_state.flags.ca_file.as_deref());
      ops::timers::init(&mut web_worker);
      ops::worker_host::init(&mut web_worker);
      ops::reg_json_sync(&mut web_worker, "op_close", deno_core::op_close);
      ops::reg_json_sync(
        &mut web_worker,
        "op_resources",
        deno_core::op_resources,
      );
      ops::reg_json_sync(
        &mut web_worker,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(&mut web_worker);
      ops::io::init(&mut web_worker);
      ops::websocket::init(&mut web_worker);

      if has_deno_namespace {
        ops::fs_events::init(&mut web_worker);
        ops::fs::init(&mut web_worker);
        ops::net::init(&mut web_worker);
        ops::os::init(&mut web_worker);
        ops::permissions::init(&mut web_worker);
        ops::plugin::init(&mut web_worker);
        ops::process::init(&mut web_worker);
        ops::random::init(&mut web_worker, global_state.flags.seed);
        ops::runtime_compiler::init(&mut web_worker);
        ops::signal::init(&mut web_worker);
        ops::tls::init(&mut web_worker);
        ops::tty::init(&mut web_worker);
      }
    }

    web_worker
  }
}

impl WebWorker {
  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WebWorkerHandle {
    self.handle.clone()
  }
}

impl Deref for WebWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.worker
  }
}

impl DerefMut for WebWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.worker
  }
}

impl Future for WebWorker {
  type Output = Result<(), AnyError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let worker = &mut inner.worker;

    let terminated = inner.handle.terminated.load(Ordering::Relaxed);

    if terminated {
      return Poll::Ready(Ok(()));
    }

    if !inner.event_loop_idle {
      match worker.poll_unpin(cx) {
        Poll::Ready(r) => {
          let terminated = inner.handle.terminated.load(Ordering::Relaxed);
          if terminated {
            return Poll::Ready(Ok(()));
          }

          if let Err(e) = r {
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }
          inner.event_loop_idle = true;
        }
        Poll::Pending => {}
      }
    }

    if let Poll::Ready(r) = inner.terminate_rx.poll_next_unpin(cx) {
      // terminate_rx should never be closed
      assert!(r.is_some());
      return Poll::Ready(Ok(()));
    }

    if let Poll::Ready(r) =
      worker.internal_channels.receiver.poll_next_unpin(cx)
    {
      match r {
        Some(msg) => {
          let msg = String::from_utf8(msg.to_vec()).unwrap();
          let script = format!("workerMessageRecvCallback({})", msg);

          if let Err(e) = worker.execute(&script) {
            // If execution was terminated during message callback then
            // just ignore it
            if inner.handle.terminated.load(Ordering::Relaxed) {
              return Poll::Ready(Ok(()));
            }

            // Otherwise forward error to host
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }

          // Let event loop be polled again
          inner.event_loop_idle = false;
          worker.waker.wake();
        }
        None => unreachable!(),
      }
    }

    Poll::Pending
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags::DenoSubcommand;
  use crate::flags::Flags;
  use crate::global_state::GlobalState;
  use crate::tokio_util;
  use crate::worker::WorkerEvent;
  use deno_core::serde_json::json;

  fn create_test_worker() -> MainWorker {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("./hello.js").unwrap();
    let flags = Flags {
      subcommand: DenoSubcommand::Run {
        script: main_module.to_string(),
      },
      ..Default::default()
    };
    let global_state = GlobalState::mock(vec!["deno".to_string()], Some(flags));
    MainWorker::new(&global_state, main_module)
  }

  #[tokio::test]
  async fn execute_mod_esm_imports_a() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/esm_imports_a.js");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = (&mut *worker).await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = (&mut *worker).await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_006_url_imports() {
    let _http_server_guard = test_util::http_server();
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/006_url_imports.ts");
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = (&mut *worker).await {
      panic!("Future got unexpected error: {:?}", e);
    }
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

  fn create_test_web_worker() -> WebWorker {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("./hello.js").unwrap();
    let global_state = GlobalState::mock(vec!["deno".to_string()], None);
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      Permissions::allow_all(),
      main_module,
      global_state,
      false,
    );
    worker
      .execute("bootstrap.workerRuntime(\"TEST\", false)")
      .unwrap();
    worker
  }
  #[tokio::test]
  async fn test_worker_messages() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_web_worker();
      let source = r#"
          onmessage = function(e) {
            console.log("msg from main script", e.data);
            if (e.data == "exit") {
              return close();
            } else {
              console.assert(e.data === "hi");
            }
            postMessage([1, 2, 3]);
            console.log("after postMessage");
          }
          "#;
      worker.execute(source).unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let r = tokio_util::run_basic(worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    let r = handle.post_message(msg.clone());
    assert!(r.is_ok());

    let maybe_msg = handle.get_event().await.unwrap();
    assert!(maybe_msg.is_some());

    let r = handle.post_message(msg.clone());
    assert!(r.is_ok());

    let maybe_msg = handle.get_event().await.unwrap();
    assert!(maybe_msg.is_some());
    match maybe_msg {
      Some(WorkerEvent::Message(buf)) => {
        assert_eq!(*buf, *b"[1,2,3]");
      }
      _ => unreachable!(),
    }

    let msg = json!("exit")
      .to_string()
      .into_boxed_str()
      .into_boxed_bytes();
    let r = handle.post_message(msg);
    assert!(r.is_ok());
    let event = handle.get_event().await.unwrap();
    assert!(event.is_none());
    handle.sender.close_channel();
    join_handle.join().expect("Failed to join worker thread");
  }

  #[tokio::test]
  async fn removed_from_resource_table_on_close() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_web_worker();
      worker.execute("onmessage = () => { close(); }").unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let r = tokio_util::run_basic(worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    let r = handle.post_message(msg.clone());
    assert!(r.is_ok());
    let event = handle.get_event().await.unwrap();
    assert!(event.is_none());
    handle.sender.close_channel();

    join_handle.join().expect("Failed to join worker thread");
  }
}
