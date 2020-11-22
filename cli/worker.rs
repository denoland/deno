// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fmt_errors::PrettyJsError;
use crate::inspector::DenoInspector;
use crate::inspector::InspectorServer;
use crate::inspector::InspectorSession;
use crate::js;
use crate::metrics::Metrics;
use crate::module_loader::CliModuleLoader;
use crate::ops;
use crate::ops::io::get_stdio;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::AtomicWaker;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use std::env;
use std::ops::Deref;
use std::ops::DerefMut;
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
  external_channels: WorkerHandle,
  inspector: Option<Box<DenoInspector>>,
  // Following fields are pub because they are accessed
  // when creating a new WebWorker instance.
  pub(crate) internal_channels: WorkerChannelsInternal,
  pub(crate) js_runtime: JsRuntime,
  pub(crate) name: String,
  should_break_on_first_statement: bool,
  waker: AtomicWaker,
}

impl Worker {
  pub fn new(
    name: String,
    startup_snapshot: Snapshot,
    module_loader: Rc<CliModuleLoader>,
    js_error_create_fn: Box<JsErrorCreateFn>,
  ) -> Self {
    let js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(module_loader),
      startup_snapshot: Some(startup_snapshot),
      js_error_create_fn: Some(js_error_create_fn),
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      ..Default::default()
    });

    let (internal_channels, external_channels) = create_channels();

    Self {
      external_channels,
      inspector: None,
      internal_channels,
      js_runtime,
      name,
      should_break_on_first_statement: false,
      waker: AtomicWaker::new(),
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
    self.js_runtime.execute(js_filename, js_source)
  }

  /// Loads and instantiates specified JavaScript module.
  pub async fn preload_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, AnyError> {
    self.js_runtime.load_module(module_specifier, None).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_module(module_specifier).await?;
    self.wait_for_inspector_session();
    self.js_runtime.mod_evaluate(id).await
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

  pub fn attach_inspector(
    &mut self,
    inspector_server: Arc<InspectorServer>,
    break_on_first_statement: bool,
  ) {
    let inspector =
      DenoInspector::new(&mut self.js_runtime, Some(inspector_server));
    self.inspector = Some(inspector);
    self.should_break_on_first_statement = break_on_first_statement;
  }

  pub fn create_inspector_session(&mut self) -> Box<InspectorSession> {
    let inspector = DenoInspector::new(&mut self.js_runtime, None);
    self.inspector = Some(inspector);
    let inspector = self.inspector.as_mut().unwrap();

    InspectorSession::new(&mut **inspector)
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    // We always poll the inspector if it exists.
    let _ = self.inspector.as_mut().map(|i| i.poll_unpin(cx));
    self.waker.register(cx.waker());
    self.js_runtime.poll_event_loop(cx)
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }
}

