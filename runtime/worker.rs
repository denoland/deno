// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::inspector_server::InspectorServer;
use crate::js;
use crate::ops;
use crate::permissions::Permissions;
use crate::BootstrapOptions;
use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::located_script_name;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::GetErrorClassFn;
use deno_core::JsErrorCreateFn;
use deno_core::JsRuntime;
use deno_core::LocalInspectorSession;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
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
}

pub struct WorkerOptions {
  pub bootstrap: BootstrapOptions,
  pub extensions: Vec<Extension>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub root_cert_store: Option<RootCertStore>,
  pub user_agent: String,
  pub seed: Option<u64>,
  pub module_loader: Rc<dyn ModuleLoader>,
  // Callback invoked when creating new instance of WebWorker
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub js_error_create_fn: Option<Rc<JsErrorCreateFn>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub should_break_on_first_statement: bool,
  pub get_error_class_fn: Option<GetErrorClassFn>,
  pub origin_storage_dir: Option<std::path::PathBuf>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
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

    // Internal modules
    let mut extensions: Vec<Extension> = vec![
      // Web APIs
      deno_webidl::init(),
      deno_console::init(),
      deno_url::init(),
      deno_web::init(
        options.blob_store.clone(),
        options.bootstrap.location.clone(),
      ),
      deno_fetch::init::<Permissions>(deno_fetch::Options {
        user_agent: options.user_agent.clone(),
        root_cert_store: options.root_cert_store.clone(),
        unsafely_ignore_certificate_errors: options
          .unsafely_ignore_certificate_errors
          .clone(),
        file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
        ..Default::default()
      }),
      deno_websocket::init::<Permissions>(
        options.user_agent.clone(),
        options.root_cert_store.clone(),
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      deno_webstorage::init(options.origin_storage_dir.clone()),
      deno_crypto::init(options.seed),
      deno_broadcast_channel::init(options.broadcast_channel.clone(), unstable),
      deno_webgpu::init(unstable),
      deno_timers::init::<Permissions>(),
      // ffi
      deno_ffi::init::<Permissions>(unstable),
      // Runtime ops
      ops::runtime::init(main_module.clone()),
      ops::worker_host::init(options.create_web_worker_cb.clone()),
      ops::fs_events::init(),
      ops::fs::init(),
      ops::io::init(),
      ops::io::init_stdio(),
      deno_tls::init(),
      deno_net::init::<Permissions>(
        options.root_cert_store.clone(),
        unstable,
        options.unsafely_ignore_certificate_errors.clone(),
      ),
      ops::os::init(None),
      ops::permissions::init(),
      ops::process::init(),
      ops::signal::init(),
      ops::tty::init(),
      deno_http::init(),
      ops::http::init(),
      // Permissions ext (worker specific state)
      perm_ext,
    ];
    extensions.extend(std::mem::take(&mut options.extensions));

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(options.module_loader.clone()),
      startup_snapshot: Some(js::deno_isolate_init()),
      js_error_create_fn: options.js_error_create_fn.clone(),
      get_error_class_fn: options.get_error_class_fn,
      shared_array_buffer_store: options.shared_array_buffer_store.clone(),
      compiled_wasm_module_store: options.compiled_wasm_module_store.clone(),
      extensions,
      ..Default::default()
    });

    if let Some(server) = options.maybe_inspector_server.clone() {
      server.register_inspector(main_module.to_string(), &mut js_runtime);
    }

    Self {
      js_runtime,
      should_break_on_first_statement: options.should_break_on_first_statement,
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
    name: &str,
    source_code: &str,
  ) -> Result<(), AnyError> {
    self.js_runtime.execute_script(name, source_code)?;
    Ok(())
  }

  /// Loads and instantiates specified JavaScript module
  /// as "main" or "side" module.
  pub async fn preload_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
    main: bool,
  ) -> Result<ModuleId, AnyError> {
    if main {
      self
        .js_runtime
        .load_main_module(module_specifier, None)
        .await
    } else {
      self
        .js_runtime
        .load_side_module(module_specifier, None)
        .await
    }
  }

  async fn evaluate_module(&mut self, id: ModuleId) -> Result<(), AnyError> {
    let mut receiver = self.js_runtime.mod_evaluate(id);
    tokio::select! {
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
    let id = self.preload_module(module_specifier, false).await?;
    self.evaluate_module(id).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  ///
  /// This module will have "import.meta.main" equal to true.
  pub async fn execute_main_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.preload_module(module_specifier, true).await?;
    self.wait_for_inspector_session();
    self.evaluate_module(id).await
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .js_runtime
        .inspector()
        .wait_for_session_and_break_on_next_statement()
    }
  }

  /// Create new inspector session. This function panics if Worker
  /// was not configured to create inspector.
  pub async fn create_inspector_session(&mut self) -> LocalInspectorSession {
    let inspector = self.js_runtime.inspector();
    inspector.create_local_session()
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
        result = &mut fut => {
          return result;
        }
        _ = self.run_event_loop(false) => {}
      };
    }
  }

  /// Return exit code set by the executed code (either in main worker
  /// or one of child web workers).
  pub fn get_exit_code(&mut self) -> i32 {
    let op_state_rc = self.js_runtime.op_state();
    let op_state = op_state_rc.borrow();
    let exit_code = op_state.borrow::<Arc<AtomicI32>>().load(Relaxed);
    exit_code
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
        apply_source_maps: false,
        args: vec![],
        cpu_count: 1,
        debug_flag: false,
        enable_testing_features: false,
        location: None,
        no_color: true,
        runtime_version: "x".to_string(),
        ts_version: "x".to_string(),
        unstable: false,
      },
      extensions: vec![],
      user_agent: "x".to_string(),
      unsafely_ignore_certificate_errors: None,
      root_cert_store: None,
      seed: None,
      js_error_create_fn: None,
      create_web_worker_cb: Arc::new(|_| unreachable!()),
      maybe_inspector_server: None,
      should_break_on_first_statement: false,
      module_loader: Rc::new(deno_core::FsModuleLoader),
      get_error_class_fn: None,
      origin_storage_dir: None,
      blob_store: BlobStore::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
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
