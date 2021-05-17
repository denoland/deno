// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::inspector::DenoInspector;
use crate::inspector::InspectorServer;
use crate::js;
use crate::metrics;
use crate::ops;
use crate::permissions::Permissions;
use crate::tokio_util::create_basic_runtime;
use deno_core::error::AnyError;
use deno_core::error::Context as ErrorContext;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::futures::stream::StreamExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::ZeroCopyBuf;
use deno_file::BlobUrlStore;
use log::debug;
use std::cell::RefCell;
use std::env;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;

#[derive(
  Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct WorkerId(u32);
impl fmt::Display for WorkerId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "worker-{}", self.0)
  }
}
impl WorkerId {
  pub fn next(&self) -> Option<WorkerId> {
    self.0.checked_add(1).map(WorkerId)
  }
}

type WorkerMessage = ZeroCopyBuf;

/// Events that are sent to host from child
/// worker.
pub enum WorkerEvent {
  Message(WorkerMessage),
  Error(AnyError),
  TerminalError(AnyError),
  Close,
}

// Channels used for communication with worker's parent
#[derive(Clone)]
pub struct WebWorkerInternalHandle {
  sender: mpsc::Sender<WorkerEvent>,
  receiver: Rc<RefCell<mpsc::Receiver<WorkerMessage>>>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorkerInternalHandle {
  /// Post WorkerEvent to parent as a worker
  pub fn post_event(&self, event: WorkerEvent) -> Result<(), AnyError> {
    let mut sender = self.sender.clone();
    // If the channel is closed,
    // the worker must have terminated but the termination message has not yet been received.
    //
    // Therefore just treat it as if the worker has terminated and return.
    if sender.is_closed() {
      self.terminated.store(true, Ordering::SeqCst);
      return Ok(());
    }
    sender.try_send(event)?;
    Ok(())
  }

  /// Get the WorkerEvent with lock
  /// Panic if more than one listener tries to get event
  pub async fn get_message(&self) -> Option<WorkerMessage> {
    let mut receiver = self.receiver.borrow_mut();
    receiver.next().await
  }

  /// Check if this worker is terminated or being terminated
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::SeqCst)
  }

  /// Terminate the worker
  /// This function will set terminated to true, terminate the isolate and close the message channel
  pub fn terminate(&mut self) {
    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.terminated.swap(true, Ordering::SeqCst);

    if !already_terminated {
      // Stop javascript execution
      self.isolate_handle.terminate_execution();
    }

    // Wake parent by closing the channel
    self.sender.close_channel();
  }
}

