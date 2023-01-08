// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_cache::CreateCache;
use deno_cache::SqliteBackedCache;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::Future;
use deno_core::located_script_name;
use deno_core::v8;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::FsModuleLoader;
use deno_core::GetErrorClassFn;
use deno_core::JsRuntime;
use deno_core::LocalInspectorSession;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_core::Snapshot;
use deno_core::SourceMapGetter;
use deno_node::RequireNpmResolver;
use deno_tls::rustls::RootCertStore;
use deno_web::BlobStore;
use log::debug;

use crate::inspector_server::InspectorServer;
use crate::js;
use crate::ops;
use crate::ops::io::Stdio;
use crate::permissions::PermissionsContainer;
use crate::BootstrapOptions;

pub type FormatJsErrorFn = dyn Fn(&JsError) -> String + Sync + Send;

#[derive(Clone, Default)]
pub struct ExitCode(Arc<AtomicI32>);

impl ExitCode {
  pub fn get(&self) -> i32 {
    self.0.load(Relaxed)
  }

  pub fn set(&mut self, code: i32) {
    self.0.store(code, Relaxed);
  }
}
/// This worker is created and used by almost all
/// subcommands in Deno executable.
///
/// It provides ops available in the `Deno` namespace.
///
/// All `WebWorker`s created during program execution
/// are descendants of this worker.
pub struct MainWorker {
  pub js_runtime: JsRuntime,
  should_break_on_first_statement: bool,
  should_wait_for_inspector_session: bool,
  exit_code: ExitCode,
}

pub struct WorkerOptions {
  pub bootstrap: BootstrapOptions,

  /// JsRuntime extensions, not to be confused with ES modules.
  /// Only ops registered by extensions will be initialized. If you need
  /// to execute JS code from extensions, use `extensions_with_js` options
  /// instead.
  pub extensions: Vec<Extension>,

  /// JsRuntime extensions, not to be confused with ES modules.
  /// Ops registered by extensions will be initialized and JS code will be
  /// executed. If you don't need to execute JS code from extensions, use
  /// `extensions` option instead.
  ///
  /// This is useful when creating snapshots, in such case you would pass
  /// extensions using `extensions_with_js`, later when creating a runtime
  /// from the snapshot, you would pass these extensions using `extensions`
  /// option.
  pub extensions_with_js: Vec<Extension>,

  /// V8 snapshot that should be loaded on startup.
  pub startup_snapshot: Option<Snapshot>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub root_cert_store: Option<RootCertStore>,
  pub seed: Option<u64>,

  /// Implementation of `ModuleLoader` which will be
  /// called when V8 requests to load ES modules.
  ///
  /// If not provided runtime will error if code being
  /// executed tries to load modules.
  pub module_loader: Rc<dyn ModuleLoader>,
  pub npm_resolver: Option<Rc<dyn RequireNpmResolver>>,
  // Callbacks invoked when creating new instance of WebWorker
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub web_worker_preload_module_cb: Arc<ops::worker_host::WorkerEventCb>,
  pub web_worker_pre_execute_module_cb: Arc<ops::worker_host::WorkerEventCb>,
  pub format_js_error_fn: Option<Arc<FormatJsErrorFn>>,

  /// Source map reference for errors.
  pub source_map_getter: Option<Box<dyn SourceMapGetter>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  // If true, the worker will wait for inspector session and break on first
  // statement of user code. Takes higher precedence than
  // `should_wait_for_inspector_session`.
  pub should_break_on_first_statement: bool,
  // If true, the worker will wait for inspector session before executing
  // user code.
  pub should_wait_for_inspector_session: bool,

  /// Allows to map error type to a string "class" used to represent
  /// error in JavaScript.
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub cache_storage_dir: Option<std::path::PathBuf>,
  pub origin_storage_dir: Option<std::path::PathBuf>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,

  /// The store to use for transferring SharedArrayBuffers between isolates.
  /// If multiple isolates should have the possibility of sharing
  /// SharedArrayBuffers, they should use the same [SharedArrayBufferStore]. If
  /// no [SharedArrayBufferStore] is specified, SharedArrayBuffer can not be
  /// serialized.
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,

  /// The store to use for transferring `WebAssembly.Module` objects between
  /// isolates.
  /// If multiple isolates should have the possibility of sharing
  /// `WebAssembly.Module` objects, they should use the same
  /// [CompiledWasmModuleStore]. If no [CompiledWasmModuleStore] is specified,
  /// `WebAssembly.Module` objects cannot be serialized.
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  pub stdio: Stdio,
}

