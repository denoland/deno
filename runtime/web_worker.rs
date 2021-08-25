// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::inspector_server::InspectorServer;
use crate::js;
use crate::metrics;
use crate::ops;
use crate::permissions::Permissions;
use crate::tokio_util::create_basic_runtime;
use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::poll_fn;
use deno_core::futures::stream::StreamExt;
use deno_core::located_script_name;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_tls::rustls::RootCertStore;
use deno_web::create_entangled_message_port;
use deno_web::BlobStore;
use deno_web::MessagePort;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebWorkerType {
  Classic,
  Module,
}

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

/// Events that are sent to host from child
/// worker.
pub enum WorkerControlEvent {
  Error(AnyError),
  TerminalError(AnyError),
  Close,
}

use deno_core::serde::Serializer;

impl Serialize for WorkerControlEvent {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let type_id = match &self {
      WorkerControlEvent::TerminalError(_) => 1_i32,
      WorkerControlEvent::Error(_) => 2_i32,
      WorkerControlEvent::Close => 3_i32,
    };

    match self {
      WorkerControlEvent::TerminalError(error)
      | WorkerControlEvent::Error(error) => {
        let value = match error.downcast_ref::<JsError>() {
          Some(js_error) => json!({
            "message": js_error.message,
            "fileName": js_error.script_resource_name,
            "lineNumber": js_error.line_number,
            "columnNumber": js_error.start_column,
          }),
          None => json!({
            "message": error.to_string(),
          }),
        };

        Serialize::serialize(&(type_id, value), serializer)
      }
      _ => Serialize::serialize(&(type_id, ()), serializer),
    }
  }
}

// Channels used for communication with worker's parent
#[derive(Clone)]
pub struct WebWorkerInternalHandle {
  sender: mpsc::Sender<WorkerControlEvent>,
  pub port: Rc<MessagePort>,
  pub cancel: Rc<CancelHandle>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
  pub worker_type: WebWorkerType,
}

