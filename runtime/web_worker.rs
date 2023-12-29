// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::inspector_server::InspectorServer;
use crate::ops;
use crate::permissions::PermissionsContainer;
use crate::shared::runtime;
use crate::tokio_util::create_and_run_current_thread;
use crate::worker::import_meta_resolve_callback;
use crate::worker::FormatJsErrorFn;
use crate::BootstrapOptions;
use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_cache::CreateCache;
use deno_cache::SqliteBackedCache;
use deno_core::ascii_str;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::poll_fn;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::AtomicWaker;
use deno_core::located_script_name;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json::json;
use deno_core::v8;
use deno_core::CancelHandle;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::FeatureChecker;
use deno_core::GetErrorClassFn;
use deno_core::JsRuntime;
use deno_core::ModuleCode;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpMetricsSummaryTracker;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_core::Snapshot;
use deno_core::SourceMapGetter;
use deno_cron::local::LocalCronHandler;
use deno_fs::FileSystem;
use deno_http::DefaultHttpPropertyExtractor;
use deno_io::Stdio;
use deno_kv::dynamic::MultiBackendDbHandler;
use deno_tls::RootCertStoreProvider;
use deno_web::create_entangled_message_port;
use deno_web::BlobStore;
use deno_web::MessagePort;
use log::debug;
use std::cell::RefCell;
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
          Some(js_error) => {
            let frame = js_error.frames.iter().find(|f| match &f.file_name {
              Some(s) => !s.trim_start_matches('[').starts_with("ext:"),
              None => false,
            });
            json!({
              "message": js_error.exception_message,
              "fileName": frame.map(|f| f.file_name.as_ref()),
              "lineNumber": frame.map(|f| f.line_number.as_ref()),
              "columnNumber": frame.map(|f| f.column_number.as_ref()),
            })
          }
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
  termination_signal: Arc<AtomicBool>,
  has_terminated: Arc<AtomicBool>,
  terminate_waker: Arc<AtomicWaker>,
  isolate_handle: v8::IsolateHandle,
  pub name: String,
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
      self.has_terminated.store(true, Ordering::SeqCst);
      return Ok(());
    }
    sender.try_send(event)?;
    Ok(())
  }

  /// Check if this worker is terminated or being terminated
  pub fn is_terminated(&self) -> bool {
    self.has_terminated.load(Ordering::SeqCst)
  }

  /// Check if this worker must terminate (because the termination signal is
  /// set), and terminates it if so. Returns whether the worker is terminated or
  /// being terminated, as with [`Self::is_terminated()`].
  pub fn terminate_if_needed(&mut self) -> bool {
    let has_terminated = self.is_terminated();

    if !has_terminated && self.termination_signal.load(Ordering::SeqCst) {
      self.terminate();
      return true;
    }

    has_terminated
  }

  /// Terminate the worker
  /// This function will set terminated to true, terminate the isolate and close the message channel
  pub fn terminate(&mut self) {
    self.cancel.cancel();
    self.terminate_waker.wake();

    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.has_terminated.swap(true, Ordering::SeqCst);

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
  termination_signal: Arc<AtomicBool>,
  has_terminated: Arc<AtomicBool>,
  terminate_waker: Arc<AtomicWaker>,
  isolate_handle: v8::IsolateHandle,
}