impl Default for WorkerOptions {
  fn default() -> Self {
    Self {
      web_worker_preload_module_cb: Arc::new(|_| {
        unimplemented!("web workers are not supported")
      }),
      web_worker_pre_execute_module_cb: Arc::new(|_| {
        unimplemented!("web workers are not supported")
      }),
      create_web_worker_cb: Arc::new(|_| {
        unimplemented!("web workers are not supported")
      }),
      module_loader: Rc::new(FsModuleLoader),
      seed: None,
      unsafely_ignore_certificate_errors: Default::default(),
      should_break_on_first_statement: Default::default(),
      should_wait_for_inspector_session: Default::default(),
      compiled_wasm_module_store: Default::default(),
      shared_array_buffer_store: Default::default(),
      maybe_inspector_server: Default::default(),
      format_js_error_fn: Default::default(),
      get_error_class_fn: Default::default(),
      origin_storage_dir: Default::default(),
      cache_storage_dir: Default::default(),
      broadcast_channel: Default::default(),
      source_map_getter: Default::default(),
      root_cert_store: Default::default(),
      npm_resolver: Default::default(),
      blob_store: Default::default(),
      extensions: Default::default(),
      extensions_with_js: Default::default(),
      startup_snapshot: Default::default(),
      bootstrap: Default::default(),
      stdio: Default::default(),
    }
  }
}

impl MainWorker {
  pub fn bootstrap_from_options(
    main_module: ModuleSpecifier,
    permissions: PermissionsContainer,
    options: WorkerOptions,
  ) -> Self {
    let bootstrap_options = options.bootstrap.clone();
    let mut worker = Self::from_options(main_module, permissions, options);
    worker.bootstrap(&bootstrap_options);
    worker
  }

  pub fn from_options(
    main_module: ModuleSpecifier,
    permissions: PermissionsContainer,
    mut options: WorkerOptions,
  ) -> Self {
    // Permissions: many ops depend on this
    let unstable = options.bootstrap.unstable;
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let perm_ext = Extension::builder("deno_permissions_worker")
      .state(move |state| {
        state.put::<PermissionsContainer>(permissions.clone());
        state.put(ops::UnstableChecker { unstable });
        state.put(ops::TestingFeaturesEnabled(enable_testing_features));
        Ok(())
      })
      .build();
    let exit_code = ExitCode(Arc::new(AtomicI32::new(0)));
    let create_cache = options.cache_storage_dir.map(|storage_dir| {
      let create_cache_fn = move || SqliteBackedCache::new(storage_dir.clone());
      CreateCache(Arc::new(create_cache_fn))
    });

    // Internal modules
    let mut extensions: Vec<Extension> = vec![
      // Web APIs
      deno_webidl::init(),
      deno_console::init(),
      deno_url::init(),
      deno_web::init::<PermissionsContainer>(
        options.blob_store.clone(),
        options.bootstrap.location.clone(),
      ),
      deno_fetch::init::<PermissionsContainer>(deno_fetch::Options {
        user_agent: options.bootstrap.user_agent.clone(),
        root_cert_store: options.root_cert_store.clone(),
        unsafely_ignore_certificate_errors: options
          .unsafely_ignore_certificate_errors
          .clone(),
        file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
        ..Default::default()
      }),
      deno_cache::init::<SqliteBackedCache>(create_cache),
      deno_websocket::init::<PermissionsContainer>(
        options.bootstrap.user_agent.clone(),
        options.root_cert_store.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_webstorage::init(options.origin_storage_dir.clone()),
      deno_broadcast_channel::init(options.broadcast_channel.clone(), unstable),
      deno_crypto::init(options.seed),
      deno_webgpu::init(unstable),
      // ffi
      deno_ffi::init::<PermissionsContainer>(unstable),
      // Runtime ops
      ops::runtime::init(main_module.clone()),
      ops::worker_host::init(
        options.create_web_worker_cb.clone(),
        options.web_worker_preload_module_cb.clone(),
        options.web_worker_pre_execute_module_cb.clone(),
        options.format_js_error_fn.clone(),
      ),
      ops::spawn::init(),
      ops::fs_events::init(),
      ops::fs::init(),
      ops::io::init(),
      ops::io::init_stdio(options.stdio),
      deno_tls::init(),
      deno_net::init::<PermissionsContainer>(
        options.root_cert_store.clone(),
        unstable,
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_napi::init::<PermissionsContainer>(unstable),
      deno_node::init::<PermissionsContainer>(options.npm_resolver),
      ops::os::init(exit_code.clone()),
      ops::permissions::init(),
      ops::process::init(),
      ops::signal::init(),
      ops::tty::init(),
      deno_http::init(),
      deno_flash::init::<PermissionsContainer>(unstable),
      ops::http::init(),
      // Permissions ext (worker specific state)
      perm_ext,
    ];
    extensions.extend(std::mem::take(&mut options.extensions));

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(
        options
          .startup_snapshot
          .unwrap_or_else(js::deno_isolate_init),
      ),
      source_map_getter: options.source_map_getter,
      get_error_class_fn: options.get_error_class_fn,
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
      extensions,
      extensions_with_js: options.extensions_with_js,
      inspector: options.maybe_inspector_server.is_some(),
      is_main: true,
      ..Default::default()
    });

    if let Some(server) = options.maybe_inspector_server.clone() {
      server.register_inspector(
        main_module.to_string(),
        &mut js_runtime,
        options.should_break_on_first_statement
          || options.should_wait_for_inspector_session,
      );

      // Put inspector handle into the op state so we can put a breakpoint when
      // executing a CJS entrypoint.
      let op_state = js_runtime.op_state();
      let inspector = js_runtime.inspector();
      op_state.borrow_mut().put(inspector);
    }

    Self {
      js_runtime,
      should_break_on_first_statement: options.should_break_on_first_statement,
      should_wait_for_inspector_session: options
        .should_wait_for_inspector_session,
      exit_code,
    }
  }

