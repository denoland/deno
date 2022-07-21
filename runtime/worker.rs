// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::inspector_server::InspectorServer;
use crate::js;
use crate::ops;
use crate::ops::io::Stdio;
use crate::permissions::Permissions;
use crate::BootstrapOptions;
use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_cache::SqliteBackedCache;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::Future;
use deno_core::located_script_name;
use deno_core::serde_json::json;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsRuntime;
use deno_core::LocalInspectorSession;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_core::SourceMapGetter;
use deno_node::DenoDirNpmResolver;
use deno_tls::rustls::RootCertStore;
use deno_web::BlobStore;
use log::debug;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

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
  exit_code: ExitCode,
  js_run_tests_callback: v8::Global<v8::Function>,
  js_run_benchmarks_callback: v8::Global<v8::Function>,
  js_enable_test_callback: v8::Global<v8::Function>,
  js_enable_bench_callback: v8::Global<v8::Function>,
}

pub struct WorkerOptions {
  pub bootstrap: BootstrapOptions,
  pub extensions: Vec<Extension>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub root_cert_store: Option<RootCertStore>,
  pub seed: Option<u64>,
  pub module_loader: Rc<dyn ModuleLoader>,
  pub npm_resolver: Option<Rc<dyn DenoDirNpmResolver>>,
  // Callbacks invoked when creating new instance of WebWorker
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub web_worker_preload_module_cb: Arc<ops::worker_host::WorkerEventCb>,
  pub web_worker_pre_execute_module_cb: Arc<ops::worker_host::WorkerEventCb>,
  pub format_js_error_fn: Option<Arc<FormatJsErrorFn>>,
  pub source_map_getter: Option<Box<dyn SourceMapGetter>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub should_break_on_first_statement: bool,
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub origin_storage_dir: Option<std::path::PathBuf>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  pub stdio: Stdio,
}

fn grab_cb(
  scope: &mut v8::HandleScope,
  path: &str,
) -> v8::Global<v8::Function> {
  let cb = JsRuntime::grab_global::<v8::Function>(scope, path)
    .unwrap_or_else(|| panic!("{} must be defined", path));
  v8::Global::new(scope, cb)
}

impl MainWorker {
  pub fn bootstrap_from_options(
    main_module: ModuleSpecifier,
    permissions: Permissions,
    options: WorkerOptions,
  ) -> Self {
    let bootstrap_options = options.bootstrap.clone();
    let mut worker = Self::from_options(main_module, permissions, options);
    worker.bootstrap(&bootstrap_options);
    worker
  }

