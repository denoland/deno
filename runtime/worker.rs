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
use deno_core::ascii_str;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::Future;
use deno_core::v8;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::FsModuleLoader;
use deno_core::GetErrorClassFn;
use deno_core::JsRuntime;
use deno_core::LocalInspectorSession;
use deno_core::ModuleCode;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_core::Snapshot;
use deno_core::SourceMapGetter;
use deno_fs::StdFs;
use deno_io::Stdio;
use deno_kv::sqlite::SqliteDbHandler;
use deno_node::RequireNpmResolver;
use deno_tls::rustls::RootCertStore;
use deno_web::BlobStore;
use log::debug;

use crate::inspector_server::InspectorServer;
use crate::ops;
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
  bootstrap_fn_global: Option<v8::Global<v8::Function>>,
}

pub struct WorkerOptions {
  pub bootstrap: BootstrapOptions,

  /// JsRuntime extensions, not to be confused with ES modules.
  ///
  /// Extensions register "ops" and JavaScript sources provided in `js` or `esm`
  /// configuration. If you are using a snapshot, then extensions shouldn't
  /// provide JavaScript sources that were already snapshotted.
  pub extensions: Vec<Extension>,

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
    deno_core::extension!(deno_permissions_worker,
      options = {
        permissions: PermissionsContainer,
        unstable: bool,
        enable_testing_features: bool,
      },
      state = |state, options| {
        state.put::<PermissionsContainer>(options.permissions);
        state.put(ops::UnstableChecker { unstable: options.unstable });
        state.put(ops::TestingFeaturesEnabled(options.enable_testing_features));
      },
    );

    // Permissions: many ops depend on this
    let unstable = options.bootstrap.unstable;
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let exit_code = ExitCode(Arc::new(AtomicI32::new(0)));
    let create_cache = options.cache_storage_dir.map(|storage_dir| {
      let create_cache_fn = move || SqliteBackedCache::new(storage_dir.clone());
      CreateCache(Arc::new(create_cache_fn))
    });

