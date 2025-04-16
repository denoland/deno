// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::CoreError;
use deno_core::futures::FutureExt;
use deno_core::v8;
use deno_core::Extension;
use deno_core::PollEventLoopOptions;
use deno_error::JsErrorBox;
use deno_lib::worker::LibMainWorker;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::ResolveNpmBinaryEntrypointError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use deno_semver::npm::NpmPackageReqReference;
use sys_traits::EnvCurrentDir;
use tokio::select;

use crate::args::CliLockfile;
use crate::args::NpmCachingStrategy;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::PackageCaching;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::file_watcher::WatcherRestartMode;

#[async_trait::async_trait(?Send)]
pub trait HmrRunner: Send + Sync {
  async fn start(&mut self) -> Result<(), CoreError>;
  async fn stop(&mut self) -> Result<(), CoreError>;
  async fn run(&mut self) -> Result<(), CoreError>;
}

#[async_trait::async_trait(?Send)]
pub trait CoverageCollector: Send + Sync {
  async fn start_collecting(&mut self) -> Result<(), CoreError>;
  async fn stop_collecting(&mut self) -> Result<(), CoreError>;
}

pub type CreateHmrRunnerCb = Box<
  dyn Fn(deno_core::LocalInspectorSession) -> Box<dyn HmrRunner> + Send + Sync,
>;

pub type CreateCoverageCollectorCb = Box<
  dyn Fn(deno_core::LocalInspectorSession) -> Box<dyn CoverageCollector>
    + Send
    + Sync,
>;

pub struct CliMainWorkerOptions {
  pub create_hmr_runner: Option<CreateHmrRunnerCb>,
  pub create_coverage_collector: Option<CreateCoverageCollectorCb>,
  pub default_npm_caching_strategy: NpmCachingStrategy,
  pub needs_test_modules: bool,
}

/// Data shared between the factory and workers.
struct SharedState {
  pub create_hmr_runner: Option<CreateHmrRunnerCb>,
  pub create_coverage_collector: Option<CreateCoverageCollectorCb>,
  pub maybe_file_watcher_communicator: Option<Arc<WatcherCommunicator>>,
}

pub struct CliMainWorker {
  worker: LibMainWorker,
  shared: Arc<SharedState>,
}

impl CliMainWorker {
  #[inline]
  pub fn into_main_worker(self) -> MainWorker {
    self.worker.into_main_worker()
  }

  pub async fn setup_repl(&mut self) -> Result<(), CoreError> {
    self.worker.run_event_loop(false).await?;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<i32, CoreError> {
    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;
    let mut maybe_hmr_runner = self.maybe_setup_hmr_runner().await?;

    log::debug!("main_module {}", self.worker.main_module());

    // WARNING: Remember to update cli/lib/worker.rs to align with
    // changes made here so that they affect deno_compile as well.
    self.execute_main_module().await?;
    self.worker.dispatch_load_event()?;

    loop {
      if let Some(hmr_runner) = maybe_hmr_runner.as_mut() {
        let hmr_future = hmr_runner.run().boxed_local();
        let event_loop_future = self.worker.run_event_loop(false).boxed_local();

        let result;
        select! {
          hmr_result = hmr_future => {
            result = hmr_result;
          },
          event_loop_result = event_loop_future => {
            result = event_loop_result;
          }
        }
        if let Err(e) = result {
          self
            .shared
            .maybe_file_watcher_communicator
            .as_ref()
            .unwrap()
            .change_restart_mode(WatcherRestartMode::Automatic);
          return Err(e);
        }
      } else {
        self
          .worker
          .run_event_loop(maybe_coverage_collector.is_none())
          .await?;
      }

      let web_continue = self.worker.dispatch_beforeunload_event()?;
      if !web_continue {
        let node_continue = self.worker.dispatch_process_beforeexit_event()?;
        if !node_continue {
          break;
        }
      }
    }

    self.worker.dispatch_unload_event()?;
    self.worker.dispatch_process_exit_event()?;

    if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
      self
        .worker
        .js_runtime()
        .with_event_loop_future(
          coverage_collector.stop_collecting().boxed_local(),
          PollEventLoopOptions::default(),
        )
        .await?;
    }
    if let Some(hmr_runner) = maybe_hmr_runner.as_mut() {
      self
        .worker
        .js_runtime()
        .with_event_loop_future(
          hmr_runner.stop().boxed_local(),
          PollEventLoopOptions::default(),
        )
        .await?;
    }

    Ok(self.worker.exit_code())
  }

