// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::error::JsError;
use deno_node::NodeRequireLoaderRc;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;
use deno_runtime::colors;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_core;
use deno_runtime::deno_core::error::CoreError;
use deno_runtime::deno_core::v8;
use deno_runtime::deno_core::CompiledWasmModuleStore;
use deno_runtime::deno_core::Extension;
use deno_runtime::deno_core::FeatureChecker;
use deno_runtime::deno_core::JsRuntime;
use deno_runtime::deno_core::LocalInspectorSession;
use deno_runtime::deno_core::ModuleLoader;
use deno_runtime::deno_core::SharedArrayBufferStore;
use deno_runtime::deno_fs;
use deno_runtime::deno_napi::DenoRtNativeAddonLoaderRc;
use deno_runtime::deno_node::NodeExtInitServices;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_process::NpmProcessStateProviderRc;
use deno_runtime::deno_telemetry::OtelConfig;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::inspector_server::InspectorServer;
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
use deno_runtime::UNSTABLE_GRANULAR_FLAGS;
use node_resolver::errors::ResolvePkgJsonBinExportError;
use node_resolver::UrlOrPath;
use url::Url;

use crate::args::has_trace_permissions_enabled;
use crate::sys::DenoLibSys;
use crate::util::checksum;

pub struct CreateModuleLoaderResult {
  pub module_loader: Rc<dyn ModuleLoader>,
  pub node_require_loader: Rc<dyn NodeRequireLoader>,
}

