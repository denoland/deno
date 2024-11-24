// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::FeatureChecker;
use deno_core::ModuleLoader;
use deno_core::PollEventLoopOptions;
use deno_core::SharedArrayBufferStore;
use deno_runtime::code_cache;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodeExtInitServices;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::NodeRequireLoaderRc;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::PackageJsonResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::ops::process::NpmProcessStateProviderRc;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::web_worker::WebWorkerServiceOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;
use deno_runtime::BootstrapOptions;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::WorkerLogLevel;
use deno_semver::npm::NpmPackageReqReference;
use deno_telemetry::OtelConfig;
use deno_terminal::colors;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use tokio::select;

use crate::args::flags_from_vec;
use crate::args::CliLockfile;
use crate::args::DenoSubcommand;
use crate::args::StorageKeyResolver;
use crate::errors;
use crate::graph_container::MainModuleGraphContainer;
use crate::npm::CliNpmResolver;
use crate::util::checksum;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::file_watcher::WatcherRestartMode;
use crate::version;

use color_print::cformat;
use color_print::cstr;
use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::ResolutionKind;
use deno_runtime::fmt_errors::FixSuggestion;
use regex::Regex;

use crate::factory::CliFactory;
use crate::graph_container::ModuleGraphContainer;
use crate::util::fs::specifier_from_file_path;

pub struct CreateModuleLoaderResult {
  pub module_loader: Rc<dyn ModuleLoader>,
  pub node_require_loader: Rc<dyn NodeRequireLoader>,
}

