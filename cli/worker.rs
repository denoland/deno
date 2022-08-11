use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::task::LocalFutureObj;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::serde_json::json;
use deno_core::Extension;
use deno_core::ModuleId;
use deno_runtime::colors;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::ops::worker_host::PreloadModuleCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;

use crate::checksum;
use crate::compat;
use crate::errors;
use crate::fmt_errors::format_js_error;
use crate::module_loader::CliModuleLoader;
use crate::ops;
use crate::proc_state::ProcState;
use crate::tools;
use crate::tools::coverage::CoverageCollector;
use crate::tools::test::TestMode;
use crate::version;

pub struct CliMainWorker {
  main_module: ModuleSpecifier,
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
    if self.ps.options.compat() {
      self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
      compat::add_global_require(
        &mut self.worker.js_runtime,
        self.main_module.as_str(),
      )?;
      self.worker.run_event_loop(false).await?;
      compat::setup_builtin_modules(&mut self.worker.js_runtime)?;
    }
    self.worker.run_event_loop(false).await?;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<i32, AnyError> {
    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;
    log::debug!("main_module {}", self.main_module);

    if self.ps.options.compat() {
      // TODO(bartlomieju): fix me
      assert_eq!(self.main_module.scheme(), "file");

      // Set up Node globals
      self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
      // And `module` module that we'll use for checking which
      // loader to use and potentially load CJS module with.
      // This allows to skip permission check for `--allow-net`
      // which would otherwise be requested by dynamically importing
      // this file.
      self.worker.execute_side_module(&compat::MODULE_URL).await?;

      let use_esm_loader =
        compat::check_if_should_use_esm_loader(&self.main_module)?;

      if use_esm_loader {
        // ES module execution in Node compatiblity mode
        self.worker.execute_main_module(&self.main_module).await?;
      } else {
        // CJS module execution in Node compatiblity mode
        compat::load_cjs_module(
          &mut self.worker.js_runtime,
          &self
            .main_module
            .to_file_path()
            .unwrap()
            .display()
            .to_string(),
          true,
        )?;
      }
    } else {
      // Regular ES module execution
      self.worker.execute_main_module(&self.main_module).await?;
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
      worker: MainWorker,
      pending_unload: bool,
      ps: ProcState,
    }

    impl FileWatcherModuleExecutor {
      pub fn new(
        worker: MainWorker,
        ps: ProcState,
      ) -> FileWatcherModuleExecutor {
        FileWatcherModuleExecutor {
          worker,
          pending_unload: false,
          ps,
        }
      }

      /// Execute the given main module emitting load and unload events before and after execution
      /// respectively.
      pub async fn execute(
        &mut self,
        main_module: &ModuleSpecifier,
      ) -> Result<(), AnyError> {
        if self.ps.options.compat() {
          self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
        }
        self.worker.execute_main_module(main_module).await?;
        self.worker.dispatch_load_event(&located_script_name!())?;
        self.pending_unload = true;

        let result = loop {
          let result = self.worker.run_event_loop(false).await;
          if !self
            .worker
            .dispatch_beforeunload_event(&located_script_name!())?
          {
            break result;
          }
        };
        self.pending_unload = false;

        if let Err(err) = result {
          return Err(err);
        }

        self.worker.dispatch_unload_event(&located_script_name!())?;

        Ok(())
      }
    }

    impl Drop for FileWatcherModuleExecutor {
      fn drop(&mut self) {
        if self.pending_unload {
          self
            .worker
            .dispatch_unload_event(&located_script_name!())
            .unwrap();
        }
      }
    }

    let mut executor = FileWatcherModuleExecutor::new(self.worker, self.ps);
    executor.execute(&self.main_module).await
  }