pub trait ModuleLoaderFactory: Send + Sync {
  fn create_for_main(
    &self,
    root_permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult;

  fn create_for_worker(
    &self,
    parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult;
}

enum StorageKeyResolverStrategy {
  Specified(Option<String>),
  UseMainModule,
}

pub struct StorageKeyResolver(StorageKeyResolverStrategy);

impl StorageKeyResolver {
  pub fn from_flag(location: &Url) -> Self {
    // if a location is set, then the ascii serialization of the location is
    // used, unless the origin is opaque, and then no storage origin is set, as
    // we can't expect the origin to be reproducible
    let storage_origin = location.origin();
    Self(StorageKeyResolverStrategy::Specified(
      if storage_origin.is_tuple() {
        Some(storage_origin.ascii_serialization())
      } else {
        None
      },
    ))
  }

  pub fn from_config_file_url(url: &Url) -> Self {
    Self(StorageKeyResolverStrategy::Specified(Some(url.to_string())))
  }

  pub fn new_use_main_module() -> Self {
    Self(StorageKeyResolverStrategy::UseMainModule)
  }

  /// Creates a storage key resolver that will always resolve to being empty.
  pub fn empty() -> Self {
    Self(StorageKeyResolverStrategy::Specified(None))
  }

  /// Resolves the storage key to use based on the current flags, config, or main module.
  pub fn resolve_storage_key(&self, main_module: &Url) -> Option<String> {
    // use the stored value or fall back to using the path of the main module.
    match &self.0 {
      StorageKeyResolverStrategy::Specified(value) => value.clone(),
      StorageKeyResolverStrategy::UseMainModule => {
        Some(main_module.to_string())
      }
    }
  }
}

pub fn get_cache_storage_dir() -> PathBuf {
  // ok because this won't ever be used by the js runtime
  #[allow(clippy::disallowed_methods)]
  // Note: we currently use temp_dir() to avoid managing storage size.
  std::env::temp_dir().join("deno_cache")
}

/// By default V8 uses 1.4Gb heap limit which is meant for browser tabs.
/// Instead probe for the total memory on the system and use it instead
/// as a default.
pub fn create_isolate_create_params() -> Option<v8::CreateParams> {
  let maybe_mem_info = deno_runtime::deno_os::sys_info::mem_info();
  maybe_mem_info.map(|mem_info| {
    v8::CreateParams::default()
      .heap_limits_from_system_memory(mem_info.total, 0)
  })
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolveNpmBinaryEntrypointError {
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgJsonBinExport(ResolvePkgJsonBinExportError),
  #[class(generic)]
  #[error("{original:#}\n\nFallback failed: {fallback:#}")]
  Fallback {
    fallback: ResolveNpmBinaryEntrypointFallbackError,
    original: ResolvePkgJsonBinExportError,
  },
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolveNpmBinaryEntrypointFallbackError {
  #[class(inherit)]
  #[error(transparent)]
  PackageSubpathResolve(node_resolver::errors::PackageSubpathResolveError),
  #[class(generic)]
  #[error("Cannot find module '{0}'")]
  ModuleNotFound(UrlOrPath),
}

pub struct LibMainWorkerOptions {
  pub argv: Vec<String>,
  pub log_level: WorkerLogLevel,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub has_node_modules_dir: bool,
  pub inspect_brk: bool,
  pub inspect_wait: bool,
  pub strace_ops: Option<Vec<String>>,
  pub is_inspecting: bool,
  /// If this is a `deno compile`-ed executable.
  pub is_standalone: bool,
  pub location: Option<Url>,
  pub argv0: Option<String>,
  pub node_debug: Option<String>,
  pub otel_config: OtelConfig,
  pub origin_data_folder_path: Option<PathBuf>,
  pub seed: Option<u64>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub skip_op_registration: bool,
  pub node_ipc: Option<i64>,
  pub no_legacy_abort: bool,
  pub startup_snapshot: Option<&'static [u8]>,
  pub serve_port: Option<u16>,
  pub serve_host: Option<String>,
}

struct LibWorkerFactorySharedState<TSys: DenoLibSys> {
  blob_store: Arc<BlobStore>,
  broadcast_channel: InMemoryBroadcastChannel,
  code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
  compiled_wasm_module_store: CompiledWasmModuleStore,
  deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  feature_checker: Arc<FeatureChecker>,
  fs: Arc<dyn deno_fs::FileSystem>,
  maybe_inspector_server: Option<Arc<InspectorServer>>,
  module_loader_factory: Box<dyn ModuleLoaderFactory>,
  node_resolver:
    Arc<NodeResolver<DenoInNpmPackageChecker, NpmResolver<TSys>, TSys>>,
  npm_process_state_provider: NpmProcessStateProviderRc,
  pkg_json_resolver: Arc<node_resolver::PackageJsonResolver<TSys>>,
  root_cert_store_provider: Arc<dyn RootCertStoreProvider>,
  shared_array_buffer_store: SharedArrayBufferStore,
  storage_key_resolver: StorageKeyResolver,
  sys: TSys,
  options: LibMainWorkerOptions,
}

impl<TSys: DenoLibSys> LibWorkerFactorySharedState<TSys> {
  fn resolve_unstable_features(
    &self,
    feature_checker: &FeatureChecker,
  ) -> Vec<i32> {
    let mut unstable_features =
      Vec::with_capacity(UNSTABLE_GRANULAR_FLAGS.len());
    for granular_flag in UNSTABLE_GRANULAR_FLAGS {
      if feature_checker.check(granular_flag.name) {
        unstable_features.push(granular_flag.id);
      }
    }
    unstable_features
  }

  fn create_node_init_services(
    &self,
    node_require_loader: NodeRequireLoaderRc,
  ) -> NodeExtInitServices<DenoInNpmPackageChecker, NpmResolver<TSys>, TSys> {
    NodeExtInitServices {
      node_require_loader,
      node_resolver: self.node_resolver.clone(),
      pkg_json_resolver: self.pkg_json_resolver.clone(),
      sys: self.sys.clone(),
    }
  }

  fn create_web_worker_callback(
    self: &Arc<Self>,
    stdio: deno_runtime::deno_io::Stdio,
  ) -> Arc<CreateWebWorkerCb> {
    let shared = self.clone();
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
        shared.create_web_worker_callback(stdio.clone());

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
      let unstable_features =
        shared.resolve_unstable_features(feature_checker.as_ref());

      let services = WebWorkerServiceOptions {
        deno_rt_native_addon_loader: shared.deno_rt_native_addon_loader.clone(),
        root_cert_store_provider: Some(shared.root_cert_store_provider.clone()),
        module_loader,
        fs: shared.fs.clone(),
        node_services: Some(
          shared.create_node_init_services(node_require_loader),
        ),
        blob_store: shared.blob_store.clone(),
        broadcast_channel: shared.broadcast_channel.clone(),
        shared_array_buffer_store: Some(
          shared.shared_array_buffer_store.clone(),
        ),
        compiled_wasm_module_store: Some(
          shared.compiled_wasm_module_store.clone(),
        ),
        maybe_inspector_server,
        feature_checker,
        npm_process_state_provider: Some(
          shared.npm_process_state_provider.clone(),
        ),
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
          color_level: colors::get_color_level(),
          unstable_features,
          user_agent: crate::version::DENO_VERSION_INFO.user_agent.to_string(),
          inspect: shared.options.is_inspecting,
          is_standalone: shared.options.is_standalone,
          has_node_modules_dir: shared.options.has_node_modules_dir,
          argv0: shared.options.argv0.clone(),
          node_debug: shared.options.node_debug.clone(),
          node_ipc_fd: None,
          mode: WorkerExecutionMode::Worker,
          serve_port: shared.options.serve_port,
          serve_host: shared.options.serve_host.clone(),
          otel_config: shared.options.otel_config.clone(),
          no_legacy_abort: shared.options.no_legacy_abort,
          close_on_idle: args.close_on_idle,
        },
        extensions: vec![],
        startup_snapshot: shared.options.startup_snapshot,
        create_params: create_isolate_create_params(),
        unsafely_ignore_certificate_errors: shared
          .options
          .unsafely_ignore_certificate_errors
          .clone(),
        seed: shared.options.seed,
        create_web_worker_cb,
        format_js_error_fn: Some(Arc::new(format_js_error)),
        worker_type: args.worker_type,
        stdio: stdio.clone(),
        cache_storage_dir,
        strace_ops: shared.options.strace_ops.clone(),
        close_on_idle: args.close_on_idle,
        maybe_worker_metadata: args.maybe_worker_metadata,
        enable_stack_trace_arg_in_ops: has_trace_permissions_enabled(),
      };

      WebWorker::bootstrap_from_options(services, options)
    })
  }
}

pub struct LibMainWorkerFactory<TSys: DenoLibSys> {
  shared: Arc<LibWorkerFactorySharedState<TSys>>,
}

impl<TSys: DenoLibSys> LibMainWorkerFactory<TSys> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    blob_store: Arc<BlobStore>,
    code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
    deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
    feature_checker: Arc<FeatureChecker>,
    fs: Arc<dyn deno_fs::FileSystem>,
    maybe_inspector_server: Option<Arc<InspectorServer>>,
    module_loader_factory: Box<dyn ModuleLoaderFactory>,
    node_resolver: Arc<
      NodeResolver<DenoInNpmPackageChecker, NpmResolver<TSys>, TSys>,
    >,
    npm_process_state_provider: NpmProcessStateProviderRc,
    pkg_json_resolver: Arc<node_resolver::PackageJsonResolver<TSys>>,
    root_cert_store_provider: Arc<dyn RootCertStoreProvider>,
    storage_key_resolver: StorageKeyResolver,
    sys: TSys,
    options: LibMainWorkerOptions,
  ) -> Self {
    Self {
      shared: Arc::new(LibWorkerFactorySharedState {
        blob_store,
        broadcast_channel: Default::default(),
        code_cache,
        compiled_wasm_module_store: Default::default(),
        deno_rt_native_addon_loader,
        feature_checker,
        fs,
        maybe_inspector_server,
        module_loader_factory,
        node_resolver,
        npm_process_state_provider,
        pkg_json_resolver,
        root_cert_store_provider,
        shared_array_buffer_store: Default::default(),
        storage_key_resolver,
        sys,
        options,
      }),
    }
  }

  pub fn create_main_worker(
    &self,
    mode: WorkerExecutionMode,
    permissions: PermissionsContainer,
    main_module: Url,
  ) -> Result<LibMainWorker, CoreError> {
    self.create_custom_worker(
      mode,
      main_module,
      permissions,
      vec![],
      Default::default(),
    )
  }

  pub fn create_custom_worker(
    &self,
    mode: WorkerExecutionMode,
    main_module: Url,
    permissions: PermissionsContainer,
    custom_extensions: Vec<Extension>,
    stdio: deno_runtime::deno_io::Stdio,
  ) -> Result<LibMainWorker, CoreError> {
    let shared = &self.shared;
    let CreateModuleLoaderResult {
      module_loader,
      node_require_loader,
    } = shared
      .module_loader_factory
      .create_for_main(permissions.clone());

    // TODO(bartlomieju): this is cruft, update FeatureChecker to spit out
    // list of enabled features.
    let feature_checker = shared.feature_checker.clone();
    let unstable_features =
      shared.resolve_unstable_features(feature_checker.as_ref());
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

    let services = WorkerServiceOptions {
      deno_rt_native_addon_loader: shared.deno_rt_native_addon_loader.clone(),
      root_cert_store_provider: Some(shared.root_cert_store_provider.clone()),
      module_loader,
      fs: shared.fs.clone(),
      node_services: Some(
        shared.create_node_init_services(node_require_loader),
      ),
      npm_process_state_provider: Some(
        shared.npm_process_state_provider.clone(),
      ),
      blob_store: shared.blob_store.clone(),
      broadcast_channel: shared.broadcast_channel.clone(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Some(shared.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(
        shared.compiled_wasm_module_store.clone(),
      ),
      feature_checker,
      permissions,
      v8_code_cache: shared.code_cache.clone(),
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
        color_level: colors::get_color_level(),
        unstable_features,
        user_agent: crate::version::DENO_VERSION_INFO.user_agent.to_string(),
        inspect: shared.options.is_inspecting,
        is_standalone: shared.options.is_standalone,
        has_node_modules_dir: shared.options.has_node_modules_dir,
        argv0: shared.options.argv0.clone(),
        node_debug: shared.options.node_debug.clone(),
        node_ipc_fd: shared.options.node_ipc,
        mode,
        no_legacy_abort: shared.options.no_legacy_abort,
        serve_port: shared.options.serve_port,
        serve_host: shared.options.serve_host.clone(),
        otel_config: shared.options.otel_config.clone(),
        close_on_idle: true,
      },
      extensions: custom_extensions,
      startup_snapshot: shared.options.startup_snapshot,
      create_params: create_isolate_create_params(),
      unsafely_ignore_certificate_errors: shared
        .options
        .unsafely_ignore_certificate_errors
        .clone(),
      seed: shared.options.seed,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      create_web_worker_cb: shared.create_web_worker_callback(stdio.clone()),
      maybe_inspector_server: shared.maybe_inspector_server.clone(),
      should_break_on_first_statement: shared.options.inspect_brk,
      should_wait_for_inspector_session: shared.options.inspect_wait,
      strace_ops: shared.options.strace_ops.clone(),
      cache_storage_dir,
      origin_storage_dir,
      stdio,
      skip_op_registration: shared.options.skip_op_registration,
      enable_stack_trace_arg_in_ops: has_trace_permissions_enabled(),
    };

    let worker =
      MainWorker::bootstrap_from_options(&main_module, services, options);

    Ok(LibMainWorker {
      main_module,
      worker,
    })
  }

  pub fn resolve_npm_binary_entrypoint(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<Url, ResolveNpmBinaryEntrypointError> {
    match self
      .shared
      .node_resolver
      .resolve_binary_export(package_folder, sub_path)
    {
      Ok(path) => Ok(url_from_file_path(&path)?),
      Err(original_err) => {
        // if the binary entrypoint was not found, fallback to regular node resolution
        let result =
          self.resolve_binary_entrypoint_fallback(package_folder, sub_path);
        match result {
          Ok(Some(path)) => Ok(url_from_file_path(&path)?),
          Ok(None) => {
            Err(ResolveNpmBinaryEntrypointError::ResolvePkgJsonBinExport(
              original_err,
            ))
          }
          Err(fallback_err) => Err(ResolveNpmBinaryEntrypointError::Fallback {
            original: original_err,
            fallback: fallback_err,
          }),
        }
      }
    }
  }

  /// resolve the binary entrypoint using regular node resolution
  fn resolve_binary_entrypoint_fallback(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
  ) -> Result<Option<PathBuf>, ResolveNpmBinaryEntrypointFallbackError> {
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
        node_resolver::ResolutionMode::Import,
        node_resolver::NodeResolutionKind::Execution,
      )
      .map_err(
        ResolveNpmBinaryEntrypointFallbackError::PackageSubpathResolve,
      )?;
    let path = match specifier {
      UrlOrPath::Url(ref url) => match url_to_file_path(url) {
        Ok(path) => path,
        Err(_) => {
          return Err(ResolveNpmBinaryEntrypointFallbackError::ModuleNotFound(
            specifier,
          ));
        }
      },
      UrlOrPath::Path(path) => path,
    };
    if self.shared.sys.fs_exists_no_err(&path) {
      Ok(Some(path))
    } else {
      Err(ResolveNpmBinaryEntrypointFallbackError::ModuleNotFound(
        UrlOrPath::Path(path),
      ))
    }
  }
}