pub trait ModuleLoaderFactory: Send + Sync {
  fn create_for_main(
    &self,
    root_permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult;

  fn maybe_main_module_graph_container(
    &self,
  ) -> Option<&Arc<MainModuleGraphContainer>>;

  fn create_for_worker(
    &self,
    parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult;
}

#[async_trait::async_trait(?Send)]
pub trait HmrRunner: Send + Sync {
  async fn start(&mut self) -> Result<(), AnyError>;
  async fn stop(&mut self) -> Result<(), AnyError>;
  async fn run(&mut self) -> Result<(), AnyError>;
}

pub trait CliCodeCache: code_cache::CodeCache {
  /// Gets if the code cache is still enabled.
  fn enabled(&self) -> bool {
    true
  }

  fn as_code_cache(self: Arc<Self>) -> Arc<dyn code_cache::CodeCache>;
}

#[async_trait::async_trait(?Send)]
pub trait CoverageCollector: Send + Sync {
  async fn start_collecting(&mut self) -> Result<(), AnyError>;
  async fn stop_collecting(&mut self) -> Result<(), AnyError>;
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
  pub argv: Vec<String>,
  pub log_level: WorkerLogLevel,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub has_node_modules_dir: bool,
  pub hmr: bool,
  pub inspect_brk: bool,
  pub inspect_wait: bool,
  pub strace_ops: Option<Vec<String>>,
  pub is_inspecting: bool,
  pub location: Option<Url>,
  pub argv0: Option<String>,
  pub node_debug: Option<String>,
  pub origin_data_folder_path: Option<PathBuf>,
  pub seed: Option<u64>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub skip_op_registration: bool,
  pub create_hmr_runner: Option<CreateHmrRunnerCb>,
  pub create_coverage_collector: Option<CreateCoverageCollectorCb>,
  pub node_ipc: Option<i64>,
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
}

struct SharedWorkerState {
  blob_store: Arc<BlobStore>,
  broadcast_channel: InMemoryBroadcastChannel,
  code_cache: Option<Arc<dyn CliCodeCache>>,
  compiled_wasm_module_store: CompiledWasmModuleStore,
  feature_checker: Arc<FeatureChecker>,
  fs: Arc<dyn deno_fs::FileSystem>,
  maybe_file_watcher_communicator: Option<Arc<WatcherCommunicator>>,
  maybe_inspector_server: Option<Arc<InspectorServer>>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  module_loader_factory: Box<dyn ModuleLoaderFactory>,
  node_resolver: Arc<NodeResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  pkg_json_resolver: Arc<PackageJsonResolver>,
  root_cert_store_provider: Arc<dyn RootCertStoreProvider>,
  root_permissions: PermissionsContainer,
  shared_array_buffer_store: SharedArrayBufferStore,
  storage_key_resolver: StorageKeyResolver,
  options: CliMainWorkerOptions,
  subcommand: DenoSubcommand,
  otel_config: Option<OtelConfig>, // `None` means OpenTelemetry is disabled.
}

impl SharedWorkerState {
  pub fn create_node_init_services(
    &self,
    node_require_loader: NodeRequireLoaderRc,
  ) -> NodeExtInitServices {
    NodeExtInitServices {
      node_require_loader,
      node_resolver: self.node_resolver.clone(),
      npm_resolver: self.npm_resolver.clone().into_npm_pkg_folder_resolver(),
      pkg_json_resolver: self.pkg_json_resolver.clone(),
    }
  }

  pub fn npm_process_state_provider(&self) -> NpmProcessStateProviderRc {
    self.npm_resolver.clone().into_process_state_provider()
  }
}

pub struct CliMainWorker {
  main_module: ModuleSpecifier,
  worker: MainWorker,
  shared: Arc<SharedWorkerState>,
}

impl CliMainWorker {
  pub fn into_main_worker(self) -> MainWorker {
    self.worker
  }

  pub async fn setup_repl(&mut self) -> Result<(), AnyError> {
    self.worker.run_event_loop(false).await?;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<i32, AnyError> {
    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;
    let mut maybe_hmr_runner = self.maybe_setup_hmr_runner().await?;

    log::debug!("main_module {}", self.main_module);

    self.execute_main_module().await?;
    self.worker.dispatch_load_event()?;

    loop {
      if let Some(hmr_runner) = maybe_hmr_runner.as_mut() {
        let watcher_communicator =
          self.shared.maybe_file_watcher_communicator.clone().unwrap();

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
          watcher_communicator
            .change_restart_mode(WatcherRestartMode::Automatic);
          return Err(e);
        }
      } else {
        self
          .worker
          .run_event_loop(maybe_coverage_collector.is_none())
          .await
          // to catch the error that caused by import call
          .map_err(|e| self.get_suggestions_for_terminal_errors(e))?;
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
        .js_runtime
        .with_event_loop_future(
          coverage_collector.stop_collecting().boxed_local(),
          PollEventLoopOptions::default(),
        )
        .await?;
    }
    if let Some(hmr_runner) = maybe_hmr_runner.as_mut() {
      self
        .worker
        .js_runtime
        .with_event_loop_future(
          hmr_runner.stop().boxed_local(),
          PollEventLoopOptions::default(),
        )
        .await?;
    }

    Ok(self.worker.exit_code())
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
        self.inner.execute_main_module().await?;
        self.inner.worker.dispatch_load_event()?;
        self.pending_unload = true;

        let result = loop {
          match self.inner.worker.run_event_loop(false).await {
            Ok(()) => {}
            // to catch the error that caused by import call
            Err(error) => {
              break Err(self.inner.get_suggestions_for_terminal_errors(error))
            }
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

  pub async fn execute_main_module(&mut self) -> Result<(), AnyError> {
    let id = self
      .worker
      .preload_main_module(&self.main_module)
      .await
      // to catch the error that caused by static import
      .map_err(|e| self.get_suggestions_for_terminal_errors(e))?;
    self.worker.evaluate_module(id).await
  }

  pub async fn execute_side_module(&mut self) -> Result<(), AnyError> {
    let id = self.worker.preload_side_module(&self.main_module).await?;
    self.worker.evaluate_module(id).await
  }

  pub async fn maybe_setup_hmr_runner(
    &mut self,
  ) -> Result<Option<Box<dyn HmrRunner>>, AnyError> {
    if !self.shared.options.hmr {
      return Ok(None);
    }
    let Some(setup_hmr_runner) = self.shared.options.create_hmr_runner.as_ref()
    else {
      return Ok(None);
    };

    let session = self.worker.create_inspector_session();

    let mut hmr_runner = setup_hmr_runner(session);

    self
      .worker
      .js_runtime
      .with_event_loop_future(
        hmr_runner.start().boxed_local(),
        PollEventLoopOptions::default(),
      )
      .await?;
    Ok(Some(hmr_runner))
  }

  pub async fn maybe_setup_coverage_collector(
    &mut self,
  ) -> Result<Option<Box<dyn CoverageCollector>>, AnyError> {
    let Some(create_coverage_collector) =
      self.shared.options.create_coverage_collector.as_ref()
    else {
      return Ok(None);
    };

    let session = self.worker.create_inspector_session();
    let mut coverage_collector = create_coverage_collector(session);
    self
      .worker
      .js_runtime
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
  ) -> Result<v8::Global<v8::Value>, AnyError> {
    self.worker.js_runtime.execute_script(name, source_code)
  }

  /// # Suggestions for non-explicit CommonJS imports
  ///
  /// A package may have CommonJS modules that are not all listed in the package.json exports.  
  /// In this case, it cannot be statically resolved when imported from ESM unless you include the extension of the target module.  
  /// So, this function suggests adding the extension of the target module to the imports or exports.
  ///
  /// reference: https://github.com/facebook/react/tree/f9e41e3a519f12cfdc3207e1df44e0d2d9602df9/packages/react-dom
  ///
  /// ```javascript
  /// // ❌
  /// import ReactDomServer from 'npm:react-dom@16.8.5/server';
  /// // ✅
  /// import ReactDomServer from 'npm:react-dom@16.8.5/server.js';
  /// ```
  ///
  /// ## Known limitation
  /// It cannot handle the case where the target module is an import call(dynamic import)
  /// which argument is not a string literal due to needing of the runtime evaluation.
  ///
  /// ```javascript
  /// // ❌
  /// const specifier = 'react-dom/server';
  /// import(specifier);
  /// // ✅
  /// import('react-dom/server');
  /// ```
  fn get_suggestions_for_non_explicit_cjs_import(
    &self,
    message: &str,
  ) -> Result<AnyError, AnyError> {
    // setup the utilities
    let args: Vec<_> = env::args_os().collect();
    let flags = Arc::new(flags_from_vec(args)?);
    let factory = CliFactory::from_flags(flags);
    let cjs_tracker = factory.cjs_tracker()?.clone();
    let fs = factory.fs().clone();
    let module_loader = self
      .shared
      .module_loader_factory
      .create_for_main(self.shared.root_permissions.clone())
      .module_loader;
    let main_module_graph_container = self
      .shared
      .module_loader_factory
      .maybe_main_module_graph_container()
      .ok_or_else(|| anyhow!("Cound not find ModuleGraphContainer"))?;

    let re = Regex::new(r"Unable to load (\S+) imported from (\S+)")?;
    let captures = re
      .captures(message)
      .ok_or_else(|| anyhow!("Could not capture the message"))?;
    let unable_to_load_file_name = captures
      .get(1)
      .map(|m| m.as_str())
      .ok_or_else(|| anyhow!("No matches the specifier"))?;

    let unable_to_load_path = Path::new(unable_to_load_file_name);
    let unable_to_load_specifier =
      specifier_from_file_path(unable_to_load_path)?;
    // TODO(Hajime-san): Use `with_added_extension` when it becomes stable.
    //
    // This extended implementation for `Path` defines an ad-hoc method with the same name,
    // since `with_added_extension` is currently only available in the nightly version.
    // This implementation should be replaced when it becomes stable.
    // https://github.com/rust-lang/rust/issues/127292
    trait PathExt {
      fn _with_added_extension(&self, extension: &str) -> PathBuf;
    }

    impl PathExt for Path {
      fn _with_added_extension(&self, extension: &str) -> PathBuf {
        let mut path = self.to_path_buf();

        let new_extension = match self.extension() {
          Some(ext) => {
            format!("{}.{}", ext.to_string_lossy(), extension)
          }
          None => extension.to_string(),
        };

        path.set_extension(new_extension);
        path
      }
    }

    // resolve the exact file that unable to load
    let extension = ["js", "cjs"]
      .iter()
      .find(|e| unable_to_load_path._with_added_extension(e).is_file())
      .ok_or_else(|| anyhow!("Cound not find the file"))?;
    let resolved_path = unable_to_load_path._with_added_extension(extension);
    let resolved_specifier = specifier_from_file_path(&resolved_path)?;

    let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
      specifier: resolved_specifier.clone(),
      text: fs
        .read_text_file_lossy_sync(&resolved_path, None)?
        .as_str()
        .into(),
      media_type: MediaType::from_specifier(&resolved_specifier),
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })?;

    // check the module that unable to load is CommonJS
    let is_cjs = cjs_tracker.is_cjs_with_known_is_script(
      &resolved_specifier,
      parsed_source.media_type(),
      parsed_source.compute_is_script(),
    )?;
    if !is_cjs {
      return Err(anyhow!("The module is not a CommonJS"));
    }

    let referrer = captures
      .get(2)
      .map(|m| m.as_str())
      .ok_or_else(|| anyhow!("No matches the referrer"))?;
    let graph = main_module_graph_container.graph();

    // search the raw specifier that causes the error in the graph
    let raw_specifier = graph
      .specifiers()
      .filter_map(|(_, module)| module.ok())
      .filter_map(|module| module.js())
      .filter(|js| {
        // check the referrer is ESM
        let Ok(is_cjs) = cjs_tracker.is_cjs_with_known_is_script(
          &js.specifier,
          js.media_type,
          js.is_script,
        ) else {
          return false;
        };

        !is_cjs
      })
      .map(|js| js.dependencies.clone())
      .find_map(|dependencies| {
        dependencies.iter().find_map(|(raw_specifier, dependency)| {
          let kind = if dependency.is_dynamic {
            ResolutionKind::DynamicImport
          } else {
            ResolutionKind::Import
          };
          let resolved_specifier =
            module_loader.resolve(raw_specifier, referrer, kind);
          if let Ok(resolved_specifier) = resolved_specifier {
            // it matches the exact file that unable to load
            if resolved_specifier == unable_to_load_specifier {
              return Some(raw_specifier.to_string());
            }
          }
          None
        })
      })
      .ok_or_else(|| anyhow!("Could not find the raw specifier"))?;

    // add the extension to the raw specifier
    let suggest_specifier = Path::new(raw_specifier.as_str())
      ._with_added_extension(extension)
      .to_string_lossy()
      .into_owned();
    let hint =
      cformat!("Did you mean to import <u>\"{}\"</>?", suggest_specifier);
    let suggestions = vec![
      FixSuggestion::info_multiline(&[
        "The module that you are trying to import seems to be a CommonJS.",
        cstr!(
        "However it is not listed in the <i>exports</> of <u>package.json</>."
      ),
        "So it cannot be statically resolved when imported from ESM.",
        "Consider to include an extension of the module.",
      ]),
      FixSuggestion::hint(&hint),
    ];

    let mut message = message.to_string();
    FixSuggestion::append_suggestion(&mut message, suggestions);
    let handled_error = anyhow!(message);

    Ok(handled_error)
  }

  /// This function should only used to map to the place where the error could caught.
  ///
  /// Keep in mind the function `get_suggestions_for_terminal_errors` in `runtime/fmt_errors.rs`
  /// behaves almost identically to this function.
  fn get_suggestions_for_terminal_errors(&self, e: AnyError) -> AnyError {
    let message = e.to_string();
    if message.contains("Unable to load") && message.contains("imported from") {
      let result = self.get_suggestions_for_non_explicit_cjs_import(&message);
      match result {
        Ok(handled_error) => handled_error,
        // return the original error
        Err(_) => e,
      }
    } else {
      e
    }
  }
}

// TODO(bartlomieju): this should be moved to some other place, added to avoid string
// duplication between worker setups and `deno info` output.
pub fn get_cache_storage_dir() -> PathBuf {
  // Note: we currently use temp_dir() to avoid managing storage size.
  std::env::temp_dir().join("deno_cache")
}

#[derive(Clone)]
pub struct CliMainWorkerFactory {
  shared: Arc<SharedWorkerState>,
}

impl CliMainWorkerFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    blob_store: Arc<BlobStore>,
    code_cache: Option<Arc<dyn CliCodeCache>>,
    feature_checker: Arc<FeatureChecker>,
    fs: Arc<dyn deno_fs::FileSystem>,
    maybe_file_watcher_communicator: Option<Arc<WatcherCommunicator>>,
    maybe_inspector_server: Option<Arc<InspectorServer>>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    module_loader_factory: Box<dyn ModuleLoaderFactory>,
    node_resolver: Arc<NodeResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
    pkg_json_resolver: Arc<PackageJsonResolver>,
    root_cert_store_provider: Arc<dyn RootCertStoreProvider>,
    root_permissions: PermissionsContainer,
    storage_key_resolver: StorageKeyResolver,
    subcommand: DenoSubcommand,
    options: CliMainWorkerOptions,
    otel_config: Option<OtelConfig>,
  ) -> Self {
    Self {
      shared: Arc::new(SharedWorkerState {
        blob_store,
        broadcast_channel: Default::default(),
        code_cache,
        compiled_wasm_module_store: Default::default(),
        feature_checker,
        fs,
        maybe_file_watcher_communicator,
        maybe_inspector_server,
        maybe_lockfile,
        module_loader_factory,
        node_resolver,
        npm_resolver,
        pkg_json_resolver,
        root_cert_store_provider,
        root_permissions,
        shared_array_buffer_store: Default::default(),
        storage_key_resolver,
        options,
        subcommand,
        otel_config,
      }),
    }
  }

  pub async fn create_main_worker(
    &self,
    mode: WorkerExecutionMode,
    main_module: ModuleSpecifier,
  ) -> Result<CliMainWorker, AnyError> {
    self
      .create_custom_worker(
        mode,
        main_module,
        self.shared.root_permissions.clone(),
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
  ) -> Result<CliMainWorker, AnyError> {
    let shared = &self.shared;
    let CreateModuleLoaderResult {
      module_loader,
      node_require_loader,
    } = shared
      .module_loader_factory
      .create_for_main(permissions.clone());
    let main_module = if let Ok(package_ref) =
      NpmPackageReqReference::from_specifier(&main_module)
    {
      if let Some(npm_resolver) = shared.npm_resolver.as_managed() {
        npm_resolver
          .add_package_reqs(&[package_ref.req().clone()])
          .await?;
      }

      // use a fake referrer that can be used to discover the package.json if necessary
      let referrer =
        ModuleSpecifier::from_directory_path(self.shared.fs.cwd()?)
          .unwrap()
          .join("package.json")?;
      let package_folder = shared
        .npm_resolver
        .resolve_pkg_folder_from_deno_module_req(
          package_ref.req(),
          &referrer,
        )?;
      let main_module = self
        .resolve_binary_entrypoint(&package_folder, package_ref.sub_path())?;

      if let Some(lockfile) = &shared.maybe_lockfile {
        // For npm binary commands, ensure that the lockfile gets updated
        // so that we can re-use the npm resolution the next time it runs
        // for better performance
        lockfile.write_if_changed()?;
      }

      main_module
    } else {
      main_module
    };

    let maybe_inspector_server = shared.maybe_inspector_server.clone();

    let create_web_worker_cb =
      create_web_worker_callback(shared.clone(), stdio.clone());

    let maybe_storage_key = shared
      .storage_key_resolver
      .resolve_storage_key(&main_module);
    let origin_storage_dir = maybe_storage_key.as_ref().map(|key| {
      shared
        .options
        .origin_data_folder_path
        .as_ref()
        .unwrap() // must be set if storage key resolver returns a value
        .join(checksum::gen(&[key.as_bytes()]))
    });
    let cache_storage_dir = maybe_storage_key.map(|key| {
      // TODO(@satyarohith): storage quota management
      get_cache_storage_dir().join(checksum::gen(&[key.as_bytes()]))
    });

    // TODO(bartlomieju): this is cruft, update FeatureChecker to spit out
    // list of enabled features.
    let feature_checker = shared.feature_checker.clone();
    let mut unstable_features =
      Vec::with_capacity(crate::UNSTABLE_GRANULAR_FLAGS.len());
    for granular_flag in crate::UNSTABLE_GRANULAR_FLAGS {
      if feature_checker.check(granular_flag.name) {
        unstable_features.push(granular_flag.id);
      }
    }

    let services = WorkerServiceOptions {
      root_cert_store_provider: Some(shared.root_cert_store_provider.clone()),
      module_loader,
      fs: shared.fs.clone(),
      node_services: Some(
        shared.create_node_init_services(node_require_loader),
      ),
      npm_process_state_provider: Some(shared.npm_process_state_provider()),
      blob_store: shared.blob_store.clone(),
      broadcast_channel: shared.broadcast_channel.clone(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Some(shared.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(
        shared.compiled_wasm_module_store.clone(),
      ),
      feature_checker,
      permissions,
      v8_code_cache: shared.code_cache.clone().map(|c| c.as_code_cache()),
    };

    let options = WorkerOptions {
      bootstrap: BootstrapOptions {
        deno_version: crate::version::DENO_VERSION_INFO.deno.to_string(),
        args: shared.options.argv.clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        log_level: shared.options.log_level,
        enable_op_summary_metrics: shared.options.enable_op_summary_metrics,
        enable_testing_features: shared.options.enable_testing_features,
        locale: deno_core::v8::icu::get_language_tag(),
        location: shared.options.location.clone(),
        no_color: !colors::use_color(),
        is_stdout_tty: deno_terminal::is_stdout_tty(),
        is_stderr_tty: deno_terminal::is_stderr_tty(),
        color_level: colors::get_color_level(),
        unstable_features,
        user_agent: version::DENO_VERSION_INFO.user_agent.to_string(),
        inspect: shared.options.is_inspecting,
        has_node_modules_dir: shared.options.has_node_modules_dir,
        argv0: shared.options.argv0.clone(),
        node_debug: shared.options.node_debug.clone(),
        node_ipc_fd: shared.options.node_ipc,
        mode,
        serve_port: shared.options.serve_port,
        serve_host: shared.options.serve_host.clone(),
        otel_config: shared.otel_config.clone(),
      },
      extensions: custom_extensions,
      startup_snapshot: crate::js::deno_isolate_init(),
      create_params: create_isolate_create_params(),
      unsafely_ignore_certificate_errors: shared
        .options
        .unsafely_ignore_certificate_errors
        .clone(),
      seed: shared.options.seed,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      create_web_worker_cb,
      maybe_inspector_server,
      should_break_on_first_statement: shared.options.inspect_brk,
      should_wait_for_inspector_session: shared.options.inspect_wait,
      strace_ops: shared.options.strace_ops.clone(),
      get_error_class_fn: Some(&errors::get_error_class_name),
      cache_storage_dir,
      origin_storage_dir,
      stdio,
      skip_op_registration: shared.options.skip_op_registration,
      enable_stack_trace_arg_in_ops: crate::args::has_trace_permissions_enabled(
      ),
    };

    let mut worker = MainWorker::bootstrap_from_options(
      main_module.clone(),
      services,
      options,
    );

    if self.shared.subcommand.needs_test() {
      macro_rules! test_file {
        ($($file:literal),*) => {
          $(worker.js_runtime.lazy_load_es_module_with_code(
            concat!("ext:cli/", $file),
            deno_core::ascii_str_include!(concat!("js/", $file)),
          )?;)*
        }
      }
      test_file!(
        "40_test_common.js",
        "40_test.js",
        "40_bench.js",
        "40_jupyter.js"
      );
    }

    Ok(CliMainWorker {
      main_module,
      worker,
      shared: shared.clone(),
    })
  }

  fn resolve_binary_entrypoint(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<ModuleSpecifier, AnyError> {
    match self
      .shared
      .node_resolver
      .resolve_binary_export(package_folder, sub_path)
    {
      Ok(specifier) => Ok(specifier),
      Err(original_err) => {
        // if the binary entrypoint was not found, fallback to regular node resolution
        let result =
          self.resolve_binary_entrypoint_fallback(package_folder, sub_path);
        match result {
          Ok(Some(specifier)) => Ok(specifier),
          Ok(None) => Err(original_err.into()),
          Err(fallback_err) => {
            bail!("{:#}\n\nFallback failed: {:#}", original_err, fallback_err)
          }
        }
      }
    }
  }

  /// resolve the binary entrypoint using regular node resolution
  fn resolve_binary_entrypoint_fallback(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<Option<ModuleSpecifier>, AnyError> {
    // only fallback if the user specified a sub path
    if sub_path.is_none() {
      // it's confusing to users if the package doesn't have any binary
      // entrypoint and we just execute the main script which will likely
      // have blank output, so do not resolve the entrypoint in this case
      return Ok(None);
    }

    let specifier = self
      .shared
      .node_resolver
      .resolve_package_subpath_from_deno_module(
        package_folder,
        sub_path,
        /* referrer */ None,
        ResolutionMode::Import,
        NodeResolutionKind::Execution,
      )?;
    if specifier
      .to_file_path()
      .map(|p| p.exists())
      .unwrap_or(false)
    {
      Ok(Some(specifier))
    } else {
      bail!("Cannot find module '{}'", specifier)
    }
  }
}

fn create_web_worker_callback(
  shared: Arc<SharedWorkerState>,
  stdio: deno_runtime::deno_io::Stdio,
) -> Arc<CreateWebWorkerCb> {
  Arc::new(move |args| {
    let maybe_inspector_server = shared.maybe_inspector_server.clone();

    let CreateModuleLoaderResult {
      module_loader,
      node_require_loader,
    } = shared.module_loader_factory.create_for_worker(
      args.parent_permissions.clone(),
      args.permissions.clone(),
    );
    let create_web_worker_cb =
      create_web_worker_callback(shared.clone(), stdio.clone());

    let maybe_storage_key = shared
      .storage_key_resolver
      .resolve_storage_key(&args.main_module);
    let cache_storage_dir = maybe_storage_key.map(|key| {
      // TODO(@satyarohith): storage quota management
      get_cache_storage_dir().join(checksum::gen(&[key.as_bytes()]))
    });

    // TODO(bartlomieju): this is cruft, update FeatureChecker to spit out
    // list of enabled features.
    let feature_checker = shared.feature_checker.clone();
    let mut unstable_features =
      Vec::with_capacity(crate::UNSTABLE_GRANULAR_FLAGS.len());
    for granular_flag in crate::UNSTABLE_GRANULAR_FLAGS {
      if feature_checker.check(granular_flag.name) {
        unstable_features.push(granular_flag.id);
      }
    }

    let services = WebWorkerServiceOptions {
      root_cert_store_provider: Some(shared.root_cert_store_provider.clone()),
      module_loader,
      fs: shared.fs.clone(),
      node_services: Some(
        shared.create_node_init_services(node_require_loader),
      ),
      blob_store: shared.blob_store.clone(),
      broadcast_channel: shared.broadcast_channel.clone(),
      shared_array_buffer_store: Some(shared.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(
        shared.compiled_wasm_module_store.clone(),
      ),
      maybe_inspector_server,
      feature_checker,
      npm_process_state_provider: Some(shared.npm_process_state_provider()),
      permissions: args.permissions,
    };
    let options = WebWorkerOptions {
      name: args.name,
      main_module: args.main_module.clone(),
      worker_id: args.worker_id,
      bootstrap: BootstrapOptions {
        deno_version: crate::version::DENO_VERSION_INFO.deno.to_string(),
        args: shared.options.argv.clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        log_level: shared.options.log_level,
        enable_op_summary_metrics: shared.options.enable_op_summary_metrics,
        enable_testing_features: shared.options.enable_testing_features,
        locale: deno_core::v8::icu::get_language_tag(),
        location: Some(args.main_module),
        no_color: !colors::use_color(),
        color_level: colors::get_color_level(),
        is_stdout_tty: deno_terminal::is_stdout_tty(),
        is_stderr_tty: deno_terminal::is_stderr_tty(),
        unstable_features,
        user_agent: version::DENO_VERSION_INFO.user_agent.to_string(),
        inspect: shared.options.is_inspecting,
        has_node_modules_dir: shared.options.has_node_modules_dir,
        argv0: shared.options.argv0.clone(),
        node_debug: shared.options.node_debug.clone(),
        node_ipc_fd: None,
        mode: WorkerExecutionMode::Worker,
        serve_port: shared.options.serve_port,
        serve_host: shared.options.serve_host.clone(),
        otel_config: shared.otel_config.clone(),
      },
      extensions: vec![],
      startup_snapshot: crate::js::deno_isolate_init(),
      create_params: create_isolate_create_params(),
      unsafely_ignore_certificate_errors: shared
        .options
        .unsafely_ignore_certificate_errors
        .clone(),
      seed: shared.options.seed,
      create_web_worker_cb,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      worker_type: args.worker_type,
      get_error_class_fn: Some(&errors::get_error_class_name),
      stdio: stdio.clone(),
      cache_storage_dir,
      strace_ops: shared.options.strace_ops.clone(),
      close_on_idle: args.close_on_idle,
      maybe_worker_metadata: args.maybe_worker_metadata,
      enable_stack_trace_arg_in_ops: crate::args::has_trace_permissions_enabled(
      ),
    };

    WebWorker::bootstrap_from_options(services, options)
  })
}

/// By default V8 uses 1.4Gb heap limit which is meant for browser tabs.
/// Instead probe for the total memory on the system and use it instead
/// as a default.
pub fn create_isolate_create_params() -> Option<v8::CreateParams> {
  let maybe_mem_info = deno_runtime::sys_info::mem_info();
  maybe_mem_info.map(|mem_info| {
    v8::CreateParams::default()
      .heap_limits_from_system_memory(mem_info.total, 0)
  })
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_path;
  use deno_core::FsModuleLoader;
  use deno_fs::RealFs;
  use deno_runtime::deno_permissions::Permissions;
  use deno_runtime::permissions::RuntimePermissionDescriptorParser;

  fn create_test_worker() -> MainWorker {
    let main_module =
      resolve_path("./hello.js", &std::env::current_dir().unwrap()).unwrap();
    let fs = Arc::new(RealFs);
    let permission_desc_parser =
      Arc::new(RuntimePermissionDescriptorParser::new(fs.clone()));
    let options = WorkerOptions {
      startup_snapshot: crate::js::deno_isolate_init(),
      ..Default::default()
    };

    MainWorker::bootstrap_from_options(
      main_module,
      WorkerServiceOptions {
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