  pub fn from_options(
    main_module: ModuleSpecifier,
    permissions: Permissions,
    mut options: WorkerOptions,
  ) -> Self {
    // Permissions: many ops depend on this
    let unstable = options.bootstrap.unstable;
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let perm_ext = Extension::builder()
      .state(move |state| {
        state.put::<Permissions>(permissions.clone());
        state.put(ops::UnstableChecker { unstable });
        state.put(ops::TestingFeaturesEnabled(enable_testing_features));
        Ok(())
      })
      .build();
    let exit_code = ExitCode(Arc::new(AtomicI32::new(0)));

    // Internal modules
    let mut extensions: Vec<Extension> = vec![
      // Web APIs
      deno_webidl::init(),
      deno_cache::init(SqliteBackedCache::new(
        std::env::current_dir().unwrap(),
      )),
      deno_console::init(),
      deno_url::init(),
      deno_web::init::<Permissions>(
        options.blob_store.clone(),
        options.bootstrap.location.clone(),
      ),
      deno_fetch::init::<Permissions>(deno_fetch::Options {
        user_agent: options.bootstrap.user_agent.clone(),
        root_cert_store: options.root_cert_store.clone(),
        unsafely_ignore_certificate_errors: options
          .unsafely_ignore_certificate_errors
          .clone(),
        file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
        ..Default::default()
      }),
      deno_websocket::init::<Permissions>(
        options.bootstrap.user_agent.clone(),
        options.root_cert_store.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_webstorage::init(options.origin_storage_dir.clone()),
      deno_broadcast_channel::init(options.broadcast_channel.clone(), unstable),
      deno_crypto::init(options.seed),
      deno_webgpu::init(unstable),
      // ffi
      deno_ffi::init::<Permissions>(unstable),
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
      deno_net::init::<Permissions>(
        options.root_cert_store.clone(),
        unstable,
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_node::init::<Permissions>(unstable, options.npm_resolver),
      ops::os::init(exit_code.clone()),
      ops::permissions::init(),
      ops::process::init(),
      ops::signal::init(),
      ops::tty::init(),
      deno_http::init(),
      deno_flash::init::<Permissions>(unstable),
      ops::http::init(),
      // Permissions ext (worker specific state)
      perm_ext,
    ];
    extensions.extend(std::mem::take(&mut options.extensions));

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(js::deno_isolate_init()),
      source_map_getter: options.source_map_getter,
      get_error_class_fn: options.get_error_class_fn,
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
      extensions,
      ..Default::default()
    });

    if let Some(server) = options.maybe_inspector_server.clone() {
      server.register_inspector(
        main_module.to_string(),
        &mut js_runtime,
        options.should_break_on_first_statement,
      );
    }

    let (
      js_run_tests_callback,
      js_run_benchmarks_callback,
      js_enable_test_callback,
      js_enable_bench_callback,
    ) = {
      let scope = &mut js_runtime.handle_scope();
      (
        grab_cb(scope, "__bootstrap.testing.runTests"),
        grab_cb(scope, "__bootstrap.testing.runBenchmarks"),
        grab_cb(scope, "__bootstrap.testing.enableTest"),
        grab_cb(scope, "__bootstrap.testing.enableBench"),
      )
    };

    Self {
      js_runtime,
      should_break_on_first_statement: options.should_break_on_first_statement,
      exit_code,
      js_run_tests_callback,
      js_run_benchmarks_callback,
      js_enable_test_callback,
      js_enable_bench_callback,
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
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(script_name, source_code)?;
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

  /// Run tests declared with `Deno.test()`. Test events will be dispatched
  /// by calling ops which are currently only implemented in the CLI crate.
  // TODO(nayeemrmn): Move testing ops to deno_runtime and redesign/unhide.
  #[doc(hidden)]
  pub async fn run_tests(
    &mut self,
    shuffle: &Option<u64>,
  ) -> Result<(), AnyError> {
    let promise = {
      let scope = &mut self.js_runtime.handle_scope();
      let cb = self.js_run_tests_callback.open(scope);
      let this = v8::undefined(scope).into();
      let options =
        serde_v8::to_v8(scope, json!({ "shuffle": shuffle })).unwrap();
      let promise = cb.call(scope, this, &[options]).unwrap();
      v8::Global::new(scope, promise)
    };
    self.js_runtime.resolve_value(promise).await?;
    Ok(())
  }

  /// Run benches declared with `Deno.bench()`. Bench events will be dispatched
  /// by calling ops which are currently only implemented in the CLI crate.
  // TODO(nayeemrmn): Move benchmark ops to deno_runtime and redesign/unhide.
  #[doc(hidden)]
  pub async fn run_benchmarks(&mut self) -> Result<(), AnyError> {
    let promise = {
      let scope = &mut self.js_runtime.handle_scope();
      let cb = self.js_run_benchmarks_callback.open(scope);
      let this = v8::undefined(scope).into();
      let promise = cb.call(scope, this, &[]).unwrap();
      v8::Global::new(scope, promise)
    };
    self.js_runtime.resolve_value(promise).await?;
    Ok(())
  }

  /// Enable `Deno.test()`. If this isn't called before executing user code,
  /// `Deno.test()` calls will noop.
  // TODO(nayeemrmn): Move testing ops to deno_runtime and redesign/unhide.
  #[doc(hidden)]
  pub fn enable_test(&mut self) {
    let scope = &mut self.js_runtime.handle_scope();
    let cb = self.js_enable_test_callback.open(scope);
    let this = v8::undefined(scope).into();
    cb.call(scope, this, &[]).unwrap();
  }

  /// Enable `Deno.bench()`. If this isn't called before executing user code,
  /// `Deno.bench()` calls will noop.
  // TODO(nayeemrmn): Move benchmark ops to deno_runtime and redesign/unhide.
  #[doc(hidden)]
  pub fn enable_bench(&mut self) {
    let scope = &mut self.js_runtime.handle_scope();
    let cb = self.js_enable_bench_callback.open(scope);
    let this = v8::undefined(scope).into();
    cb.call(scope, this, &[]).unwrap();
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .js_runtime
        .inspector()
        .borrow_mut()
        .wait_for_session_and_break_on_next_statement()
    }
  }

  /// Create new inspector session. This function panics if Worker
  /// was not configured to create inspector.
  pub async fn create_inspector_session(&mut self) -> LocalInspectorSession {
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
  pub fn get_exit_code(&self) -> i32 {
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
    )
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
    )
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

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url_or_path;

  fn create_test_worker() -> MainWorker {
    let main_module = resolve_url_or_path("./hello.js").unwrap();
    let permissions = Permissions::default();

    let options = WorkerOptions {
      bootstrap: BootstrapOptions {
        args: vec![],
        cpu_count: 1,
        debug_flag: false,
        enable_testing_features: false,
        location: None,
        no_color: true,
        is_tty: false,
        runtime_version: "x".to_string(),
        ts_version: "x".to_string(),
        unstable: false,
        user_agent: "x".to_string(),
      },
      extensions: vec![],
      unsafely_ignore_certificate_errors: None,
      root_cert_store: None,
      seed: None,
      format_js_error_fn: None,
      source_map_getter: None,
      web_worker_preload_module_cb: Arc::new(|_| unreachable!()),
      web_worker_pre_execute_module_cb: Arc::new(|_| unreachable!()),
      create_web_worker_cb: Arc::new(|_| unreachable!()),
      maybe_inspector_server: None,
      should_break_on_first_statement: false,
      module_loader: Rc::new(deno_core::FsModuleLoader),
      npm_resolver: None,
      get_error_class_fn: None,
      origin_storage_dir: None,
      blob_store: BlobStore::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      stdio: Default::default(),
    };

    MainWorker::bootstrap_from_options(main_module, permissions, options)
  }

  #[tokio::test]
  async fn execute_mod_esm_imports_a() {
    let p = test_util::testdata_path().join("esm_imports_a.js");
    let module_specifier = resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.js");
    let module_specifier = resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {:?}", err);
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {:?}", e);
    }
  }

  #[tokio::test]
  async fn execute_mod_resolve_error() {
    // "foo" is not a valid module specifier so this should return an error.
    let mut worker = create_test_worker();
    let module_specifier = resolve_url_or_path("does-not-exist").unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let mut worker = create_test_worker();
    let p = test_util::testdata_path().join("001_hello.js");
    let module_specifier = resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_ok());
  }
}
