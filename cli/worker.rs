use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::task::LocalFutureObj;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::Extension;
use deno_core::ModuleId;
use deno_runtime::colors;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::ops::worker_host::WorkerEventCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;

use crate::args::DenoSubcommand;
use crate::checksum;
use crate::errors;
use crate::module_loader::CliModuleLoader;
use crate::node;
use crate::npm::NpmPackageReference;
use crate::ops;
use crate::proc_state::ProcState;
use crate::tools;
use crate::tools::coverage::CoverageCollector;
use crate::tools::test::TestMode;
use crate::version;

pub struct CliMainWorker {
  main_module: ModuleSpecifier,
  is_main_cjs: bool,
  worker: MainWorker,
  ps: ProcState,
}

impl CliMainWorker {
  pub fn into_main_worker(self) -> MainWorker {
    self.worker
  }

  pub async fn preload_main_module(&mut self) -> Result<ModuleId, AnyError> {
    self.worker.preload_main_module(&self.main_module).await
  }

  pub async fn setup_repl(&mut self) -> Result<(), AnyError> {
    self.worker.run_event_loop(false).await?;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<i32, AnyError> {
    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;
    log::debug!("main_module {}", self.main_module);

    if self.is_main_cjs {
      self.ps.prepare_node_std_graph().await?;
      self.initialize_main_module_for_node().await?;
      node::load_cjs_module_from_ext_node(
        &mut self.worker.js_runtime,
        &self.main_module.to_file_path().unwrap().to_string_lossy(),
        true,
      )?;
    } else {
      self.execute_main_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(&located_script_name!())?;

    loop {
      self
        .worker
        .run_event_loop(maybe_coverage_collector.is_none())
        .await?;
      if !self
        .worker
        .dispatch_beforeunload_event(&located_script_name!())?
      {
        break;
      }
    }

    self.worker.dispatch_unload_event(&located_script_name!())?;

    if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
      self
        .worker
        .with_event_loop(coverage_collector.stop_collecting().boxed_local())
        .await?;
    }

    Ok(self.worker.get_exit_code())
  }

  pub async fn run_for_watcher(self) -> Result<(), AnyError> {
    /// The FileWatcherModuleExecutor provides module execution with safe dispatching of life-cycle events by tracking the
    /// state of any pending events and emitting accordingly on drop in the case of a future
    /// cancellation.
    struct FileWatcherModuleExecutor {
      inner: CliMainWorker,
      pending_unload: bool,
    }

    impl FileWatcherModuleExecutor {
      pub fn new(worker: CliMainWorker) -> FileWatcherModuleExecutor {
        FileWatcherModuleExecutor {
          inner: worker,
          pending_unload: false,
        }
      }

      /// Execute the given main module emitting load and unload events before and after execution
      /// respectively.
      pub async fn execute(&mut self) -> Result<(), AnyError> {
        self.inner.execute_main_module_possibly_with_npm().await?;
        self
          .inner
          .worker
          .dispatch_load_event(&located_script_name!())?;
        self.pending_unload = true;

        let result = loop {
          match self.inner.worker.run_event_loop(false).await {
            Ok(()) => {}
            Err(error) => break Err(error),
          }
          match self
            .inner
            .worker
            .dispatch_beforeunload_event(&located_script_name!())
          {
            Ok(default_prevented) if default_prevented => {} // continue loop
            Ok(_) => break Ok(()),
            Err(error) => break Err(error),
          }
        };
        self.pending_unload = false;

        result?;

        self
          .inner
          .worker
          .dispatch_unload_event(&located_script_name!())?;

        Ok(())
      }
    }

    impl Drop for FileWatcherModuleExecutor {
      fn drop(&mut self) {
        if self.pending_unload {
          let _ = self
            .inner
            .worker
            .dispatch_unload_event(&located_script_name!());
        }
      }
    }

    let mut executor = FileWatcherModuleExecutor::new(self);
    executor.execute().await
  }

  pub async fn run_test_specifier(
    &mut self,
    mode: TestMode,
  ) -> Result<(), AnyError> {
    self.worker.enable_test();

    // Enable op call tracing in core to enable better debugging of op sanitizer
    // failures.
    if self.ps.options.trace_ops() {
      self
        .worker
        .js_runtime
        .execute_script(
          &located_script_name!(),
          "Deno.core.enableOpCallTracing();",
        )
        .unwrap();
    }

    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;

    // We only execute the specifier as a module if it is tagged with TestMode::Module or
    // TestMode::Both.
    if mode != TestMode::Documentation {
      // We execute the module module as a side module so that import.meta.main is not set.
      self.execute_side_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(&located_script_name!())?;
    self
      .worker
      .run_tests(&self.ps.options.shuffle_tests())
      .await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(&located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }

    self.worker.dispatch_unload_event(&located_script_name!())?;

    if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
      self
        .worker
        .with_event_loop(coverage_collector.stop_collecting().boxed_local())
        .await?;
    }
    Ok(())
  }

  pub async fn run_lsp_test_specifier(
    &mut self,
    mode: TestMode,
  ) -> Result<(), AnyError> {
    self.worker.enable_test();

    self
      .worker
      .execute_script(
        &located_script_name!(),
        "Deno.core.enableOpCallTracing();",
      )
      .unwrap();

    if mode != TestMode::Documentation {
      // We execute the module module as a side module so that import.meta.main is not set.
      self.execute_side_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(&located_script_name!())?;
    self.worker.run_tests(&None).await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(&located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }
    self.worker.dispatch_unload_event(&located_script_name!())?;
    Ok(())
  }

  pub async fn run_bench_specifier(&mut self) -> Result<(), AnyError> {
    self.worker.enable_bench();

    // We execute the module module as a side module so that import.meta.main is not set.
    self.execute_side_module_possibly_with_npm().await?;

    self.worker.dispatch_load_event(&located_script_name!())?;
    self.worker.run_benchmarks().await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(&located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }
    self.worker.dispatch_unload_event(&located_script_name!())?;
    Ok(())
  }

  async fn execute_main_module_possibly_with_npm(
    &mut self,
  ) -> Result<(), AnyError> {
    if self.ps.npm_resolver.has_packages() {
      self.ps.prepare_node_std_graph().await?;
    }
    let id = self.worker.preload_main_module(&self.main_module).await?;
    self.evaluate_module_possibly_with_npm(id).await
  }

  async fn execute_side_module_possibly_with_npm(
    &mut self,
  ) -> Result<(), AnyError> {
    let id = self.worker.preload_side_module(&self.main_module).await?;
    self.evaluate_module_possibly_with_npm(id).await
  }

  async fn evaluate_module_possibly_with_npm(
    &mut self,
    id: ModuleId,
  ) -> Result<(), AnyError> {
    if self.ps.npm_resolver.has_packages() {
      self.initialize_main_module_for_node().await?;
    }
    self.worker.evaluate_module(id).await
  }

  async fn initialize_main_module_for_node(&mut self) -> Result<(), AnyError> {
    self.ps.prepare_node_std_graph().await?;
    node::initialize_runtime(&mut self.worker.js_runtime).await?;
    if let DenoSubcommand::Run(flags) = self.ps.options.sub_command() {
      if let Ok(pkg_ref) = NpmPackageReference::from_str(&flags.script) {
        // if the user ran a binary command, we'll need to set process.argv[0]
        // to be the name of the binary command instead of deno
        let binary_name = pkg_ref
          .sub_path
          .as_deref()
          .unwrap_or(pkg_ref.req.name.as_str());
        node::initialize_binary_command(
          &mut self.worker.js_runtime,
          binary_name,
        )
        .await?;
      }
    }
    Ok(())
  }

  async fn maybe_setup_coverage_collector(
    &mut self,
  ) -> Result<Option<CoverageCollector>, AnyError> {
    if let Some(ref coverage_dir) = self.ps.options.coverage_dir() {
      let session = self.worker.create_inspector_session().await;

      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
      self
        .worker
        .with_event_loop(coverage_collector.start_collecting().boxed_local())
        .await?;
      Ok(Some(coverage_collector))
    } else {
      Ok(None)
    }
  }
}

pub async fn create_main_worker(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  mut custom_extensions: Vec<Extension>,
  stdio: deno_runtime::ops::io::Stdio,
) -> Result<CliMainWorker, AnyError> {
  let (main_module, is_main_cjs) = if let Ok(package_ref) =
    NpmPackageReference::from_specifier(&main_module)
  {
    ps.npm_resolver
      .add_package_reqs(vec![package_ref.req.clone()])
      .await?;
    let node_resolution = node::node_resolve_binary_export(
      &package_ref.req,
      package_ref.sub_path.as_deref(),
      &ps.npm_resolver,
    )?;
    let is_main_cjs =
      matches!(node_resolution, node::NodeResolution::CommonJs(_));
    (node_resolution.into_url(), is_main_cjs)
  } else if ps.npm_resolver.is_npm_main() {
    let node_resolution =
      node::url_to_node_resolution(main_module, &ps.npm_resolver)?;
    let is_main_cjs =
      matches!(node_resolution, node::NodeResolution::CommonJs(_));
    (node_resolution.into_url(), is_main_cjs)
  } else {
    (main_module, false)
  };

  let module_loader = CliModuleLoader::new(ps.clone());

  let maybe_inspector_server = ps.maybe_inspector_server.clone();
  let should_break_on_first_statement = ps.options.inspect_brk().is_some();

  let create_web_worker_cb =
    create_web_worker_callback(ps.clone(), stdio.clone());
  let web_worker_preload_module_cb =
    create_web_worker_preload_module_callback(ps.clone());
  let web_worker_pre_execute_module_cb =
    create_web_worker_pre_execute_module_callback(ps.clone());

  let maybe_storage_key = ps.options.resolve_storage_key(&main_module);
  let origin_storage_dir = maybe_storage_key.as_ref().map(|key| {
    ps.dir
      .root
      // TODO(@crowlKats): change to origin_data for 2.0
      .join("location_data")
      .join(checksum::gen(&[key.as_bytes()]))
  });
  let cache_storage_dir = maybe_storage_key.map(|key| {
    // TODO(@satyarohith): storage quota management
    // Note: we currently use temp_dir() to avoid managing storage size.
    std::env::temp_dir()
      .join("deno_cache")
      .join(checksum::gen(&[key.as_bytes()]))
  });

  let mut extensions = ops::cli_exts(ps.clone());
  extensions.append(&mut custom_extensions);

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: ps.options.argv().clone(),
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      debug_flag: ps
        .options
        .log_level()
        .map_or(false, |l| l == log::Level::Debug),
      enable_testing_features: ps.options.enable_testing_features(),
      locale: deno_core::v8::icu::get_language_tag(),
      location: ps.options.location_flag().map(ToOwned::to_owned),
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: ps.options.unstable(),
      user_agent: version::get_user_agent(),
      inspect: ps.options.is_inspecting(),
    },
    extensions,
    unsafely_ignore_certificate_errors: ps
      .options
      .unsafely_ignore_certificate_errors()
      .map(ToOwned::to_owned),
    root_cert_store: Some(ps.root_cert_store.clone()),
    seed: ps.options.seed(),
    source_map_getter: Some(Box::new(module_loader.clone())),
    format_js_error_fn: Some(Arc::new(format_js_error)),
    create_web_worker_cb,
    web_worker_preload_module_cb,
    web_worker_pre_execute_module_cb,
    maybe_inspector_server,
    should_break_on_first_statement,
    module_loader,
    npm_resolver: Some(Rc::new(ps.npm_resolver.clone())),
    get_error_class_fn: Some(&errors::get_error_class_name),
    cache_storage_dir,
    origin_storage_dir,
    blob_store: ps.blob_store.clone(),
    broadcast_channel: ps.broadcast_channel.clone(),
    shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
    compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
    stdio,
  };