impl From<SendableWebWorkerHandle> for WebWorkerHandle {
  fn from(handle: SendableWebWorkerHandle) -> Self {
    WebWorkerHandle {
      receiver: Rc::new(RefCell::new(handle.receiver)),
      port: Rc::new(handle.port),
      termination_signal: handle.termination_signal,
      has_terminated: handle.has_terminated,
      terminate_waker: handle.terminate_waker,
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
  termination_signal: Arc<AtomicBool>,
  has_terminated: Arc<AtomicBool>,
  terminate_waker: Arc<AtomicWaker>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorkerHandle {
  /// Get the WorkerEvent with lock
  /// Return error if more than one listener tries to get event
  #[allow(clippy::await_holding_refcell_ref)] // TODO(ry) remove!
  pub async fn get_control_event(
    &self,
  ) -> Result<Option<WorkerControlEvent>, AnyError> {
    let mut receiver = self.receiver.borrow_mut();
    Ok(receiver.next().await)
  }

  /// Terminate the worker
  /// This function will set the termination signal, close the message channel,
  /// and schedule to terminate the isolate after two seconds.
  pub fn terminate(self) {
    use std::thread::sleep;
    use std::thread::spawn;
    use std::time::Duration;

    let schedule_termination =
      !self.termination_signal.swap(true, Ordering::SeqCst);

    self.port.disentangle();

    if schedule_termination && !self.has_terminated.load(Ordering::SeqCst) {
      // Wake up the worker's event loop so it can terminate.
      self.terminate_waker.wake();

      let has_terminated = self.has_terminated.clone();

      // Schedule to terminate the isolate's execution.
      spawn(move || {
        sleep(Duration::from_secs(2));

        // A worker's isolate can only be terminated once, so we need a guard
        // here.
        let already_terminated = has_terminated.swap(true, Ordering::SeqCst);

        if !already_terminated {
          // Stop javascript execution
          self.isolate_handle.terminate_execution();
        }
      });
    }
  }
}

fn create_handles(
  isolate_handle: v8::IsolateHandle,
  name: String,
  worker_type: WebWorkerType,
) -> (WebWorkerInternalHandle, SendableWebWorkerHandle) {
  let (parent_port, worker_port) = create_entangled_message_port();
  let (ctrl_tx, ctrl_rx) = mpsc::channel::<WorkerControlEvent>(1);
  let termination_signal = Arc::new(AtomicBool::new(false));
  let has_terminated = Arc::new(AtomicBool::new(false));
  let terminate_waker = Arc::new(AtomicWaker::new());
  let internal_handle = WebWorkerInternalHandle {
    name,
    port: Rc::new(parent_port),
    termination_signal: termination_signal.clone(),
    has_terminated: has_terminated.clone(),
    terminate_waker: terminate_waker.clone(),
    isolate_handle: isolate_handle.clone(),
    cancel: CancelHandle::new_rc(),
    sender: ctrl_tx,
    worker_type,
  };
  let external_handle = SendableWebWorkerHandle {
    receiver: ctrl_rx,
    port: worker_port,
    termination_signal,
    has_terminated,
    terminate_waker,
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
  pub worker_type: WebWorkerType,
  pub main_module: ModuleSpecifier,
  poll_for_messages_fn: Option<v8::Global<v8::Value>>,
  bootstrap_fn_global: Option<v8::Global<v8::Function>>,
}

pub struct WebWorkerOptions {
  pub bootstrap: BootstrapOptions,
  pub extensions: Vec<Extension>,
  pub startup_snapshot: Option<Snapshot>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub seed: Option<u64>,
  pub fs: Arc<dyn FileSystem>,
  pub module_loader: Rc<dyn ModuleLoader>,
  pub npm_resolver: Option<Arc<dyn deno_node::NpmResolver>>,
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub format_js_error_fn: Option<Arc<FormatJsErrorFn>>,
  pub source_map_getter: Option<Box<dyn SourceMapGetter>>,
  pub worker_type: WebWorkerType,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub blob_store: Arc<BlobStore>,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  pub cache_storage_dir: Option<std::path::PathBuf>,
  pub stdio: Stdio,
  pub feature_checker: Arc<FeatureChecker>,
}

impl WebWorker {
  pub fn bootstrap_from_options(
    name: String,
    permissions: PermissionsContainer,
    main_module: ModuleSpecifier,
    worker_id: WorkerId,
    options: WebWorkerOptions,
  ) -> (Self, SendableWebWorkerHandle) {
    let bootstrap_options = options.bootstrap.clone();
    let (mut worker, handle) =
      Self::from_options(name, permissions, main_module, worker_id, options);
    worker.bootstrap(&bootstrap_options);
    (worker, handle)
  }

  pub fn from_options(
    name: String,
    permissions: PermissionsContainer,
    main_module: ModuleSpecifier,
    worker_id: WorkerId,
    mut options: WebWorkerOptions,
  ) -> (Self, SendableWebWorkerHandle) {
    deno_core::extension!(deno_permissions_web_worker,
      options = {
        permissions: PermissionsContainer,
        enable_testing_features: bool,
      },
      state = |state, options| {
        state.put::<PermissionsContainer>(options.permissions);
        state.put(ops::TestingFeaturesEnabled(options.enable_testing_features));
      },
    );

    // Permissions: many ops depend on this
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let create_cache = options.cache_storage_dir.map(|storage_dir| {
      let create_cache_fn = move || SqliteBackedCache::new(storage_dir.clone());
      CreateCache(Arc::new(create_cache_fn))
    });

    // NOTE(bartlomieju): ordering is important here, keep it in sync with
    // `runtime/build.rs`, `runtime/worker.rs` and `runtime/snapshot.rs`!

    let mut extensions = vec![
      // Web APIs
      deno_webidl::deno_webidl::init_ops_and_esm(),
      deno_console::deno_console::init_ops_and_esm(),
      deno_url::deno_url::init_ops_and_esm(),
      deno_web::deno_web::init_ops_and_esm::<PermissionsContainer>(
        options.blob_store.clone(),
        Some(main_module.clone()),
      ),
      deno_webgpu::deno_webgpu::init_ops_and_esm(),
      deno_fetch::deno_fetch::init_ops_and_esm::<PermissionsContainer>(
        deno_fetch::Options {
          user_agent: options.bootstrap.user_agent.clone(),
          root_cert_store_provider: options.root_cert_store_provider.clone(),
          unsafely_ignore_certificate_errors: options
            .unsafely_ignore_certificate_errors
            .clone(),
          file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
          ..Default::default()
        },
      ),
      deno_cache::deno_cache::init_ops_and_esm::<SqliteBackedCache>(
        create_cache,
      ),
      deno_websocket::deno_websocket::init_ops_and_esm::<PermissionsContainer>(
        options.bootstrap.user_agent.clone(),
        options.root_cert_store_provider.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_webstorage::deno_webstorage::init_ops_and_esm(None).disable(),
      deno_crypto::deno_crypto::init_ops_and_esm(options.seed),
      deno_broadcast_channel::deno_broadcast_channel::init_ops_and_esm(
        options.broadcast_channel.clone(),
      ),
      deno_ffi::deno_ffi::init_ops_and_esm::<PermissionsContainer>(),
      deno_net::deno_net::init_ops_and_esm::<PermissionsContainer>(
        options.root_cert_store_provider.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_tls::deno_tls::init_ops_and_esm(),
      deno_kv::deno_kv::init_ops_and_esm(
        MultiBackendDbHandler::remote_or_sqlite::<PermissionsContainer>(
          None,
          options.seed,
          deno_kv::remote::HttpOptions {
            user_agent: options.bootstrap.user_agent.clone(),
            root_cert_store_provider: options.root_cert_store_provider.clone(),
            unsafely_ignore_certificate_errors: options
              .unsafely_ignore_certificate_errors
              .clone(),
            client_cert_chain_and_key: None,
            proxy: None,
          },
        ),
      ),
      deno_cron::deno_cron::init_ops_and_esm(LocalCronHandler::new()),
      deno_napi::deno_napi::init_ops_and_esm::<PermissionsContainer>(),
      deno_http::deno_http::init_ops_and_esm::<DefaultHttpPropertyExtractor>(),
      deno_io::deno_io::init_ops_and_esm(Some(options.stdio)),
      deno_fs::deno_fs::init_ops_and_esm::<PermissionsContainer>(
        options.fs.clone(),
      ),
      deno_node::deno_node::init_ops_and_esm::<PermissionsContainer>(
        options.npm_resolver,
        options.fs,
      ),
      // Runtime ops that are always initialized for WebWorkers
      ops::runtime::deno_runtime::init_ops_and_esm(main_module.clone()),
      ops::worker_host::deno_worker_host::init_ops_and_esm(
        options.create_web_worker_cb.clone(),
        options.format_js_error_fn.clone(),
      ),
      ops::fs_events::deno_fs_events::init_ops_and_esm(),
      ops::os::deno_os_worker::init_ops_and_esm(),
      ops::permissions::deno_permissions::init_ops_and_esm(),
      ops::process::deno_process::init_ops_and_esm(),
      ops::signal::deno_signal::init_ops_and_esm(),
      ops::tty::deno_tty::init_ops_and_esm(),
      ops::http::deno_http_runtime::init_ops_and_esm(),
      ops::bootstrap::deno_bootstrap::init_ops_and_esm(None),
      deno_permissions_web_worker::init_ops_and_esm(
        permissions,
        enable_testing_features,
      ),
      runtime::init_ops_and_esm(),
      ops::web_worker::deno_web_worker::init_ops_and_esm(),
    ];

    for extension in &mut extensions {
      #[cfg(not(feature = "__runtime_js_sources"))]
      {
        extension.js_files = std::borrow::Cow::Borrowed(&[]);
        extension.esm_files = std::borrow::Cow::Borrowed(&[]);
        extension.esm_entry_point = None;
      }
      #[cfg(feature = "__runtime_js_sources")]
      {
        use crate::shared::maybe_transpile_source;
        for source in extension.esm_files.to_mut() {
          maybe_transpile_source(source).unwrap();
        }
        for source in extension.js_files.to_mut() {
          maybe_transpile_source(source).unwrap();
        }
      }
    }

    extensions.extend(std::mem::take(&mut options.extensions));

    #[cfg(all(feature = "include_js_files_for_snapshotting", feature = "dont_create_runtime_snapshot", not(feature = "__runtime_js_sources")))]
    options.startup_snapshot.as_ref().expect("Sources are not embedded, snapshotting was disabled and a user snapshot was not provided.");

    // Hook up the summary metrics if the user or subcommand requested them
    let (op_summary_metrics, op_metrics_factory_fn) =
      if options.bootstrap.enable_op_summary_metrics {
        let op_summary_metrics = Rc::new(OpMetricsSummaryTracker::default());
        (
          Some(op_summary_metrics.clone()),
          Some(op_summary_metrics.op_metrics_factory_fn(|_| true)),
        )
      } else {
        (None, None)
      };

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: options
        .startup_snapshot
        .or_else(crate::js::deno_isolate_init),
      source_map_getter: options.source_map_getter,
      get_error_class_fn: options.get_error_class_fn,
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
      extensions,
      inspector: options.maybe_inspector_server.is_some(),
      feature_checker: Some(options.feature_checker.clone()),
      op_metrics_factory_fn,
      import_meta_resolve_callback: Some(Box::new(
        import_meta_resolve_callback,
      )),
      ..Default::default()
    });

    if let Some(op_summary_metrics) = op_summary_metrics {
      js_runtime.op_state().borrow_mut().put(op_summary_metrics);
    }

    if let Some(server) = options.maybe_inspector_server.clone() {
      server.register_inspector(
        main_module.to_string(),
        &mut js_runtime,
        false,
      );

      // Put inspector handle into the op state so we can put a breakpoint when
      // executing a CJS entrypoint.
      let op_state = js_runtime.op_state();
      let inspector = js_runtime.inspector();
      op_state.borrow_mut().put(inspector);
    }

    let (internal_handle, external_handle) = {
      let handle = js_runtime.v8_isolate().thread_safe_handle();
      let (internal_handle, external_handle) =
        create_handles(handle, name.clone(), options.worker_type);
      let op_state = js_runtime.op_state();
      let mut op_state = op_state.borrow_mut();
      op_state.put(internal_handle.clone());
      (internal_handle, external_handle)
    };

    let bootstrap_fn_global = {
      let context = js_runtime.main_context();
      let scope = &mut js_runtime.handle_scope();
      let context_local = v8::Local::new(scope, context);
      let global_obj = context_local.global(scope);
      let bootstrap_str =
        v8::String::new_external_onebyte_static(scope, b"bootstrap").unwrap();
      let bootstrap_ns: v8::Local<v8::Object> = global_obj
        .get(scope, bootstrap_str.into())
        .unwrap()
        .try_into()
        .unwrap();
      let main_runtime_str =
        v8::String::new_external_onebyte_static(scope, b"workerRuntime")
          .unwrap();
      let bootstrap_fn =
        bootstrap_ns.get(scope, main_runtime_str.into()).unwrap();
      let bootstrap_fn =
        v8::Local::<v8::Function>::try_from(bootstrap_fn).unwrap();
      v8::Global::new(scope, bootstrap_fn)
    };

    (
      Self {
        id: worker_id,
        js_runtime,
        name,
        internal_handle,
        worker_type: options.worker_type,
        main_module,
        poll_for_messages_fn: None,
        bootstrap_fn_global: Some(bootstrap_fn_global),
      },
      external_handle,
    )
  }

  pub fn bootstrap(&mut self, options: &BootstrapOptions) {
    self.js_runtime.op_state().borrow_mut().put(options.clone());
    // Instead of using name for log we use `worker-${id}` because
    // WebWorkers can have empty string as name.
    {
      let scope = &mut self.js_runtime.handle_scope();
      let args = options.as_v8(scope);
      let bootstrap_fn = self.bootstrap_fn_global.take().unwrap();
      let bootstrap_fn = v8::Local::new(scope, bootstrap_fn);
      let undefined = v8::undefined(scope);
      let name_str: v8::Local<v8::Value> =
        v8::String::new(scope, &self.name).unwrap().into();
      let id_str: v8::Local<v8::Value> =
        v8::String::new(scope, &format!("{}", self.id))
          .unwrap()
          .into();
      bootstrap_fn
        .call(scope, undefined.into(), &[args, name_str, id_str])
        .unwrap();
    }
    // TODO(bartlomieju): this could be done using V8 API, without calling `execute_script`.
    // Save a reference to function that will start polling for messages
    // from a worker host; it will be called after the user code is loaded.
    let script = ascii_str!(
      r#"
    const pollForMessages = globalThis.pollForMessages;
    delete globalThis.pollForMessages;
    pollForMessages
    "#
    );
    let poll_for_messages_fn = self
      .js_runtime
      .execute_script(located_script_name!(), script)
      .expect("Failed to execute worker bootstrap script");
    self.poll_for_messages_fn = Some(poll_for_messages_fn);
  }

  /// See [JsRuntime::execute_script](deno_core::JsRuntime::execute_script)
  pub fn execute_script(
    &mut self,
    name: &'static str,
    source_code: ModuleCode,
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(name, source_code)?;
    Ok(())
  }

  /// Loads and instantiates specified JavaScript module as "main" module.
  pub async fn preload_main_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, AnyError> {
    self
      .js_runtime
      .load_main_module(module_specifier, None)
      .await
  }

  /// Loads and instantiates specified JavaScript module as "side" module.
  pub async fn preload_side_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, AnyError> {
    self
      .js_runtime
      .load_side_module(module_specifier, None)
      .await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  ///
  /// This method assumes that worker can't be terminated when executing
  /// side module code.
  pub async fn execute_side_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_side_module(module_specifier).await?;
    let mut receiver = self.js_runtime.mod_evaluate(id);
    tokio::select! {
      biased;

      maybe_result = &mut receiver => {
        debug!("received module evaluate {:#?}", maybe_result);
        maybe_result
      }

      event_loop_result = self.js_runtime.run_event_loop(PollEventLoopOptions::default()) => {
        event_loop_result?;
        receiver.await
      }
    }
  }

  /// Loads, instantiates and executes specified JavaScript module.
  ///
  /// This module will have "import.meta.main" equal to true.
  pub async fn execute_main_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), AnyError> {
    let mut receiver = self.js_runtime.mod_evaluate(id);
    let poll_options = PollEventLoopOptions::default();

    tokio::select! {
      biased;

      maybe_result = &mut receiver => {
        debug!("received worker module evaluate {:#?}", maybe_result);
        maybe_result
      }

      event_loop_result = self.run_event_loop(poll_options) => {
        if self.internal_handle.is_terminated() {
           return Ok(());
        }
        event_loop_result?;
        receiver.await
      }
    }
  }

  fn poll_event_loop(
    &mut self,
    cx: &mut Context,
    poll_options: PollEventLoopOptions,
  ) -> Poll<Result<(), AnyError>> {
    // If awakened because we are terminating, just return Ok
    if self.internal_handle.terminate_if_needed() {
      return Poll::Ready(Ok(()));
    }

    self.internal_handle.terminate_waker.register(cx.waker());

    match self.js_runtime.poll_event_loop(cx, poll_options) {
      Poll::Ready(r) => {
        // If js ended because we are terminating, just return Ok
        if self.internal_handle.terminate_if_needed() {
          return Poll::Ready(Ok(()));
        }

        if let Err(e) = r {
          return Poll::Ready(Err(e));
        }

        // TODO(mmastrac): we don't want to test this w/classic workers because
        // WPT triggers a failure here. This is only exposed via --enable-testing-features-do-not-use.
        if self.worker_type == WebWorkerType::Module {
          panic!(
            "coding error: either js is polling or the worker is terminated"
          );
        } else {
          eprintln!("classic worker terminated unexpectedly");
          Poll::Ready(Ok(()))
        }
      }
      Poll::Pending => Poll::Pending,
    }
  }

  pub async fn run_event_loop(
    &mut self,
    poll_options: PollEventLoopOptions,
  ) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx, poll_options)).await
  }