  pub async fn run_for_watcher(self) -> Result<(), CoreError> {
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
      pub async fn execute(&mut self) -> Result<(), CoreError> {
        self.inner.execute_main_module().await?;
        self.inner.worker.dispatch_load_event()?;
        self.pending_unload = true;

        let result = loop {
          match self.inner.worker.run_event_loop(false).await {
            Ok(()) => {}
            Err(error) => break Err(error),
          }
          let web_continue = self.inner.worker.dispatch_beforeunload_event()?;
          if !web_continue {
            let node_continue =
              self.inner.worker.dispatch_process_beforeexit_event()?;
            if !node_continue {
              break Ok(());
            }
          }
        };
        self.pending_unload = false;

        result?;

        self.inner.worker.dispatch_unload_event()?;
        self.inner.worker.dispatch_process_exit_event()?;

        Ok(())
      }
    }

    impl Drop for FileWatcherModuleExecutor {
      fn drop(&mut self) {
        if self.pending_unload {
          let _ = self.inner.worker.dispatch_unload_event();
        }
      }
    }

    let mut executor = FileWatcherModuleExecutor::new(self);
    executor.execute().await
  }

  #[inline]
  pub async fn execute_main_module(&mut self) -> Result<(), CoreError> {
    self.worker.execute_main_module().await
  }

  #[inline]
  pub async fn execute_side_module(&mut self) -> Result<(), CoreError> {
    self.worker.execute_side_module().await
  }

  pub async fn maybe_setup_hmr_runner(
    &mut self,
  ) -> Result<Option<Box<dyn HmrRunner>>, CoreError> {
    let Some(setup_hmr_runner) = self.shared.create_hmr_runner.as_ref() else {
      return Ok(None);
    };

    let session = self.worker.create_inspector_session();

    let mut hmr_runner = setup_hmr_runner(session);

    self
      .worker
      .js_runtime()
      .with_event_loop_future(
        hmr_runner.start().boxed_local(),
        PollEventLoopOptions::default(),
      )
      .await?;
    Ok(Some(hmr_runner))
  }

  pub async fn maybe_setup_coverage_collector(
    &mut self,
  ) -> Result<Option<Box<dyn CoverageCollector>>, CoreError> {
    let Some(create_coverage_collector) =
      self.shared.create_coverage_collector.as_ref()
    else {
      return Ok(None);
    };

    let session = self.worker.create_inspector_session();
    let mut coverage_collector = create_coverage_collector(session);
    self
      .worker
      .js_runtime()
      .with_event_loop_future(
        coverage_collector.start_collecting().boxed_local(),
        PollEventLoopOptions::default(),
      )
      .await?;
    Ok(Some(coverage_collector))
  }

  pub fn execute_script_static(
    &mut self,
    name: &'static str,
    source_code: &'static str,
  ) -> Result<v8::Global<v8::Value>, CoreError> {
    self.worker.js_runtime().execute_script(name, source_code)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CreateCustomWorkerError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  Core(#[from] CoreError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(
    #[from] deno_resolver::npm::ResolvePkgFolderFromDenoReqError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  UrlParse(#[from] deno_core::url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveNpmBinaryEntrypoint(#[from] ResolveNpmBinaryEntrypointError),
  #[class(inherit)]
  #[error(transparent)]
  NpmPackageReq(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  AtomicWriteFileWithRetries(
    #[from] crate::args::AtomicWriteFileWithRetriesError,
  ),
}

pub struct CliMainWorkerFactory {
  lib_main_worker_factory: LibMainWorkerFactory<CliSys>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  npm_installer: Option<Arc<NpmInstaller>>,
  npm_resolver: CliNpmResolver,
  root_permissions: PermissionsContainer,
  shared: Arc<SharedState>,
  sys: CliSys,
  default_npm_caching_strategy: NpmCachingStrategy,
  needs_test_modules: bool,
}

impl CliMainWorkerFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    lib_main_worker_factory: LibMainWorkerFactory<CliSys>,
    maybe_file_watcher_communicator: Option<Arc<WatcherCommunicator>>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    npm_installer: Option<Arc<NpmInstaller>>,
    npm_resolver: CliNpmResolver,
    sys: CliSys,
    options: CliMainWorkerOptions,
    root_permissions: PermissionsContainer,
  ) -> Self {
    Self {
      lib_main_worker_factory,
      maybe_lockfile,
      npm_installer,
      npm_resolver,
      root_permissions,
      sys,
      shared: Arc::new(SharedState {
        create_hmr_runner: options.create_hmr_runner,
        create_coverage_collector: options.create_coverage_collector,
        maybe_file_watcher_communicator,
      }),
      default_npm_caching_strategy: options.default_npm_caching_strategy,
      needs_test_modules: options.needs_test_modules,
    }
  }

  pub async fn create_main_worker(
    &self,
    mode: WorkerExecutionMode,
    main_module: ModuleSpecifier,
  ) -> Result<CliMainWorker, CreateCustomWorkerError> {
    self
      .create_custom_worker(
        mode,
        main_module,
        self.root_permissions.clone(),
        vec![],
        Default::default(),
      )
      .await
  }

  pub async fn create_custom_worker(
    &self,
    mode: WorkerExecutionMode,
    main_module: ModuleSpecifier,
    permissions: PermissionsContainer,
    custom_extensions: Vec<Extension>,
    stdio: deno_runtime::deno_io::Stdio,
  ) -> Result<CliMainWorker, CreateCustomWorkerError> {
    let main_module = if let Ok(package_ref) =
      NpmPackageReqReference::from_specifier(&main_module)
    {
      if let Some(npm_installer) = &self.npm_installer {
        let reqs = &[package_ref.req().clone()];
        npm_installer
          .add_package_reqs(
            reqs,
            if matches!(
              self.default_npm_caching_strategy,
              NpmCachingStrategy::Lazy
            ) {
              PackageCaching::Only(reqs.into())
            } else {
              PackageCaching::All
            },
          )
          .await
          .map_err(CreateCustomWorkerError::NpmPackageReq)?;
      }

      // use a fake referrer that can be used to discover the package.json if necessary
      let referrer =
        ModuleSpecifier::from_directory_path(self.sys.env_current_dir()?)
          .unwrap()
          .join("package.json")?;
      let package_folder =
        self.npm_resolver.resolve_pkg_folder_from_deno_module_req(
          package_ref.req(),
          &referrer,
        )?;
      let main_module =
        self.lib_main_worker_factory.resolve_npm_binary_entrypoint(
          &package_folder,
          package_ref.sub_path(),
        )?;

      if let Some(lockfile) = &self.maybe_lockfile {
        // For npm binary commands, ensure that the lockfile gets updated
        // so that we can re-use the npm resolution the next time it runs
        // for better performance
        lockfile.write_if_changed()?;
      }

      main_module
    } else {
      main_module
    };

    let mut worker = self.lib_main_worker_factory.create_custom_worker(
      mode,
      main_module,
      permissions,
      custom_extensions,
      stdio,
    )?;

    if self.needs_test_modules {
      macro_rules! test_file {
        ($($file:literal),*) => {
          $(worker.js_runtime().lazy_load_es_module_with_code(
            concat!("ext:cli/", $file),
            deno_core::ascii_str_include!(concat!("js/", $file)),
          )?;)*
        }
      }
      test_file!(
        "40_test_common.js",
        "40_test.js",
        "40_bench.js",
        "40_jupyter.js",
        // TODO(bartlomieju): probably shouldn't include these files here?
        "40_lint_selector.js",
        "40_lint.js"
      );
    }

    Ok(CliMainWorker {
      worker,
      shared: self.shared.clone(),
    })
  }
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use deno_core::resolve_path;
  use deno_core::FsModuleLoader;
  use deno_resolver::npm::DenoInNpmPackageChecker;
  use deno_runtime::deno_fs::RealFs;
  use deno_runtime::deno_permissions::Permissions;
  use deno_runtime::permissions::RuntimePermissionDescriptorParser;
  use deno_runtime::worker::WorkerOptions;
  use deno_runtime::worker::WorkerServiceOptions;

  use super::*;

  fn create_test_worker() -> MainWorker {
    let main_module =
      resolve_path("./hello.js", &std::env::current_dir().unwrap()).unwrap();
    let fs = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(
      RuntimePermissionDescriptorParser::new(crate::sys::CliSys::default()),
    );
    let options = WorkerOptions {
      startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
      ..Default::default()
    };

    MainWorker::bootstrap_from_options::<
      DenoInNpmPackageChecker,
      CliNpmResolver,
      CliSys,
    >(
      &main_module,
      WorkerServiceOptions {
        deno_rt_native_addon_loader: None,
        module_loader: Rc::new(FsModuleLoader),
        permissions: PermissionsContainer::new(
          permission_desc_parser,
          Permissions::none_without_prompt(),
        ),
        blob_store: Default::default(),
        broadcast_channel: Default::default(),
        feature_checker: Default::default(),
        node_services: Default::default(),
        npm_process_state_provider: Default::default(),
        root_cert_store_provider: Default::default(),
        fetch_dns_resolver: Default::default(),
        shared_array_buffer_store: Default::default(),
        compiled_wasm_module_store: Default::default(),
        v8_code_cache: Default::default(),
        fs,
      },
      options,
    )
  }

  #[tokio::test]
  async fn execute_mod_esm_imports_a() {
    let p = test_util::testdata_path().join("runtime/esm_imports_a.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {err:?}");
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {e:?}");
    }
  }

  #[tokio::test]
  async fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {err:?}");
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {e:?}");
    }
  }

  #[tokio::test]
  async fn execute_mod_resolve_error() {
    // "foo" is not a valid module specifier so this should return an error.
    let mut worker = create_test_worker();
    let module_specifier =
      resolve_path("./does-not-exist", &std::env::current_dir().unwrap())
        .unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let mut worker = create_test_worker();
    let p = test_util::testdata_path().join("run/001_hello.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_ok());
  }
}
