// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fmt_errors::JsError;
use crate::global_state::GlobalState;
use crate::inspector::DenoInspector;
use crate::js;
use crate::metrics::Metrics;
use crate::ops;
use crate::ops::io::get_stdio;
use crate::ops::timers;
use crate::ops::worker_host::WorkerId;
use crate::ops::worker_host::WorkersTable;
use crate::permissions::Permissions;
use crate::state::CliModuleLoader;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use futures::task::AtomicWaker;
use rand::rngs::StdRng;
use rand::SeedableRng;
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
/// Currently there are three types of workers:
///  - `MainWorker`
///  - `CompilerWorker`
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
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    name: String,
    startup_snapshot: Option<Snapshot>,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    global_state: Arc<GlobalState>,
    state: Rc<CliModuleLoader>,
    is_main: bool,
    is_internal: bool,
  ) -> Self {
    let global_state_ = global_state.clone();

    let mut isolate = JsRuntime::new(RuntimeOptions {
      module_loader: Some(state),
      startup_snapshot,
      js_error_create_fn: Some(Box::new(move |core_js_error| {
        JsError::create(core_js_error, &global_state_.ts_compiler)
      })),
      ..Default::default()
    });
    {
      let op_state = isolate.op_state();
      let mut op_state = op_state.borrow_mut();
      op_state.get_error_class_fn = &crate::errors::get_error_class_name;

      let ca_file = global_state.flags.ca_file.as_deref();
      let client = crate::http_util::create_http_client(ca_file).unwrap();
      op_state.put(client);

      op_state.put(timers::GlobalTimer::default());
      op_state.put(timers::StartTime::now());

      if let Some(seed) = global_state.flags.seed {
        op_state.put(StdRng::seed_from_u64(seed));
      }

      op_state.put(Metrics::default());

      op_state.put(WorkersTable::default());
      op_state.put(WorkerId::default());

      op_state.put(permissions);

      op_state.put(main_module);
      op_state.put(global_state.clone());
    }
    let inspector = {
      global_state
        .flags
        .inspect
        .or(global_state.flags.inspect_brk)
        .filter(|_| !is_internal)
        .map(|inspector_host| DenoInspector::new(&mut isolate, inspector_host))
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
    self.isolate.mod_evaluate(id)
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
    self.isolate.mod_evaluate(id)
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
  // TODO(ry) combine MainWorker::new and MainWorker::create.
  fn new(
    name: String,
    startup_snapshot: Option<Snapshot>,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    global_state: Arc<GlobalState>,
  ) -> Self {
    let loader = CliModuleLoader::new(global_state.maybe_import_map.clone());
    let mut worker = Worker::new(
      name,
      startup_snapshot,
      permissions,
      main_module,
      global_state,
      loader,
      true,
      false,
    );
    {
      ops::runtime::init(&mut worker);
      ops::runtime_compiler::init(&mut worker);
      ops::errors::init(&mut worker);
      ops::fetch::init(&mut worker);
      ops::websocket::init(&mut worker);
      ops::fs::init(&mut worker);
      ops::fs_events::init(&mut worker);
      ops::reg_json_sync(
        &mut worker,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::io::init(&mut worker);
      ops::plugin::init(&mut worker);
      ops::net::init(&mut worker);
      ops::tls::init(&mut worker);
      ops::os::init(&mut worker);
      ops::permissions::init(&mut worker);
      ops::process::init(&mut worker);
      ops::random::init(&mut worker);
      ops::repl::init(&mut worker);
      ops::reg_json_sync(&mut worker, "op_close", deno_core::op_close);
      ops::reg_json_sync(&mut worker, "op_resources", deno_core::op_resources);
      ops::signal::init(&mut worker);
      ops::timers::init(&mut worker);
      ops::tty::init(&mut worker);
      ops::worker_host::init(&mut worker);
    }
    Self(worker)
  }

  pub fn create(
    global_state: &Arc<GlobalState>,
    main_module: ModuleSpecifier,
  ) -> Result<MainWorker, AnyError> {
    let mut worker = MainWorker::new(
      "main".to_string(),
      Some(js::deno_isolate_init()),
      global_state.permissions.clone(),
      main_module,
      global_state.clone(),
    );
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
    worker.execute("bootstrap.mainRuntime()")?;
    Ok(worker)
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
  use crate::js;
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
    let global_state_ = global_state.clone();
    tokio_util::run_basic(async {
      let mut worker = MainWorker::new(
        "TEST".to_string(),
        None,
        global_state.permissions.clone(),
        module_specifier.clone(),
        global_state_,
      );
      let result = worker.execute_module(&module_specifier).await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      if let Err(e) = (&mut *worker).await {
        panic!("Future got unexpected error: {:?}", e);
      }
    });
    // Check that we didn't start the compiler.
    assert_eq!(global_state.compiler_starts.load(Ordering::SeqCst), 0);
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
    let global_state_ = global_state.clone();
    tokio_util::run_basic(async {
      let mut worker = MainWorker::new(
        "TEST".to_string(),
        None,
        global_state_.permissions.clone(),
        module_specifier.clone(),
        global_state_,
      );
      let result = worker.execute_module(&module_specifier).await;
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      if let Err(e) = (&mut *worker).await {
        panic!("Future got unexpected error: {:?}", e);
      }
    });

    // Check that we didn't start the compiler.
    assert_eq!(global_state.compiler_starts.load(Ordering::SeqCst), 0);
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
    let flags = flags::Flags {
      subcommand: flags::DenoSubcommand::Run {
        script: module_specifier.to_string(),
      },
      reload: true,
      ..flags::Flags::default()
    };
    let global_state = GlobalState::new(flags).unwrap();
    let mut worker = MainWorker::new(
      "TEST".to_string(),
      Some(js::deno_isolate_init()),
      global_state.permissions.clone(),
      module_specifier.clone(),
      global_state.clone(),
    );
    worker.execute("bootstrap.mainRuntime()").unwrap();
    let result = worker.execute_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = (&mut *worker).await {
      panic!("Future got unexpected error: {:?}", e);
    }
    // Check that we've only invoked the compiler once.
    assert_eq!(global_state.compiler_starts.load(Ordering::SeqCst), 1);
  }

  fn create_test_worker() -> MainWorker {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("./hello.js").unwrap();
    let global_state = GlobalState::mock(vec!["deno".to_string()], None);
    let mut worker = MainWorker::new(
      "TEST".to_string(),
      Some(js::deno_isolate_init()),
      Permissions::allow_all(),
      main_module,
      global_state,
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