  // Starts polling for messages from worker host from JavaScript.
  fn start_polling_for_messages(&mut self) {
    let poll_for_messages_fn = self.poll_for_messages_fn.take().unwrap();
    let scope = &mut self.js_runtime.handle_scope();
    let poll_for_messages =
      v8::Local::<v8::Value>::new(scope, poll_for_messages_fn);
    let fn_ = v8::Local::<v8::Function>::try_from(poll_for_messages).unwrap();
    let undefined = v8::undefined(scope);
    // This call may return `None` if worker is terminated.
    fn_.call(scope, undefined.into(), &[]);
  }
}

fn print_worker_error(
  error: &AnyError,
  name: &str,
  format_js_error_fn: Option<&FormatJsErrorFn>,
) {
  let error_str = match format_js_error_fn {
    Some(format_js_error_fn) => match error.downcast_ref::<JsError>() {
      Some(js_error) => format_js_error_fn(js_error),
      None => error.to_string(),
    },
    None => error.to_string(),
  };
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
  mut maybe_source_code: Option<String>,
  format_js_error_fn: Option<Arc<FormatJsErrorFn>>,
) -> Result<(), AnyError> {
  let name = worker.name.to_string();

  // TODO(bartlomieju): run following block using "select!"
  // with terminate

  let fut = async move {
    let internal_handle = worker.internal_handle.clone();

    // Execute provided source code immediately
    let result = if let Some(source_code) = maybe_source_code.take() {
      let r = worker.execute_script(located_script_name!(), source_code.into());
      worker.start_polling_for_messages();
      r
    } else {
      // TODO(bartlomieju): add "type": "classic", ie. ability to load
      // script instead of module
      match worker.preload_main_module(&specifier).await {
        Ok(id) => {
          worker.start_polling_for_messages();
          worker.execute_main_module(id).await
        }
        Err(e) => Err(e),
      }
    };

    // If sender is closed it means that worker has already been closed from
    // within using "globalThis.close()"
    if internal_handle.is_terminated() {
      return Ok(());
    }

    let result = if result.is_ok() {
      worker
        .run_event_loop(PollEventLoopOptions {
          wait_for_inspector: true,
          ..Default::default()
        })
        .await
    } else {
      result
    };

    if let Err(e) = result {
      print_worker_error(&e, &name, format_js_error_fn.as_deref());
      internal_handle
        .post_event(WorkerControlEvent::TerminalError(e))
        .expect("Failed to post message to host");

      // Failure to execute script is a terminal error, bye, bye.
      return Ok(());
    }

    debug!("Worker thread shuts down {}", &name);
    result
  };
  create_and_run_current_thread(fut)
}