  let worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    options,
  );
  Ok(CliMainWorker {
    main_module,
    is_main_cjs,
    worker,
    ps: ps.clone(),
  })
}

// TODO(bartlomieju): this callback could have default value
// and not be required
fn create_web_worker_preload_module_callback(
  _ps: ProcState,
) -> Arc<WorkerEventCb> {
  Arc::new(move |worker| {
    let fut = async move { Ok(worker) };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_pre_execute_module_callback(
  ps: ProcState,
) -> Arc<WorkerEventCb> {
  Arc::new(move |mut worker| {
    let ps = ps.clone();
    let fut = async move {
      // this will be up to date after pre-load
      if ps.npm_resolver.has_packages() {
        node::initialize_runtime(&mut worker.js_runtime).await?;
      }

      Ok(worker)
    };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_callback(
  ps: ProcState,
  stdio: deno_runtime::ops::io::Stdio,
) -> Arc<CreateWebWorkerCb> {
  Arc::new(move |args| {
    let maybe_inspector_server = ps.maybe_inspector_server.clone();

    let module_loader = CliModuleLoader::new_for_worker(
      ps.clone(),
      args.parent_permissions.clone(),
    );
    let create_web_worker_cb =
      create_web_worker_callback(ps.clone(), stdio.clone());
    let preload_module_cb =
      create_web_worker_preload_module_callback(ps.clone());
    let pre_execute_module_cb =
      create_web_worker_pre_execute_module_callback(ps.clone());

    let extensions = ops::cli_exts(ps.clone());

    let maybe_storage_key = ps.options.resolve_storage_key(&args.main_module);
    let cache_storage_dir = maybe_storage_key.map(|key| {
      // TODO(@satyarohith): storage quota management
      // Note: we currently use temp_dir() to avoid managing storage size.
      std::env::temp_dir()
        .join("deno_cache")
        .join(checksum::gen(&[key.as_bytes()]))
    });

    let options = WebWorkerOptions {
      bootstrap: BootstrapOptions {
        args: ps.options.argv().clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        debug_flag: ps
          .options
          .log_level()
          .map_or(false, |l| l == log::Level::Debug),
        enable_testing_features: ps.options.enable_testing_features(),
        locale: deno_core::v8::icu::get_language_tag(),
        location: Some(args.main_module.clone()),
        no_color: !colors::use_color(),
        is_tty: colors::is_tty(),
        runtime_version: version::deno(),
        ts_version: version::TYPESCRIPT.to_string(),
        unstable: ps.options.unstable(),
        user_agent: version::get_user_agent(),
        inspect: ps.options.is_inspecting(),
      },
      extensions,
      unsafely_ignore_certificate_errors: ps
        .options
        .unsafely_ignore_certificate_errors()
        .map(ToOwned::to_owned),
      root_cert_store: Some(ps.root_cert_store.clone()),
      seed: ps.options.seed(),
      create_web_worker_cb,
      preload_module_cb,
      pre_execute_module_cb,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      source_map_getter: Some(Box::new(module_loader.clone())),
      module_loader,
      npm_resolver: Some(Rc::new(ps.npm_resolver.clone())),
      worker_type: args.worker_type,
      maybe_inspector_server,
      get_error_class_fn: Some(&errors::get_error_class_name),
      blob_store: ps.blob_store.clone(),
      broadcast_channel: ps.broadcast_channel.clone(),
      shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
      stdio: stdio.clone(),
      cache_storage_dir,
    };

    WebWorker::bootstrap_from_options(
      args.name,
      args.permissions,
      args.main_module,
      args.worker_id,
      options,
    )
  })
}