#[derive(Clone)]
pub struct WebWorkerHandle {
  sender: mpsc::Sender<WorkerMessage>,
  receiver: Arc<AsyncMutex<mpsc::Receiver<WorkerEvent>>>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorkerHandle {
  /// Post WorkerMessage to worker as a host
  pub fn post_message(&self, buf: WorkerMessage) -> Result<(), AnyError> {
    let mut sender = self.sender.clone();
    // If the channel is closed,
    // the worker must have terminated but the termination message has not yet been recieved.
    //
    // Therefore just treat it as if the worker has terminated and return.
    if sender.is_closed() {
      self.terminated.store(true, Ordering::SeqCst);
      return Ok(());
    }
    sender.try_send(buf)?;
    Ok(())
  }

  /// Get the WorkerEvent with lock
  /// Return error if more than one listener tries to get event
  pub async fn get_event(&self) -> Result<Option<WorkerEvent>, AnyError> {
    let mut receiver = self.receiver.try_lock()?;
    Ok(receiver.next().await)
  }

  /// Terminate the worker
  /// This function will set terminated to true, terminate the isolate and close the message channel
  pub fn terminate(&mut self) {
    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.terminated.swap(true, Ordering::SeqCst);

    if !already_terminated {
      // Stop javascript execution
      self.isolate_handle.terminate_execution();
    }

    // Wake web worker by closing the channel
    self.sender.close_channel();
  }
}

fn create_handles(
  isolate_handle: v8::IsolateHandle,
) -> (WebWorkerInternalHandle, WebWorkerHandle) {
  let (in_tx, in_rx) = mpsc::channel::<WorkerMessage>(1);
  let (out_tx, out_rx) = mpsc::channel::<WorkerEvent>(1);
  let terminated = Arc::new(AtomicBool::new(false));
  let internal_handle = WebWorkerInternalHandle {
    sender: out_tx,
    receiver: Rc::new(RefCell::new(in_rx)),
    terminated: terminated.clone(),
    isolate_handle: isolate_handle.clone(),
  };
  let external_handle = WebWorkerHandle {
    sender: in_tx,
    receiver: Arc::new(AsyncMutex::new(out_rx)),
    terminated,
    isolate_handle,
  };
  (internal_handle, external_handle)
}

/// This struct is an implementation of `Worker` Web API
///
/// Each `WebWorker` is either a child of `MainWorker` or other
/// `WebWorker`.
pub struct WebWorker {
  id: WorkerId,
  inspector: Option<Box<DenoInspector>>,
  pub js_runtime: JsRuntime,
  pub name: String,
  internal_handle: WebWorkerInternalHandle,
  external_handle: WebWorkerHandle,
  pub use_deno_namespace: bool,
  pub main_module: ModuleSpecifier,
}

pub struct WebWorkerOptions {
  /// Sets `Deno.args` in JS runtime.
  pub args: Vec<String>,
  pub debug_flag: bool,
  pub unstable: bool,
  pub ca_data: Option<Vec<u8>>,
  pub user_agent: String,
  pub seed: Option<u64>,
  pub module_loader: Rc<dyn ModuleLoader>,
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
  pub use_deno_namespace: bool,
  pub attach_inspector: bool,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub apply_source_maps: bool,
  /// Sets `Deno.version.deno` in JS runtime.
  pub runtime_version: String,
  /// Sets `Deno.version.typescript` in JS runtime.
  pub ts_version: String,
  /// Sets `Deno.noColor` in JS runtime.
  pub no_color: bool,
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub blob_url_store: BlobUrlStore,
}

impl WebWorker {
  pub fn from_options(
    name: String,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    worker_id: WorkerId,
    options: &WebWorkerOptions,
  ) -> Self {
    // Permissions: many ops depend on this
    let unstable = options.unstable;
    let perm_ext = Extension::builder()
      .state(move |state| {
        state.put::<Permissions>(permissions.clone());
        state.put(ops::UnstableChecker { unstable });
        Ok(())
      })
      .build();

    let mut extensions: Vec<Extension> = vec![
      // Web APIs
      deno_webidl::init(),
      deno_console::init(),
      deno_url::init(),
      deno_web::init(),
      deno_file::init(
        options.blob_url_store.clone(),
        Some(main_module.clone()),
      ),
      deno_fetch::init::<Permissions>(
        options.user_agent.clone(),
        options.ca_data.clone(),
      ),
      deno_websocket::init::<Permissions>(
        options.user_agent.clone(),
        options.ca_data.clone(),
      ),
      deno_crypto::init(options.seed),
      deno_webgpu::init(options.unstable),
      deno_timers::init::<Permissions>(),
      // Metrics
      metrics::init(),
      // Permissions ext (worker specific state)
      perm_ext,
    ];

    // Runtime ops that are always initialized for WebWorkers
    let runtime_exts = vec![
      ops::web_worker::init(),
      ops::runtime::init(main_module.clone()),
      ops::worker_host::init(options.create_web_worker_cb.clone()),
      ops::io::init(),
    ];

    // Extensions providing Deno.* features
    let deno_ns_exts = if options.use_deno_namespace {
      vec![
        ops::fs_events::init(),
        ops::fs::init(),
        ops::net::init(),
        ops::os::init(),
        ops::http::init(),
        ops::permissions::init(),
        ops::plugin::init(),
        ops::process::init(),
        ops::signal::init(),
        ops::tls::init(),
        ops::tty::init(),
        ops::io::init_stdio(),
      ]
    } else {
      vec![]
    };

    // Append exts
    extensions.extend(runtime_exts);
    extensions.extend(deno_ns_exts); // May be empty

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(js::deno_isolate_init()),
      js_error_create_fn: options.js_error_create_fn.clone(),
      get_error_class_fn: options.get_error_class_fn,
      extensions,
      ..Default::default()
    });