pub struct LibMainWorker {
  main_module: Url,
  worker: MainWorker,
}

impl LibMainWorker {
  pub fn into_main_worker(self) -> MainWorker {
    self.worker
  }

  pub fn main_module(&self) -> &Url {
    &self.main_module
  }

  pub fn js_runtime(&mut self) -> &mut JsRuntime {
    &mut self.worker.js_runtime
  }

  #[inline]
  pub fn create_inspector_session(&mut self) -> LocalInspectorSession {
    self.worker.create_inspector_session()
  }

  #[inline]
  pub fn dispatch_load_event(&mut self) -> Result<(), JsError> {
    self.worker.dispatch_load_event()
  }

  #[inline]
  pub fn dispatch_beforeunload_event(&mut self) -> Result<bool, JsError> {
    self.worker.dispatch_beforeunload_event()
  }

  #[inline]
  pub fn dispatch_process_beforeexit_event(&mut self) -> Result<bool, JsError> {
    self.worker.dispatch_process_beforeexit_event()
  }

  #[inline]
  pub fn dispatch_unload_event(&mut self) -> Result<(), JsError> {
    self.worker.dispatch_unload_event()
  }

  #[inline]
  pub fn dispatch_process_exit_event(&mut self) -> Result<(), JsError> {
    self.worker.dispatch_process_exit_event()
  }