impl Drop for Worker {
  fn drop(&mut self) {
    // The Isolate object must outlive the Inspector object, but this is
    // currently not enforced by the type system.
    self.inspector.take();
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
    program_state: &Arc<ProgramState>,
    main_module: ModuleSpecifier,
    permissions: Permissions,
  ) -> Self {
    let program_state = program_state.clone();
    let program_state_ = program_state.clone();
    let loader = CliModuleLoader::new(program_state.maybe_import_map.clone());
    let js_error_create_fn = Box::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, program_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let mut worker = Worker::new(
      "main".to_string(),
      js::deno_isolate_init(),
      loader,
      js_error_create_fn,
    );

    if let Some(inspector_server) = program_state.maybe_inspector_server.clone()
    {
      worker.attach_inspector(
        inspector_server,
        program_state.flags.inspect_brk.is_some(),
      );
    }

    let js_runtime = &mut worker.js_runtime;
    {
      // All ops registered in this function depend on these
      {
        let op_state = js_runtime.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<ProgramState>>(program_state.clone());
        op_state.put::<Permissions>(permissions);
      }

      ops::runtime::init(js_runtime, main_module);
      ops::fetch::init(js_runtime, program_state.flags.ca_file.as_deref());
      ops::timers::init(js_runtime);
      ops::worker_host::init(js_runtime, None);
      ops::crypto::init(js_runtime, program_state.flags.seed);
      ops::reg_json_sync(js_runtime, "op_close", deno_core::op_close);
      ops::reg_json_sync(js_runtime, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        js_runtime,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(js_runtime);
      ops::fs_events::init(js_runtime);
      ops::fs::init(js_runtime);
      ops::io::init(js_runtime);
      ops::net::init(js_runtime);
      ops::os::init(js_runtime);
      ops::permissions::init(js_runtime);
      ops::plugin::init(js_runtime);
      ops::process::init(js_runtime);
      ops::runtime_compiler::init(js_runtime);
      ops::signal::init(js_runtime);
      ops::tls::init(js_runtime);
      ops::tty::init(js_runtime);
      ops::websocket::init(js_runtime);
    }
    {
      let op_state = js_runtime.op_state();
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
    program_state: Arc<ProgramState>,
    has_deno_namespace: bool,
  ) -> Self {
    let program_state_ = program_state.clone();
    let loader = CliModuleLoader::new_for_worker();
    let js_error_create_fn = Box::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, program_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let mut worker =
      Worker::new(name, js::deno_isolate_init(), loader, js_error_create_fn);

    if let Some(inspector_server) = program_state.maybe_inspector_server.clone()
    {
      worker.attach_inspector(inspector_server, false);
    }

    let terminated = Arc::new(AtomicBool::new(false));
    let isolate_handle = worker.js_runtime.v8_isolate().thread_safe_handle();
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
      let js_runtime = &mut web_worker.js_runtime;
      // All ops registered in this function depend on these
      {
        let op_state = js_runtime.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<ProgramState>>(program_state.clone());
        op_state.put::<Permissions>(permissions);
      }

      ops::web_worker::init(js_runtime, sender.clone(), handle);
      ops::runtime::init(js_runtime, main_module);
      ops::fetch::init(js_runtime, program_state.flags.ca_file.as_deref());
      ops::timers::init(js_runtime);
      ops::worker_host::init(js_runtime, Some(sender));
      ops::reg_json_sync(js_runtime, "op_close", deno_core::op_close);
      ops::reg_json_sync(js_runtime, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        js_runtime,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(js_runtime);
      ops::io::init(js_runtime);
      ops::websocket::init(js_runtime);

      if has_deno_namespace {
        ops::fs_events::init(js_runtime);
        ops::fs::init(js_runtime);
        ops::net::init(js_runtime);
        ops::os::init(js_runtime);
        ops::permissions::init(js_runtime);
        ops::plugin::init(js_runtime);
        ops::process::init(js_runtime);
        ops::crypto::init(js_runtime, program_state.flags.seed);
        ops::runtime_compiler::init(js_runtime);
        ops::signal::init(js_runtime);
        ops::tls::init(js_runtime);
        ops::tty::init(js_runtime);
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

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    let worker = &mut self.worker;

    let terminated = self.handle.terminated.load(Ordering::Relaxed);

    if terminated {
      return Poll::Ready(Ok(()));
    }

    if !self.event_loop_idle {
      match worker.poll_event_loop(cx) {
        Poll::Ready(r) => {
          let terminated = self.handle.terminated.load(Ordering::Relaxed);
          if terminated {
            return Poll::Ready(Ok(()));
          }

          if let Err(e) = r {
            eprintln!(
              "{}: Uncaught (in worker \"{}\") {}",
              colors::red_bold("error"),
              worker.name.to_string(),
              e.to_string().trim_start_matches("Uncaught "),
            );
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }
          self.event_loop_idle = true;
        }
        Poll::Pending => {}
      }
    }

    if let Poll::Ready(r) = self.terminate_rx.poll_next_unpin(cx) {
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
            if self.handle.terminated.load(Ordering::Relaxed) {
              return Poll::Ready(Ok(()));
            }

            // Otherwise forward error to host
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }

          // Let event loop be polled again
          self.event_loop_idle = false;
          worker.waker.wake();
        }
        None => unreachable!(),
      }
    }

    Poll::Pending
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags::DenoSubcommand;
  use crate::flags::Flags;
  use crate::program_state::ProgramState;
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
    let permissions = Permissions::from_flags(&flags);
    let program_state =
      ProgramState::mock(vec!["deno".to_string()], Some(flags));
    MainWorker::new(&program_state, main_module, permissions)
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
    if let Err(e) = worker.run_event_loop().await {
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
    if let Err(e) = worker.run_event_loop().await {
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
    if let Err(e) = worker.run_event_loop().await {
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
    let program_state = ProgramState::mock(vec!["deno".to_string()], None);
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      Permissions::allow_all(),
      main_module,
      program_state,
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
      let r = tokio_util::run_basic(worker.run_event_loop());
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
      let r = tokio_util::run_basic(worker.run_event_loop());
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