    let inspector = if options.attach_inspector {
      Some(DenoInspector::new(
        &mut js_runtime,
        options.maybe_inspector_server.clone(),
      ))
    } else {
      None
    };

    let (internal_handle, external_handle) = {
      let handle = js_runtime.v8_isolate().thread_safe_handle();
      let (internal_handle, external_handle) = create_handles(handle);
      let op_state = js_runtime.op_state();
      let mut op_state = op_state.borrow_mut();
      op_state.put(internal_handle.clone());
      (internal_handle, external_handle)
    };

    Self {
      id: worker_id,
      inspector,
      js_runtime,
      name,
      internal_handle,
      external_handle,
      use_deno_namespace: options.use_deno_namespace,
      main_module,
    }
  }

  pub fn bootstrap(&mut self, options: &WebWorkerOptions) {
    let runtime_options = json!({
      "args": options.args,
      "applySourceMaps": options.apply_source_maps,
      "debugFlag": options.debug_flag,
      "denoVersion": options.runtime_version,
      "noColor": options.no_color,
      "pid": std::process::id(),
      "ppid": ops::runtime::ppid(),
      "target": env!("TARGET"),
      "tsVersion": options.ts_version,
      "unstableFlag": options.unstable,
      "v8Version": deno_core::v8_version(),
      "location": self.main_module,
    });

    let runtime_options_str =
      serde_json::to_string_pretty(&runtime_options).unwrap();

    // Instead of using name for log we use `worker-${id}` because
    // WebWorkers can have empty string as name.
    let script = format!(
      "bootstrap.workerRuntime({}, \"{}\", {}, \"{}\")",
      runtime_options_str, self.name, options.use_deno_namespace, self.id
    );
    self
      .execute(&script)
      .expect("Failed to execute worker bootstrap script");
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), AnyError> {
    let path = env::current_dir()
      .context("Failed to get current working directory")?
      .join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.js_runtime.execute(url.as_str(), js_source)
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

    let mut receiver = self.js_runtime.mod_evaluate(id);
    tokio::select! {
      maybe_result = receiver.next() => {
        debug!("received worker module evaluate {:#?}", maybe_result);
        // If `None` is returned it means that runtime was destroyed before
        // evaluation was complete. This can happen in Web Worker when `self.close()`
        // is called at top level.
        let result = maybe_result.unwrap_or(Ok(()));
        return result;
      }

      event_loop_result = self.run_event_loop() => {
        if self.internal_handle.is_terminated() {
           return Ok(());
        }
        event_loop_result?;
        let maybe_result = receiver.next().await;
        let result = maybe_result.unwrap_or(Ok(()));
        return result;
      }
    }
  }

  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WebWorkerHandle {
    self.external_handle.clone()
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    // If awakened because we are terminating, just return Ok
    if self.internal_handle.is_terminated() {
      return Poll::Ready(Ok(()));
    }

    // We always poll the inspector if it exists.
    let _ = self.inspector.as_mut().map(|i| i.poll_unpin(cx));
    match self.js_runtime.poll_event_loop(cx) {
      Poll::Ready(r) => {
        // If js ended because we are terminating, just return Ok
        if self.internal_handle.is_terminated() {
          return Poll::Ready(Ok(()));
        }

        // In case of an error, pass to parent without terminating worker
        if let Err(e) = r {
          print_worker_error(e.to_string(), &self.name);
          let handle = self.internal_handle.clone();
          handle
            .post_event(WorkerEvent::Error(e))
            .expect("Failed to post message to host");

          return Poll::Pending;
        }

        panic!(
          "coding error: either js is polling or the worker is terminated"
        );
      }
      Poll::Pending => Poll::Pending,
    }
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }
}

impl Drop for WebWorker {
  fn drop(&mut self) {
    // The Isolate object must outlive the Inspector object, but this is
    // currently not enforced by the type system.
    self.inspector.take();
  }
}

fn print_worker_error(error_str: String, name: &str) {
  eprintln!(
    "{}: Uncaught (in worker \"{}\") {}",
    colors::red_bold("error"),
    name,
    error_str.trim_start_matches("Uncaught "),
  );
}