  pub async fn execute_main_module(&mut self) -> Result<(), CoreError> {
    let id = self.worker.preload_main_module(&self.main_module).await?;
    self.worker.evaluate_module(id).await
  }

  pub async fn execute_side_module(&mut self) -> Result<(), CoreError> {
    let id = self.worker.preload_side_module(&self.main_module).await?;
    self.worker.evaluate_module(id).await
  }

  pub async fn run(&mut self) -> Result<i32, CoreError> {
    log::debug!("main_module {}", self.main_module);

    self.execute_main_module().await?;
    self.worker.dispatch_load_event()?;

    loop {
      self
        .worker
        .run_event_loop(/* wait for inspector */ false)
        .await?;

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

    Ok(self.worker.exit_code())
  }

  #[inline]
  pub async fn run_event_loop(
    &mut self,
    wait_for_inspector: bool,
  ) -> Result<(), CoreError> {
    self.worker.run_event_loop(wait_for_inspector).await
  }

  #[inline]
  pub fn exit_code(&self) -> i32 {
    self.worker.exit_code()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn storage_key_resolver_test() {
    let resolver =
      StorageKeyResolver(StorageKeyResolverStrategy::UseMainModule);
    let specifier = Url::parse("file:///a.ts").unwrap();
    assert_eq!(
      resolver.resolve_storage_key(&specifier),
      Some(specifier.to_string())
    );
    let resolver =
      StorageKeyResolver(StorageKeyResolverStrategy::Specified(None));
    assert_eq!(resolver.resolve_storage_key(&specifier), None);
    let resolver = StorageKeyResolver(StorageKeyResolverStrategy::Specified(
      Some("value".to_string()),
    ));
    assert_eq!(
      resolver.resolve_storage_key(&specifier),
      Some("value".to_string())
    );

    // test empty
    let resolver = StorageKeyResolver::empty();
    assert_eq!(resolver.resolve_storage_key(&specifier), None);
  }
}