  pub async fn run_test_specifier(
    &mut self,
    mode: TestMode,
  ) -> Result<(), AnyError> {
    self.worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].enableTestAndBench()"#,
    )?;

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
      if self.ps.options.compat() {
        self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
        self.worker.execute_side_module(&compat::MODULE_URL).await?;

        let use_esm_loader =
          compat::check_if_should_use_esm_loader(&self.main_module)?;

        if use_esm_loader {
          self.worker.execute_side_module(&self.main_module).await?;
        } else {
          compat::load_cjs_module(
            &mut self.worker.js_runtime,
            &self
              .main_module
              .to_file_path()
              .unwrap()
              .display()
              .to_string(),
            false,
          )?;
          self.worker.run_event_loop(false).await?;
        }
      } else {
        // We execute the module module as a side module so that import.meta.main is not set.
        self.worker.execute_side_module(&self.main_module).await?;
      }
    }

    self.worker.dispatch_load_event(&located_script_name!())?;

    let test_result = self.worker.js_runtime.execute_script(
      &located_script_name!(),
      &format!(
        r#"Deno[Deno.internal].runTests({})"#,
        json!({ "shuffle": self.ps.options.shuffle_tests() }),
      ),
    )?;

    self.worker.js_runtime.resolve_value(test_result).await?;

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
    self.worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].enableTestAndBench()"#,
    )?;

    self
      .worker
      .execute_script(
        &located_script_name!(),
        "Deno.core.enableOpCallTracing();",
      )
      .unwrap();

    if mode != TestMode::Documentation {
      self.worker.execute_side_module(&self.main_module).await?;
    }

    self.worker.dispatch_load_event(&located_script_name!())?;

    let test_result = self.worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].runTests()"#,
    )?;

    self.worker.js_runtime.resolve_value(test_result).await?;

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
    self.worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].enableTestAndBench()"#,
    )?;

    if self.ps.options.compat() {
      self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
      self.worker.execute_side_module(&compat::MODULE_URL).await?;

      let use_esm_loader =
        compat::check_if_should_use_esm_loader(&self.main_module)?;

      if use_esm_loader {
        self.worker.execute_side_module(&self.main_module).await?;
      } else {
        compat::load_cjs_module(
          &mut self.worker.js_runtime,
          &self
            .main_module
            .to_file_path()
            .unwrap()
            .display()
            .to_string(),
          false,
        )?;
        self.worker.run_event_loop(false).await?;
      }
    } else {
      // We execute the module module as a side module so that import.meta.main is not set.
      self.worker.execute_side_module(&self.main_module).await?;
    }

    self.worker.dispatch_load_event(&located_script_name!())?;

    let bench_result = self.worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].runBenchmarks()"#,
    )?;

    self.worker.js_runtime.resolve_value(bench_result).await?;

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

  async fn maybe_setup_coverage_collector(
    &mut self,
  ) -> Result<Option<CoverageCollector>, AnyError> {
    if let Some(ref coverage_dir) = self.ps.coverage_dir {
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

pub fn create_main_worker(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  mut custom_extensions: Vec<Extension>,
  stdio: deno_runtime::ops::io::Stdio,
) -> CliMainWorker {
  let module_loader = CliModuleLoader::new(ps.clone());

  let maybe_inspector_server = ps.maybe_inspector_server.clone();
  let should_break_on_first_statement = ps.options.inspect_brk().is_some();

  let create_web_worker_cb =
    create_web_worker_callback(ps.clone(), stdio.clone());
  let web_worker_preload_module_cb =
    create_web_worker_preload_module_callback(ps.clone());

  let maybe_storage_key = ps.options.resolve_storage_key(&main_module);
  let origin_storage_dir = maybe_storage_key.map(|key| {
    ps.dir
      .root
      // TODO(@crowlKats): change to origin_data for 2.0
      .join("location_data")
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
      location: ps.options.location_flag().map(ToOwned::to_owned),
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: ps.options.unstable(),
      user_agent: version::get_user_agent(),
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
    maybe_inspector_server,
    should_break_on_first_statement,
    module_loader,
    get_error_class_fn: Some(&errors::get_error_class_name),
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
  CliMainWorker {
    main_module,
    worker,
    ps: ps.clone(),
  }
}

fn create_web_worker_preload_module_callback(
  ps: ProcState,
) -> Arc<PreloadModuleCb> {
  let compat = ps.options.compat();

  Arc::new(move |mut worker| {
    let fut = async move {
      if compat {
        worker.execute_side_module(&compat::GLOBAL_URL).await?;
        worker.execute_side_module(&compat::MODULE_URL).await?;
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

    let extensions = ops::cli_exts(ps.clone());

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
        location: Some(args.main_module.clone()),
        no_color: !colors::use_color(),
        is_tty: colors::is_tty(),
        runtime_version: version::deno(),
        ts_version: version::TYPESCRIPT.to_string(),
        unstable: ps.options.unstable(),
        user_agent: version::get_user_agent(),
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
      format_js_error_fn: Some(Arc::new(format_js_error)),
      source_map_getter: Some(Box::new(module_loader.clone())),
      module_loader,
      worker_type: args.worker_type,
      maybe_inspector_server,
      get_error_class_fn: Some(&errors::get_error_class_name),
      blob_store: ps.blob_store.clone(),
      broadcast_channel: ps.broadcast_channel.clone(),
      shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
      stdio: stdio.clone(),
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