impl WebWorkerInternalHandle {
  /// Post WorkerEvent to parent as a worker
  pub fn post_event(&self, event: WorkerControlEvent) -> Result<(), AnyError> {
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

  /// Check if this worker is terminated or being terminated
  pub fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::SeqCst)
  }

  /// Terminate the worker
  /// This function will set terminated to true, terminate the isolate and close the message channel
  pub fn terminate(&mut self) {
    self.cancel.cancel();

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

pub struct SendableWebWorkerHandle {
  port: MessagePort,
  receiver: mpsc::Receiver<WorkerControlEvent>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl From<SendableWebWorkerHandle> for WebWorkerHandle {
  fn from(handle: SendableWebWorkerHandle) -> Self {
    WebWorkerHandle {
      receiver: Rc::new(RefCell::new(handle.receiver)),
      port: Rc::new(handle.port),
      terminated: handle.terminated,
      isolate_handle: handle.isolate_handle,
    }
  }
}

/// This is the handle to the web worker that the parent thread uses to
/// communicate with the worker. It is created from a `SendableWebWorkerHandle`
/// which is sent to the parent thread from the worker thread where it is
/// created. The reason for this separation is that the handle first needs to be
/// `Send` when transferring between threads, and then must be `Clone` when it
/// has arrived on the parent thread. It can not be both at once without large
/// amounts of Arc<Mutex> and other fun stuff.
#[derive(Clone)]
pub struct WebWorkerHandle {
  pub port: Rc<MessagePort>,
  receiver: Rc<RefCell<mpsc::Receiver<WorkerControlEvent>>>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorkerHandle {
  /// Get the WorkerEvent with lock
  /// Return error if more than one listener tries to get event
  pub async fn get_control_event(
    &self,
  ) -> Result<Option<WorkerControlEvent>, AnyError> {
    let mut receiver = self.receiver.borrow_mut();
    Ok(receiver.next().await)
  }

  /// Terminate the worker
  /// This function will set terminated to true, terminate the isolate and close the message channel
  pub fn terminate(self) {
    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.terminated.swap(true, Ordering::SeqCst);

    if !already_terminated {
      // Stop javascript execution
      self.isolate_handle.terminate_execution();
    }

    self.port.disentangle();
  }
}

fn create_handles(
  isolate_handle: v8::IsolateHandle,
  worker_type: WebWorkerType,
) -> (WebWorkerInternalHandle, SendableWebWorkerHandle) {
  let (parent_port, worker_port) = create_entangled_message_port();
  let (ctrl_tx, ctrl_rx) = mpsc::channel::<WorkerControlEvent>(1);
  let terminated = Arc::new(AtomicBool::new(false));
  let internal_handle = WebWorkerInternalHandle {
    sender: ctrl_tx,
    port: Rc::new(parent_port),
    terminated: terminated.clone(),
    isolate_handle: isolate_handle.clone(),
    cancel: CancelHandle::new_rc(),
    worker_type,
  };
  let external_handle = SendableWebWorkerHandle {
    receiver: ctrl_rx,
    port: worker_port,
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
  pub js_runtime: JsRuntime,
  pub name: String,
  internal_handle: WebWorkerInternalHandle,
  pub use_deno_namespace: bool,
  pub worker_type: WebWorkerType,
  pub main_module: ModuleSpecifier,
}

pub struct WebWorkerOptions {
  /// Sets `Deno.args` in JS runtime.
  pub args: Vec<String>,
  pub debug_flag: bool,
  pub unstable: bool,
  pub enable_testing_features: bool,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub root_cert_store: Option<RootCertStore>,
  pub user_agent: String,
  pub seed: Option<u64>,
  pub module_loader: Rc<dyn ModuleLoader>,
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
  pub use_deno_namespace: bool,
  pub worker_type: WebWorkerType,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub apply_source_maps: bool,
  /// Sets `Deno.version.deno` in JS runtime.
  pub runtime_version: String,
  /// Sets `Deno.version.typescript` in JS runtime.
  pub ts_version: String,
  /// Sets `Deno.noColor` in JS runtime.
  pub no_color: bool,
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub cpu_count: usize,
}

impl WebWorker {
  pub fn from_options(
    name: String,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    worker_id: WorkerId,
    options: &WebWorkerOptions,
  ) -> (Self, SendableWebWorkerHandle) {
    // Permissions: many ops depend on this
    let unstable = options.unstable;
    let enable_testing_features = options.enable_testing_features;
    let perm_ext = Extension::builder()
      .state(move |state| {
        state.put::<Permissions>(permissions.clone());
        state.put(ops::UnstableChecker { unstable });
        state.put(ops::TestingFeaturesEnabled(enable_testing_features));
        Ok(())
      })
      .build();

    let mut extensions: Vec<Extension> = vec![
      // Web APIs
      deno_webidl::init(),
      deno_console::init(),
      deno_url::init(),
      deno_web::init(options.blob_store.clone(), Some(main_module.clone())),
      deno_fetch::init::<Permissions>(
        options.user_agent.clone(),
        options.root_cert_store.clone(),
        None,
        None,
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_websocket::init::<Permissions>(
        options.user_agent.clone(),
        options.root_cert_store.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_broadcast_channel::init(
        options.broadcast_channel.clone(),
        options.unstable,
      ),
      deno_crypto::init(options.seed),
      deno_webgpu::init(options.unstable),
      deno_timers::init::<Permissions>(),
      // ffi
      deno_ffi::init::<Permissions>(options.unstable),
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
        deno_tls::init(),
        deno_net::init::<Permissions>(
          options.root_cert_store.clone(),
          options.unstable,
          options.unsafely_ignore_certificate_errors.clone(),
        ),
        ops::os::init(),
        ops::permissions::init(),
        ops::process::init(),
        ops::signal::init(),
        ops::tty::init(),
        deno_http::init(),
        ops::http::init(),
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
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      extensions,
      ..Default::default()
    });

    if let Some(server) = options.maybe_inspector_server.clone() {
      let inspector = js_runtime.inspector();
      let session_sender = inspector.get_session_sender();
      let deregister_rx = inspector.add_deregister_handler();
      server.register_inspector(
        session_sender,
        deregister_rx,
        main_module.to_string(),
      );
    }

    let (internal_handle, external_handle) = {
      let handle = js_runtime.v8_isolate().thread_safe_handle();
      let (internal_handle, external_handle) =
        create_handles(handle, options.worker_type);
      let op_state = js_runtime.op_state();
      let mut op_state = op_state.borrow_mut();
      op_state.put(internal_handle.clone());
      (internal_handle, external_handle)
    };

    (
      Self {
        id: worker_id,
        js_runtime,
        name,
        internal_handle,
        use_deno_namespace: options.use_deno_namespace,
        worker_type: options.worker_type,
        main_module,
      },
      external_handle,
    )
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
      "enableTestingFeaturesFlag": options.enable_testing_features,
      "v8Version": deno_core::v8_version(),
      "location": self.main_module,
      "cpuCount": options.cpu_count,
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
      .execute_script(&located_script_name!(), &script)
      .expect("Failed to execute worker bootstrap script");
  }

  /// See [JsRuntime::execute_script](deno_core::JsRuntime::execute_script)
  pub fn execute_script(
    &mut self,
    name: &str,
    source_code: &str,
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(name, source_code)?;
    Ok(())
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
      maybe_result = &mut receiver => {
        debug!("received worker module evaluate {:#?}", maybe_result);
        // If `None` is returned it means that runtime was destroyed before
        // evaluation was complete. This can happen in Web Worker when `self.close()`
        // is called at top level.
        maybe_result.unwrap_or(Ok(()))
      }

      event_loop_result = self.run_event_loop(false) => {
        if self.internal_handle.is_terminated() {
           return Ok(());
        }
        event_loop_result?;
        let maybe_result = receiver.await;
        maybe_result.unwrap_or(Ok(()))
      }
    }
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
    wait_for_inspector: bool,
  ) -> Poll<Result<(), AnyError>> {
    // If awakened because we are terminating, just return Ok
    if self.internal_handle.is_terminated() {
      return Poll::Ready(Ok(()));
    }

    match self.js_runtime.poll_event_loop(cx, wait_for_inspector) {
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
            .post_event(WorkerControlEvent::Error(e))
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

  pub async fn run_event_loop(
    &mut self,
    wait_for_inspector: bool,
  ) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx, wait_for_inspector)).await
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

  let fut = async move {
    // Execute provided source code immediately
    let result = if let Some(source_code) = maybe_source_code {
      worker.execute_script(&located_script_name!(), &source_code)
    } else {
      // TODO(bartlomieju): add "type": "classic", ie. ability to load
      // script instead of module
      worker.execute_module(&specifier).await
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
        .post_event(WorkerControlEvent::TerminalError(e))
        .expect("Failed to post message to host");

      // Failure to execute script is a terminal error, bye, bye.
      return Ok(());
    }

    let result = worker.run_event_loop(true).await;
    debug!("Worker thread shuts down {}", &name);
    result
  };

  rt.block_on(fut)
}