  pub fn bootstrap(&mut self, options: &BootstrapOptions) {
    let script = format!("bootstrap.mainRuntime({})", options.as_json());
    self
      .execute_script(&located_script_name!(), &script)
      .expect("Failed to execute bootstrap script");
  }

  /// See [JsRuntime::execute_script](deno_core::JsRuntime::execute_script)
  pub fn execute_script(
    &mut self,
    script_name: &str,
    source_code: &str,
  ) -> Result<v8::Global<v8::Value>, AnyError> {
    self.js_runtime.execute_script(script_name, source_code)
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

  /// Executes specified JavaScript module.
  pub async fn evaluate_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), AnyError> {
    self.wait_for_inspector_session();
    let mut receiver = self.js_runtime.mod_evaluate(id);
    tokio::select! {
      // Not using biased mode leads to non-determinism for relatively simple
      // programs.
      biased;

      maybe_result = &mut receiver => {
        debug!("received module evaluate {:#?}", maybe_result);
        maybe_result.expect("Module evaluation result not provided.")
      }

      event_loop_result = self.run_event_loop(false) => {
        event_loop_result?;
        let maybe_result = receiver.await;
        maybe_result.expect("Module evaluation result not provided.")
      }
    }
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_side_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_side_module(module_specifier).await?;
    self.evaluate_module(id).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  ///
  /// This module will have "import.meta.main" equal to true.
  pub async fn execute_main_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_main_module(module_specifier).await?;
    self.evaluate_module(id).await
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .js_runtime
        .inspector()
        .borrow_mut()
        .wait_for_session_and_break_on_next_statement();
    } else if self.should_wait_for_inspector_session {
      self.js_runtime.inspector().borrow_mut().wait_for_session();
    }
  }

  /// Create new inspector session. This function panics if Worker
  /// was not configured to create inspector.
  pub async fn create_inspector_session(&mut self) -> LocalInspectorSession {
    self.js_runtime.maybe_init_inspector();
    self.js_runtime.inspector().borrow().create_local_session()
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
    wait_for_inspector: bool,
  ) -> Poll<Result<(), AnyError>> {
    self.js_runtime.poll_event_loop(cx, wait_for_inspector)
  }

  pub async fn run_event_loop(
    &mut self,
    wait_for_inspector: bool,
  ) -> Result<(), AnyError> {
    self.js_runtime.run_event_loop(wait_for_inspector).await
  }

  /// A utility function that runs provided future concurrently with the event loop.
  ///
  /// Useful when using a local inspector session.
  pub async fn with_event_loop<'a, T>(
    &mut self,
    mut fut: Pin<Box<dyn Future<Output = T> + 'a>>,
  ) -> T {
    loop {
      tokio::select! {
        biased;
        result = &mut fut => {
          return result;
        }
        _ = self.run_event_loop(false) => {}
      };
    }
  }

  /// Return exit code set by the executed code (either in main worker
  /// or one of child web workers).
  pub fn exit_code(&self) -> i32 {
    self.exit_code.get()
  }

  /// Dispatches "load" event to the JavaScript runtime.
  ///
  /// Does not poll event loop, and thus not await any of the "load" event handlers.
  pub fn dispatch_load_event(
    &mut self,
    script_name: &str,
  ) -> Result<(), AnyError> {
    self.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      "dispatchEvent(new Event('load'))",
    )?;
    Ok(())
  }

  /// Dispatches "unload" event to the JavaScript runtime.
  ///
  /// Does not poll event loop, and thus not await any of the "unload" event handlers.
  pub fn dispatch_unload_event(
    &mut self,
    script_name: &str,
  ) -> Result<(), AnyError> {
    self.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      "dispatchEvent(new Event('unload'))",
    )?;
    Ok(())
  }

  /// Dispatches "beforeunload" event to the JavaScript runtime. Returns a boolean
  /// indicating if the event was prevented and thus event loop should continue
  /// running.
  pub fn dispatch_beforeunload_event(
    &mut self,
    script_name: &str,
  ) -> Result<bool, AnyError> {
    let value = self.js_runtime.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      "dispatchEvent(new Event('beforeunload', { cancelable: true }));",
    )?;
    let local_value = value.open(&mut self.js_runtime.handle_scope());
    Ok(local_value.is_false())
  }
}