    // NOTE(bartlomieju): ordering is important here, keep it in sync with
    // `runtime/build.rs`, `runtime/web_worker.rs` and `cli/build.rs`!
    let mut extensions = vec![
      // Web APIs
      deno_webidl::deno_webidl::init_ops(),
      deno_console::deno_console::init_ops(),
      deno_url::deno_url::init_ops(),
      deno_web::deno_web::init_ops::<PermissionsContainer>(
        options.blob_store.clone(),
        options.bootstrap.location.clone(),
      ),
      deno_fetch::deno_fetch::init_ops::<PermissionsContainer>(
        deno_fetch::Options {
          user_agent: options.bootstrap.user_agent.clone(),
          root_cert_store: options.root_cert_store.clone(),
          unsafely_ignore_certificate_errors: options
            .unsafely_ignore_certificate_errors
            .clone(),
          file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
          ..Default::default()
        },
      ),
      deno_cache::deno_cache::init_ops::<SqliteBackedCache>(create_cache),
      deno_websocket::deno_websocket::init_ops::<PermissionsContainer>(
        options.bootstrap.user_agent.clone(),
        options.root_cert_store.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_webstorage::deno_webstorage::init_ops(
        options.origin_storage_dir.clone(),
      ),
      deno_crypto::deno_crypto::init_ops(options.seed),
      deno_broadcast_channel::deno_broadcast_channel::init_ops(
        options.broadcast_channel.clone(),
        unstable,
      ),
      deno_ffi::deno_ffi::init_ops::<PermissionsContainer>(unstable),
      deno_net::deno_net::init_ops::<PermissionsContainer>(
        options.root_cert_store.clone(),
        unstable,
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_tls::deno_tls::init_ops(),
      deno_kv::deno_kv::init_ops(
        SqliteDbHandler::<PermissionsContainer>::new(
          options.origin_storage_dir.clone(),
        ),
        unstable,
      ),
      deno_napi::deno_napi::init_ops::<PermissionsContainer>(),
      deno_http::deno_http::init_ops(),
      deno_io::deno_io::init_ops(Some(options.stdio)),
      deno_fs::deno_fs::init_ops::<_, PermissionsContainer>(unstable, StdFs),
      deno_node::deno_node::init_ops::<crate::RuntimeNodeEnv>(
        options.npm_resolver,
      ),
      // Ops from this crate
      ops::runtime::deno_runtime::init_ops(main_module.clone()),
      ops::worker_host::deno_worker_host::init_ops(
        options.create_web_worker_cb.clone(),
        options.web_worker_preload_module_cb.clone(),
        options.web_worker_pre_execute_module_cb.clone(),
        options.format_js_error_fn.clone(),
      ),
      ops::fs_events::deno_fs_events::init_ops(),
      ops::os::deno_os::init_ops(exit_code.clone()),
      ops::permissions::deno_permissions::init_ops(),
      ops::process::deno_process::init_ops(),
      ops::signal::deno_signal::init_ops(),
      ops::tty::deno_tty::init_ops(),
      ops::http::deno_http_runtime::init_ops(),
      deno_permissions_worker::init_ops(
        permissions,
        unstable,
        enable_testing_features,
      ),
    ];

    extensions.extend(std::mem::take(&mut options.extensions));

    #[cfg(not(feature = "dont_create_runtime_snapshot"))]
    let startup_snapshot = options
      .startup_snapshot
      .unwrap_or_else(crate::js::deno_isolate_init);
    #[cfg(feature = "dont_create_runtime_snapshot")]
    let startup_snapshot = options.startup_snapshot
      .expect("deno_runtime startup snapshot is not available with 'create_runtime_snapshot' Cargo feature.");

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(startup_snapshot),
      source_map_getter: options.source_map_getter,
      get_error_class_fn: options.get_error_class_fn,
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
      extensions,
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

    let bootstrap_fn_global = {
      let context = js_runtime.global_context();
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
        v8::String::new_external_onebyte_static(scope, b"mainRuntime").unwrap();
      let bootstrap_fn =
        bootstrap_ns.get(scope, main_runtime_str.into()).unwrap();
      let bootstrap_fn =
        v8::Local::<v8::Function>::try_from(bootstrap_fn).unwrap();
      v8::Global::new(scope, bootstrap_fn)
    };

    Self {
      js_runtime,
      should_break_on_first_statement: options.should_break_on_first_statement,
      should_wait_for_inspector_session: options
        .should_wait_for_inspector_session,
      exit_code,
      bootstrap_fn_global: Some(bootstrap_fn_global),
    }
  }

  pub fn bootstrap(&mut self, options: &BootstrapOptions) {
    let scope = &mut self.js_runtime.handle_scope();
    let args = options.as_v8(scope);
    let bootstrap_fn = self.bootstrap_fn_global.take().unwrap();
    let bootstrap_fn = v8::Local::new(scope, bootstrap_fn);
    let undefined = v8::undefined(scope);
    bootstrap_fn
      .call(scope, undefined.into(), &[args.into()])
      .unwrap();
  }

  /// See [JsRuntime::execute_script](deno_core::JsRuntime::execute_script)
  pub fn execute_script(
    &mut self,
    script_name: &'static str,
    source_code: ModuleCode,
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
    script_name: &'static str,
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      ascii_str!("dispatchEvent(new Event('load'))"),
    )?;
    Ok(())
  }

  /// Dispatches "unload" event to the JavaScript runtime.
  ///
  /// Does not poll event loop, and thus not await any of the "unload" event handlers.
  pub fn dispatch_unload_event(
    &mut self,
    script_name: &'static str,
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      ascii_str!("dispatchEvent(new Event('unload'))"),
    )?;
    Ok(())
  }

  /// Dispatches "beforeunload" event to the JavaScript runtime. Returns a boolean
  /// indicating if the event was prevented and thus event loop should continue
  /// running.
  pub fn dispatch_beforeunload_event(
    &mut self,
    script_name: &'static str,
  ) -> Result<bool, AnyError> {
    let value = self.js_runtime.execute_script(
      script_name,
      // NOTE(@bartlomieju): not using `globalThis` here, because user might delete
      // it. Instead we're using global `dispatchEvent` function which will
      // used a saved reference to global scope.
      ascii_str!(
        "dispatchEvent(new Event('beforeunload', { cancelable: true }));"
      ),
    )?;
    let local_value = value.open(&mut self.js_runtime.handle_scope());
    Ok(local_value.is_false())
  }
}