/// This function should be called from a thread dedicated to this worker.
// TODO(bartlomieju): check if order of actions is aligned to Worker spec
pub fn run_web_worker(
  mut worker: WebWorker,
  specifier: ModuleSpecifier,
  maybe_source_code: Option<String>,
) -> Result<(), AnyError> {
  let name = worker.name.to_string();

  let rt = create_basic_runtime();

  // TODO(bartlomieju): run following block using "select!"
  // with terminate

  // Execute provided source code immediately
  let result = if let Some(source_code) = maybe_source_code {
    worker.execute(&source_code)
  } else {
    // TODO(bartlomieju): add "type": "classic", ie. ability to load
    // script instead of module
    let load_future = worker.execute_module(&specifier).boxed_local();

    rt.block_on(load_future)
  };

  let internal_handle = worker.internal_handle.clone();

  // If sender is closed it means that worker has already been closed from
  // within using "globalThis.close()"
  if internal_handle.is_terminated() {
    return Ok(());
  }

  if let Err(e) = result {
    print_worker_error(e.to_string(), &name);
    internal_handle
      .post_event(WorkerEvent::TerminalError(e))
      .expect("Failed to post message to host");

    // Failure to execute script is a terminal error, bye, bye.
    return Ok(());
  }

  let result = rt.block_on(worker.run_event_loop());
  debug!("Worker thread shuts down {}", &name);
  result
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tokio_util;

  fn create_test_web_worker() -> WebWorker {
    let main_module = deno_core::resolve_url_or_path("./hello.js").unwrap();
    let module_loader = Rc::new(deno_core::NoopModuleLoader);
    let create_web_worker_cb = Arc::new(|_| unreachable!());

    let options = WebWorkerOptions {
      args: vec![],
      apply_source_maps: false,
      debug_flag: false,
      unstable: false,
      ca_data: None,
      user_agent: "x".to_string(),
      seed: None,
      module_loader,
      create_web_worker_cb,
      js_error_create_fn: None,
      use_deno_namespace: false,
      attach_inspector: false,
      maybe_inspector_server: None,
      runtime_version: "x".to_string(),
      ts_version: "x".to_string(),
      no_color: true,
      get_error_class_fn: None,
      blob_url_store: BlobUrlStore::default(),
    };

    let mut worker = WebWorker::from_options(
      "TEST".to_string(),
      Permissions::allow_all(),
      main_module,
      WorkerId(1),
      &options,
    );
    worker.bootstrap(&options);
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

    // TODO(Inteon): use Deno.core.serialize() instead of hardcoded encoded value
    let msg = vec![34, 2, 104, 105].into_boxed_slice(); // "hi" encoded
    let r = handle.post_message(msg.clone().into());
    assert!(r.is_ok());

    let maybe_msg = handle.get_event().await.unwrap();
    assert!(maybe_msg.is_some());

    let r = handle.post_message(msg.clone().into());
    assert!(r.is_ok());

    let maybe_msg = handle.get_event().await.unwrap();
    assert!(maybe_msg.is_some());
    match maybe_msg {
      Some(WorkerEvent::Message(buf)) => {
        // TODO(Inteon): use Deno.core.serialize() instead of hardcoded encoded value
        assert_eq!(*buf, [65, 3, 73, 2, 73, 4, 73, 6, 36, 0, 3]);
      }
      _ => unreachable!(),
    }

    // TODO(Inteon): use Deno.core.serialize() instead of hardcoded encoded value
    let msg = vec![34, 4, 101, 120, 105, 116].into_boxed_slice(); // "exit" encoded
    let r = handle.post_message(msg.into());
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

    // TODO(Inteon): use Deno.core.serialize() instead of hardcoded encoded value
    let msg = vec![34, 2, 104, 105].into_boxed_slice(); // "hi" encoded
    let r = handle.post_message(msg.clone().into());
    assert!(r.is_ok());
    let event = handle.get_event().await.unwrap();
    assert!(event.is_none());
    handle.sender.close_channel();

    join_handle.join().expect("Failed to join worker thread");
  }
}
