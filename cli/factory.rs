// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::npm::NpmCacheDir;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::FeatureChecker;
use deno_error::JsErrorBox;
use deno_lib::args::get_root_cert_store;
use deno_lib::args::resolve_npm_resolution_snapshot;
use deno_lib::args::CaData;
use deno_lib::args::NpmProcessStateKind;
use deno_lib::args::NPM_PROCESS_STATE;
use deno_lib::loader::NpmModuleLoader;
use deno_lib::npm::create_npm_process_state_provider;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::LibMainWorkerOptions;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm_cache::NpmCacheSetting;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::factory::ConfigDiscoveryOption;
use deno_resolver::factory::DenoDirPathProviderOptions;
use deno_resolver::factory::NpmProcessStateOptions;
use deno_resolver::factory::ResolverFactoryOptions;
use deno_resolver::factory::SpecifiedImportMapProvider;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use node_resolver::NodeResolverOptions;
use once_cell::sync::OnceCell;
use sys_traits::EnvCurrentDir;

use crate::args::deno_json::TsConfigResolver;
use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::ConfigFlag;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::NpmInstallDepsProvider;
use crate::args::WorkspaceExternalImportMapLoader;
use crate::cache::Caches;
use crate::cache::CodeCache;
use crate::cache::DenoDir;
use crate::cache::DenoDirProvider;
use crate::cache::EmitCache;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::file_fetcher::CliFileFetcher;
use crate::file_fetcher::TextDecodedFile;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_util::FileWatcherReporter;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphCreator;
use crate::http_util::HttpClientProvider;
use crate::module_loader::CliModuleLoaderFactory;
use crate::module_loader::EszipModuleLoader;
use crate::module_loader::ModuleLoadPreparer;
use crate::node::CliCjsCodeAnalyzer;
use crate::node::CliCjsModuleExportAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::NpmResolutionInstaller;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::CliNpmTarballCache;
use crate::npm::NpmResolutionInitializer;
use crate::npm::WorkspaceNpmPatchPackages;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliDenoResolver;
use crate::resolver::CliNpmGraphResolver;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::resolver::FoundPackageJsonDepFlag;
use crate::standalone::binary::DenoCompileBinaryWriter;
use crate::sys::CliSys;
use crate::tools::coverage::CoverageCollector;
use crate::tools::lint::LintRuleProvider;
use crate::tools::run::hmr::HmrRunner;
use crate::tsc::TypeCheckingCjsTracker;
use crate::type_checker::TypeChecker;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;

struct CliRootCertStoreProvider {
  cell: OnceCell<RootCertStore>,
  maybe_root_path: Option<PathBuf>,
  maybe_ca_stores: Option<Vec<String>>,
  maybe_ca_data: Option<CaData>,
}

impl CliRootCertStoreProvider {
  pub fn new(
    maybe_root_path: Option<PathBuf>,
    maybe_ca_stores: Option<Vec<String>>,
    maybe_ca_data: Option<CaData>,
  ) -> Self {
    Self {
      cell: Default::default(),
      maybe_root_path,
      maybe_ca_stores,
      maybe_ca_data,
    }
  }
}

impl RootCertStoreProvider for CliRootCertStoreProvider {
  fn get_or_try_init(&self) -> Result<&RootCertStore, JsErrorBox> {
    self
      .cell
      .get_or_try_init(|| {
        get_root_cert_store(
          self.maybe_root_path.clone(),
          self.maybe_ca_stores.clone(),
          self.maybe_ca_data.clone(),
        )
      })
      .map_err(JsErrorBox::from_err)
  }
}

#[derive(Debug)]
struct EszipModuleLoaderProvider {
  cli_options: Arc<CliOptions>,
  deferred: once_cell::sync::OnceCell<Arc<EszipModuleLoader>>,
}

impl EszipModuleLoaderProvider {
  pub async fn get(&self) -> Result<Option<&Arc<EszipModuleLoader>>, AnyError> {
    if self.cli_options.eszip() {
      if let DenoSubcommand::Run(run_flags) = self.cli_options.sub_command() {
        if self.deferred.get().is_none() {
          let eszip_loader = EszipModuleLoader::create(
            &run_flags.script,
            self.cli_options.initial_cwd(),
          )
          .await?;
          _ = self.deferred.set(Arc::new(eszip_loader));
        }
        return Ok(Some(self.deferred.get().unwrap()));
      }
    }
    Ok(None)
  }
}

#[derive(Debug)]
struct CliSpecifiedImportMapProvider {
  cli_options: Arc<CliOptions>,
  file_fetcher: Arc<CliFileFetcher>,
  eszip_module_loader_provider: Arc<EszipModuleLoaderProvider>,
  workspace_external_import_map_loader: Arc<WorkspaceExternalImportMapLoader>,
}

#[async_trait::async_trait(?Send)]
impl SpecifiedImportMapProvider for CliSpecifiedImportMapProvider {
  async fn get(
    &self,
  ) -> Result<Option<deno_resolver::workspace::SpecifiedImportMap>, AnyError>
  {
    async fn resolve_import_map_value_from_specifier(
      specifier: &Url,
      file_fetcher: &CliFileFetcher,
    ) -> Result<serde_json::Value, AnyError> {
      if specifier.scheme() == "data" {
        let data_url_text =
          deno_media_type::data_url::RawDataUrl::parse(specifier)?.decode()?;
        Ok(serde_json::from_str(&data_url_text)?)
      } else {
        let file = TextDecodedFile::decode(
          file_fetcher.fetch_bypass_permissions(specifier).await?,
        )?;
        Ok(serde_json::from_str(&file.source)?)
      }
    }

    let maybe_import_map_specifier =
      self.cli_options.resolve_specified_import_map_specifier()?;
    match maybe_import_map_specifier {
      Some(specifier) => {
        let value = match self.eszip_module_loader_provider.get().await? {
          Some(eszip) => eszip.load_import_map_value(&specifier)?,
          None => resolve_import_map_value_from_specifier(
            &specifier,
            &self.file_fetcher,
          )
          .await
          .with_context(|| {
            format!("Unable to load '{}' import map", specifier)
          })?,
        };
        Ok(Some(deno_resolver::workspace::SpecifiedImportMap {
          base_url: specifier,
          value,
        }))
      }
      None => {
        if let Some(import_map) =
          self.workspace_external_import_map_loader.get_or_load()?
        {
          let path_url = deno_path_util::url_from_file_path(&import_map.path)?;
          Ok(Some(deno_resolver::workspace::SpecifiedImportMap {
            base_url: path_url,
            value: import_map.value.clone(),
          }))
        } else {
          Ok(None)
        }
      }
    }
  }
}

pub type CliWorkspaceFactory = deno_resolver::factory::WorkspaceFactory<CliSys>;
pub type CliDenoDirPathProvider =
  deno_resolver::factory::DenoDirPathProvider<CliSys>;

pub type CliResolverFactory = deno_resolver::factory::ResolverFactory<CliSys>;

pub struct Deferred<T>(once_cell::unsync::OnceCell<T>);

impl<T> Default for Deferred<T> {
  fn default() -> Self {
    Self(once_cell::unsync::OnceCell::default())
  }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Deferred<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Deferred").field(&self.0).finish()
  }
}

impl<T> Deferred<T> {
  #[inline(always)]
  pub fn get_or_try_init(
    &self,
    create: impl FnOnce() -> Result<T, AnyError>,
  ) -> Result<&T, AnyError> {
    self.0.get_or_try_init(create)
  }

  #[inline(always)]
  pub fn get_or_init(&self, create: impl FnOnce() -> T) -> &T {
    self.0.get_or_init(create)
  }

  pub async fn get_or_try_init_async(
    &self,
    // some futures passed here are boxed because it was discovered
    // that they were called a lot, causing other futures to get
    // really big causing stack overflows on Windows
    create: impl Future<Output = Result<T, AnyError>>,
  ) -> Result<&T, AnyError> {
    if self.0.get().is_none() {
      // todo(dsherret): it would be more ideal if this enforced a
      // single executor and then we could make some initialization
      // concurrent
      let val = create.await?;
      _ = self.0.set(val);
    }
    Ok(self.0.get().unwrap())
  }
}

#[derive(Default)]
struct CliFactoryServices {
  blob_store: Deferred<Arc<BlobStore>>,
  caches: Deferred<Arc<Caches>>,
  cjs_module_export_analyzer: Deferred<Arc<CliCjsModuleExportAnalyzer>>,
  cjs_tracker: Deferred<Arc<CliCjsTracker>>,
  cli_options: Deferred<Arc<CliOptions>>,
  code_cache: Deferred<Arc<CodeCache>>,
  deno_dir_path_provider: Deferred<Arc<CliDenoDirPathProvider>>,
  deno_dir_provider: Deferred<Arc<DenoDirProvider>>,
  emit_cache: Deferred<Arc<EmitCache>>,
  emitter: Deferred<Arc<Emitter>>,
  eszip_module_loader_provider: Deferred<Arc<EszipModuleLoaderProvider>>,
  feature_checker: Deferred<Arc<FeatureChecker>>,
  file_fetcher: Deferred<Arc<CliFileFetcher>>,
  found_pkg_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  fs: Deferred<Arc<dyn deno_fs::FileSystem>>,
  http_client_provider: Deferred<Arc<HttpClientProvider>>,
  main_graph_container: Deferred<Arc<MainModuleGraphContainer>>,
  maybe_file_watcher_reporter: Deferred<Option<FileWatcherReporter>>,
  maybe_inspector_server: Deferred<Option<Arc<InspectorServer>>>,
  module_graph_builder: Deferred<Arc<ModuleGraphBuilder>>,
  module_graph_creator: Deferred<Arc<ModuleGraphCreator>>,
  module_info_cache: Deferred<Arc<ModuleInfoCache>>,
  module_load_preparer: Deferred<Arc<ModuleLoadPreparer>>,
  node_code_translator: Deferred<Arc<CliNodeCodeTranslator>>,
  npm_cache: Deferred<Arc<CliNpmCache>>,
  npm_cache_http_client: Deferred<Arc<CliNpmCacheHttpClient>>,
  npm_graph_resolver: Deferred<Arc<CliNpmGraphResolver>>,
  npm_installer: Deferred<Arc<NpmInstaller>>,
  npm_registry_info_provider: Deferred<Arc<CliNpmRegistryInfoProvider>>,
  npm_resolution_initializer: Deferred<Arc<NpmResolutionInitializer>>,
  npm_resolution_installer: Deferred<Arc<NpmResolutionInstaller>>,
  npm_tarball_cache: Deferred<Arc<CliNpmTarballCache>>,
  parsed_source_cache: Deferred<Arc<ParsedSourceCache>>,
  permission_desc_parser:
    Deferred<Arc<RuntimePermissionDescriptorParser<CliSys>>>,
  resolver: Deferred<Arc<CliResolver>>,
  resolver_factory: Deferred<Arc<CliResolverFactory>>,
  root_cert_store_provider: Deferred<Arc<dyn RootCertStoreProvider>>,
  root_permissions_container: Deferred<PermissionsContainer>,
  text_only_progress_bar: Deferred<ProgressBar>,
  tsconfig_resolver: Deferred<Arc<TsConfigResolver>>,
  type_checker: Deferred<Arc<TypeChecker>>,
  workspace_factory: Deferred<Arc<CliWorkspaceFactory>>,
  workspace_external_import_map_loader:
    Deferred<Arc<WorkspaceExternalImportMapLoader>>,
  workspace_npm_patch_packages: Deferred<Arc<WorkspaceNpmPatchPackages>>,
  lockfile: Deferred<Option<Arc<CliLockfile>>>,
}

#[derive(Debug, Default)]
struct CliFactoryOverrides {
  initial_cwd: Option<PathBuf>,
  workspace_directory: Option<Arc<WorkspaceDirectory>>,
}

pub struct CliFactory {
  watcher_communicator: Option<Arc<WatcherCommunicator>>,
  flags: Arc<Flags>,
  services: CliFactoryServices,
  overrides: CliFactoryOverrides,
}

impl CliFactory {
  pub fn from_flags(flags: Arc<Flags>) -> Self {
    Self {
      flags,
      watcher_communicator: None,
      services: Default::default(),
      overrides: Default::default(),
    }
  }

  pub fn from_flags_for_watcher(
    flags: Arc<Flags>,
    watcher_communicator: Arc<WatcherCommunicator>,
  ) -> Self {
    CliFactory {
      watcher_communicator: Some(watcher_communicator),
      flags,
      services: Default::default(),
      overrides: Default::default(),
    }
  }

  pub fn set_initial_cwd(&mut self, initial_cwd: PathBuf) {
    self.overrides.initial_cwd = Some(initial_cwd);
  }

  pub fn set_workspace_dir(&mut self, dir: Arc<WorkspaceDirectory>) {
    self.overrides.workspace_directory = Some(dir);
  }

  pub async fn maybe_lockfile(
    &self,
  ) -> Result<Option<&Arc<CliLockfile>>, AnyError> {
    self
      .services
      .lockfile
      .get_or_try_init_async(async move {
        let workspace_factory = self.workspace_factory()?;
        let workspace_directory = workspace_factory.workspace_directory()?;
        let maybe_external_import_map =
          self.workspace_external_import_map_loader()?.get_or_load()?;
        let adapter = self.lockfile_npm_package_info_provider()?;

        let maybe_lock_file = CliLockfile::discover(
          &self.sys(),
          &self.flags,
          &workspace_directory.workspace,
          maybe_external_import_map.as_ref().map(|v| &v.value),
          &adapter,
        )
        .await?
        .map(Arc::new);

        Ok(maybe_lock_file)
      })
      .await
      .map(|c| c.as_ref())
  }

  pub fn cli_options(&self) -> Result<&Arc<CliOptions>, AnyError> {
    self.services.cli_options.get_or_try_init(|| {
      let workspace_factory = self.workspace_factory()?;
      let workspace_directory = workspace_factory.workspace_directory()?;
      CliOptions::from_flags(
        self.flags.clone(),
        workspace_factory.initial_cwd().clone(),
        workspace_directory.clone(),
      )
      .map(Arc::new)
    })
  }

  pub fn deno_dir_path_provider(&self) -> &Arc<CliDenoDirPathProvider> {
    self.services.deno_dir_path_provider.get_or_init(|| {
      Arc::new(CliDenoDirPathProvider::new(
        self.sys(),
        DenoDirPathProviderOptions {
          maybe_custom_root: self.flags.internal.cache_path.clone(),
        },
      ))
    })
  }

  pub fn deno_dir_provider(&self) -> &Arc<DenoDirProvider> {
    self.services.deno_dir_provider.get_or_init(|| {
      Arc::new(DenoDirProvider::new(
        self.sys(),
        self.deno_dir_path_provider().clone(),
      ))
    })
  }

  pub fn deno_dir(&self) -> Result<&DenoDir, AnyError> {
    Ok(self.deno_dir_provider().get_or_create()?)
  }

  pub fn caches(&self) -> Result<&Arc<Caches>, AnyError> {
    self.services.caches.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let caches = Arc::new(Caches::new(self.deno_dir_provider().clone()));
      // Warm up the caches we know we'll likely need based on the CLI mode
      match cli_options.sub_command() {
        DenoSubcommand::Run(_)
        | DenoSubcommand::Serve(_)
        | DenoSubcommand::Bench(_)
        | DenoSubcommand::Test(_)
        | DenoSubcommand::Check(_) => {
          _ = caches.dep_analysis_db();
          _ = caches.node_analysis_db();
          if cli_options.type_check_mode().is_true() {
            _ = caches.fast_check_db();
            _ = caches.type_checking_cache_db();
          }
          if cli_options.code_cache_enabled() {
            _ = caches.code_cache_db();
          }
        }
        _ => {}
      }
      Ok(caches)
    })
  }

  pub fn blob_store(&self) -> &Arc<BlobStore> {
    self.services.blob_store.get_or_init(Default::default)
  }

  pub fn root_cert_store_provider(&self) -> &Arc<dyn RootCertStoreProvider> {
    self.services.root_cert_store_provider.get_or_init(|| {
      Arc::new(CliRootCertStoreProvider::new(
        None,
        self.flags.ca_stores.clone(),
        self.flags.ca_data.clone(),
      ))
    })
  }

  pub fn text_only_progress_bar(&self) -> &ProgressBar {
    self
      .services
      .text_only_progress_bar
      .get_or_init(|| ProgressBar::new(ProgressBarStyle::TextOnly))
  }

  pub fn global_http_cache(&self) -> Result<&Arc<GlobalHttpCache>, AnyError> {
    Ok(self.workspace_factory()?.global_http_cache()?)
  }

  pub fn http_cache(&self) -> Result<&Arc<dyn HttpCache>, AnyError> {
    Ok(self.workspace_factory()?.http_cache()?)
  }

  pub fn http_client_provider(&self) -> &Arc<HttpClientProvider> {
    self.services.http_client_provider.get_or_init(|| {
      Arc::new(HttpClientProvider::new(
        Some(self.root_cert_store_provider().clone()),
        self.flags.unsafely_ignore_certificate_errors.clone(),
      ))
    })
  }

  fn eszip_module_loader_provider(
    &self,
  ) -> Result<&Arc<EszipModuleLoaderProvider>, AnyError> {
    self
      .services
      .eszip_module_loader_provider
      .get_or_try_init(|| {
        Ok(Arc::new(EszipModuleLoaderProvider {
          cli_options: self.cli_options()?.clone(),
          deferred: Default::default(),
        }))
      })
  }

  pub fn file_fetcher(&self) -> Result<&Arc<CliFileFetcher>, AnyError> {
    self.services.file_fetcher.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(CliFileFetcher::new(
        self.http_cache()?.clone(),
        self.http_client_provider().clone(),
        self.sys(),
        self.blob_store().clone(),
        Some(self.text_only_progress_bar().clone()),
        !cli_options.no_remote(),
        cli_options.cache_setting(),
        log::Level::Info,
      )))
    })
  }

  pub fn fs(&self) -> &Arc<dyn deno_fs::FileSystem> {
    self.services.fs.get_or_init(|| Arc::new(RealFs))
  }

  pub fn sys(&self) -> CliSys {
    CliSys::default() // very cheap to make
  }

  pub fn in_npm_pkg_checker(
    &self,
  ) -> Result<&DenoInNpmPackageChecker, AnyError> {
    self.resolver_factory()?.in_npm_package_checker()
  }

  pub fn npm_cache(&self) -> Result<&Arc<CliNpmCache>, AnyError> {
    self.services.npm_cache.get_or_try_init(|| {
      let cache_setting = self.cli_options()?.cache_setting();
      Ok(Arc::new(CliNpmCache::new(
        self.npm_cache_dir()?.clone(),
        self.sys(),
        NpmCacheSetting::from_cache_setting(&cache_setting),
        self.npmrc()?.clone(),
      )))
    })
  }

  pub fn npm_cache_dir(&self) -> Result<&Arc<NpmCacheDir>, AnyError> {
    Ok(self.workspace_factory()?.npm_cache_dir()?)
  }

  pub fn npm_cache_http_client(&self) -> &Arc<CliNpmCacheHttpClient> {
    self.services.npm_cache_http_client.get_or_init(|| {
      Arc::new(CliNpmCacheHttpClient::new(
        self.http_client_provider().clone(),
        self.text_only_progress_bar().clone(),
      ))
    })
  }

  pub fn npmrc(&self) -> Result<&Arc<ResolvedNpmRc>, AnyError> {
    Ok(self.workspace_factory()?.npmrc()?)
  }

  pub fn npm_resolution(&self) -> Result<&Arc<NpmResolutionCell>, AnyError> {
    Ok(self.resolver_factory()?.npm_resolution())
  }

  pub async fn npm_graph_resolver(
    &self,
  ) -> Result<&Arc<CliNpmGraphResolver>, AnyError> {
    self
      .services
      .npm_graph_resolver
      .get_or_try_init_async(
        async move {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(CliNpmGraphResolver::new(
            self.npm_installer_if_managed().await?.cloned(),
            self.services.found_pkg_json_dep_flag.clone(),
            cli_options.unstable_bare_node_builtins(),
            cli_options.default_npm_caching_strategy(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn npm_installer_if_managed(
    &self,
  ) -> Result<Option<&Arc<NpmInstaller>>, AnyError> {
    if self.resolver_factory()?.use_byonm()? || self.cli_options()?.no_npm() {
      Ok(None)
    } else {
      Ok(Some(self.npm_installer().await?))
    }
  }

  pub async fn npm_installer(&self) -> Result<&Arc<NpmInstaller>, AnyError> {
    self
      .services
      .npm_installer
      .get_or_try_init_async(async move {
        let cli_options = self.cli_options()?;
        let workspace_factory = self.workspace_factory()?;
        Ok(Arc::new(NpmInstaller::new(
          self.npm_cache()?.clone(),
          Arc::new(NpmInstallDepsProvider::from_workspace(
            cli_options.workspace(),
          )),
          Arc::new(self.npm_registry_info_provider()?.as_npm_registry_api()),
          self.npm_resolution()?.clone(),
          self.npm_resolution_initializer().await?.clone(),
          self.npm_resolution_installer().await?.clone(),
          self.text_only_progress_bar(),
          self.sys(),
          self.npm_tarball_cache()?.clone(),
          self.maybe_lockfile().await?.cloned(),
          workspace_factory
            .node_modules_dir_path()?
            .map(|p| p.to_path_buf()),
          cli_options.lifecycle_scripts_config(),
          cli_options.npm_system_info(),
          self.workspace_npm_patch_packages()?.clone(),
        )))
      })
      .await
  }

  pub fn npm_registry_info_provider(
    &self,
  ) -> Result<&Arc<CliNpmRegistryInfoProvider>, AnyError> {
    self
      .services
      .npm_registry_info_provider
      .get_or_try_init(|| {
        Ok(Arc::new(CliNpmRegistryInfoProvider::new(
          self.npm_cache()?.clone(),
          self.npm_cache_http_client().clone(),
          self.npmrc()?.clone(),
        )))
      })
  }

  pub fn lockfile_npm_package_info_provider(
    &self,
  ) -> Result<crate::npm::NpmPackageInfoApiAdapter, AnyError> {
    Ok(crate::npm::NpmPackageInfoApiAdapter::new(
      Arc::new(self.npm_registry_info_provider()?.as_npm_registry_api()),
      self.workspace_npm_patch_packages()?.clone(),
    ))
  }

  pub async fn npm_resolution_initializer(
    &self,
  ) -> Result<&Arc<NpmResolutionInitializer>, AnyError> {
    self
      .services
      .npm_resolution_initializer
      .get_or_try_init_async(async move {
        Ok(Arc::new(NpmResolutionInitializer::new(
          self.npm_resolution()?.clone(),
          self.workspace_npm_patch_packages()?.clone(),
          match resolve_npm_resolution_snapshot()? {
            Some(snapshot) => {
              CliNpmResolverManagedSnapshotOption::Specified(Some(snapshot))
            }
            None => match self.maybe_lockfile().await? {
              Some(lockfile) => {
                CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(
                  lockfile.clone(),
                )
              }
              None => CliNpmResolverManagedSnapshotOption::Specified(None),
            },
          },
        )))
      })
      .await
  }

  pub async fn npm_resolution_installer(
    &self,
  ) -> Result<&Arc<NpmResolutionInstaller>, AnyError> {
    self
      .services
      .npm_resolution_installer
      .get_or_try_init_async(async move {
        Ok(Arc::new(NpmResolutionInstaller::new(
          self.npm_registry_info_provider()?.clone(),
          self.npm_resolution()?.clone(),
          self.maybe_lockfile().await?.cloned(),
          self.workspace_npm_patch_packages()?.clone(),
        )))
      })
      .await
  }

  pub fn workspace_npm_patch_packages(
    &self,
  ) -> Result<&Arc<WorkspaceNpmPatchPackages>, AnyError> {
    self
      .services
      .workspace_npm_patch_packages
      .get_or_try_init(|| {
        let cli_options = self.cli_options()?;
        let npm_packages = Arc::new(WorkspaceNpmPatchPackages::from_workspace(
          cli_options.workspace().as_ref(),
        ));
        if !npm_packages.0.is_empty() && !matches!(self.workspace_factory()?.node_modules_dir_mode()?, NodeModulesDirMode::Auto | NodeModulesDirMode::Manual) {
          bail!("Patching npm packages requires using a node_modules directory. Ensure you have a package.json or set the \"nodeModulesDir\" option to \"auto\" or \"manual\" in your workspace root deno.json.")
        } else {
          Ok(npm_packages)
        }
      })
  }

  pub async fn npm_resolver(&self) -> Result<&CliNpmResolver, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.npm_resolver()
  }

  pub fn npm_tarball_cache(
    &self,
  ) -> Result<&Arc<CliNpmTarballCache>, AnyError> {
    self.services.npm_tarball_cache.get_or_try_init(|| {
      Ok(Arc::new(CliNpmTarballCache::new(
        self.npm_cache()?.clone(),
        self.npm_cache_http_client().clone(),
        self.sys(),
        self.npmrc()?.clone(),
      )))
    })
  }

  pub fn workspace(&self) -> Result<&Arc<Workspace>, AnyError> {
    Ok(&self.workspace_directory()?.workspace)
  }

  pub fn workspace_directory(
    &self,
  ) -> Result<&Arc<WorkspaceDirectory>, AnyError> {
    Ok(self.workspace_factory()?.workspace_directory()?)
  }

  fn workspace_factory(&self) -> Result<&Arc<CliWorkspaceFactory>, AnyError> {
    self.services.workspace_factory.get_or_try_init(|| {
      let initial_cwd = match self.overrides.initial_cwd.clone() {
        Some(v) => v,
        None => self
          .sys()
          .env_current_dir()
          .with_context(|| "Failed getting cwd.")?,
      };
      let options = new_workspace_factory_options(
        &initial_cwd,
        &self.flags,
        self.deno_dir_path_provider().clone(),
      );
      let mut factory =
        CliWorkspaceFactory::new(self.sys(), initial_cwd, options);
      if let Some(workspace_dir) = &self.overrides.workspace_directory {
        factory.set_workspace_directory(workspace_dir.clone());
      }
      Ok(Arc::new(factory))
    })
  }

  pub async fn workspace_resolver(
    &self,
  ) -> Result<&Arc<WorkspaceResolver<CliSys>>, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.workspace_resolver().await
  }

  pub async fn deno_resolver(&self) -> Result<&Arc<CliDenoResolver>, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.deno_resolver().await
  }

  pub async fn resolver(&self) -> Result<&Arc<CliResolver>, AnyError> {
    self
      .services
      .resolver
      .get_or_try_init_async(
        async {
          Ok(Arc::new(CliResolver::new(
            self.deno_resolver().await?.clone(),
            self.services.found_pkg_json_dep_flag.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub fn maybe_file_watcher_reporter(&self) -> &Option<FileWatcherReporter> {
    let maybe_file_watcher_reporter = self
      .watcher_communicator
      .as_ref()
      .map(|i| FileWatcherReporter::new(i.clone()));
    self
      .services
      .maybe_file_watcher_reporter
      .get_or_init(|| maybe_file_watcher_reporter)
  }

  pub fn emit_cache(&self) -> Result<&Arc<EmitCache>, AnyError> {
    self.services.emit_cache.get_or_try_init(|| {
      Ok(Arc::new(EmitCache::new(self.deno_dir()?.gen_cache.clone())))
    })
  }

  pub fn module_info_cache(&self) -> Result<&Arc<ModuleInfoCache>, AnyError> {
    self.services.module_info_cache.get_or_try_init(|| {
      Ok(Arc::new(ModuleInfoCache::new(
        self.caches()?.dep_analysis_db(),
        self.parsed_source_cache().clone(),
      )))
    })
  }

  pub fn code_cache(&self) -> Result<&Arc<CodeCache>, AnyError> {
    self.services.code_cache.get_or_try_init(|| {
      Ok(Arc::new(CodeCache::new(self.caches()?.code_cache_db())))
    })
  }

  pub fn parsed_source_cache(&self) -> &Arc<ParsedSourceCache> {
    self
      .services
      .parsed_source_cache
      .get_or_init(Default::default)
  }

  pub fn emitter(&self) -> Result<&Arc<Emitter>, AnyError> {
    self.services.emitter.get_or_try_init(|| {
      Ok(Arc::new(Emitter::new(
        self.cjs_tracker()?.clone(),
        self.emit_cache()?.clone(),
        self.parsed_source_cache().clone(),
        self.tsconfig_resolver()?.clone(),
      )))
    })
  }

  pub async fn lint_rule_provider(&self) -> Result<LintRuleProvider, AnyError> {
    Ok(LintRuleProvider::new(Some(
      self.workspace_resolver().await?.clone(),
    )))
  }

  pub async fn node_resolver(&self) -> Result<&Arc<CliNodeResolver>, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.node_resolver()
  }

  async fn initialize_npm_resolution_if_managed(&self) -> Result<(), AnyError> {
    let npm_resolver = self.resolver_factory()?.npm_resolver()?;
    if npm_resolver.is_managed() {
      self
        .npm_resolution_initializer()
        .await?
        .ensure_initialized()
        .await?;
    }
    Ok(())
  }

  pub async fn cjs_module_export_analyzer(
    &self,
  ) -> Result<&Arc<CliCjsModuleExportAnalyzer>, AnyError> {
    self
      .services
      .cjs_module_export_analyzer
      .get_or_try_init_async(async {
        let node_resolver = self.node_resolver().await?.clone();
        let cjs_code_analyzer = self.create_cjs_code_analyzer()?;

        Ok(Arc::new(CliCjsModuleExportAnalyzer::new(
          cjs_code_analyzer,
          self.in_npm_pkg_checker()?.clone(),
          node_resolver,
          self.npm_resolver().await?.clone(),
          self.pkg_json_resolver()?.clone(),
          self.sys(),
        )))
      })
      .await
  }

  pub async fn node_code_translator(
    &self,
  ) -> Result<&Arc<CliNodeCodeTranslator>, AnyError> {
    self
      .services
      .node_code_translator
      .get_or_try_init_async(
        async {
          let module_export_analyzer =
            self.cjs_module_export_analyzer().await?;
          Ok(Arc::new(NodeCodeTranslator::new(
            module_export_analyzer.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  fn create_cjs_code_analyzer(&self) -> Result<CliCjsCodeAnalyzer, AnyError> {
    let caches = self.caches()?;
    let node_analysis_cache = NodeAnalysisCache::new(caches.node_analysis_db());
    Ok(CliCjsCodeAnalyzer::new(
      node_analysis_cache,
      self.cjs_tracker()?.clone(),
      self.fs().clone(),
      Some(self.parsed_source_cache().clone()),
    ))
  }

  pub fn npm_req_resolver(&self) -> Result<&Arc<CliNpmReqResolver>, AnyError> {
    self.resolver_factory()?.npm_req_resolver()
  }

  pub fn pkg_json_resolver(
    &self,
  ) -> Result<&Arc<CliPackageJsonResolver>, AnyError> {
    Ok(self.resolver_factory()?.pkg_json_resolver())
  }

  pub fn tsconfig_resolver(&self) -> Result<&Arc<TsConfigResolver>, AnyError> {
    self.services.tsconfig_resolver.get_or_try_init(|| {
      let workspace = self.workspace()?;
      Ok(Arc::new(TsConfigResolver::from_workspace(workspace)))
    })
  }

  pub async fn type_checker(&self) -> Result<&Arc<TypeChecker>, AnyError> {
    self
      .services
      .type_checker
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(TypeChecker::new(
            self.caches()?.clone(),
            Arc::new(TypeCheckingCjsTracker::new(
              self.cjs_tracker()?.clone(),
              self.module_info_cache()?.clone(),
            )),
            cli_options.clone(),
            self.module_graph_builder().await?.clone(),
            self.node_resolver().await?.clone(),
            self.npm_installer_if_managed().await?.cloned(),
            self.npm_resolver().await?.clone(),
            self.sys(),
            self.tsconfig_resolver()?.clone(),
            if cli_options.code_cache_enabled() {
              Some(self.code_cache()?.clone())
            } else {
              None
            },
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn module_graph_builder(
    &self,
  ) -> Result<&Arc<ModuleGraphBuilder>, AnyError> {
    self
      .services
      .module_graph_builder
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(ModuleGraphBuilder::new(
            self.caches()?.clone(),
            self.cjs_tracker()?.clone(),
            cli_options.clone(),
            self.file_fetcher()?.clone(),
            self.global_http_cache()?.clone(),
            self.in_npm_pkg_checker()?.clone(),
            self.maybe_lockfile().await?.cloned(),
            self.maybe_file_watcher_reporter().clone(),
            self.module_info_cache()?.clone(),
            self.npm_graph_resolver().await?.clone(),
            self.npm_installer_if_managed().await?.cloned(),
            self.npm_resolver().await?.clone(),
            self.parsed_source_cache().clone(),
            self.resolver().await?.clone(),
            self.root_permissions_container()?.clone(),
            self.sys(),
            self.tsconfig_resolver()?.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn module_graph_creator(
    &self,
  ) -> Result<&Arc<ModuleGraphCreator>, AnyError> {
    self
      .services
      .module_graph_creator
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(ModuleGraphCreator::new(
            cli_options.clone(),
            self.npm_installer_if_managed().await?.cloned(),
            self.module_graph_builder().await?.clone(),
            self.type_checker().await?.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn main_module_graph_container(
    &self,
  ) -> Result<&Arc<MainModuleGraphContainer>, AnyError> {
    self
      .services
      .main_graph_container
      .get_or_try_init_async(
        async {
          Ok(Arc::new(MainModuleGraphContainer::new(
            self.cli_options()?.clone(),
            self.module_load_preparer().await?.clone(),
            self.root_permissions_container()?.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub fn maybe_inspector_server(
    &self,
  ) -> Result<&Option<Arc<InspectorServer>>, AnyError> {
    self.services.maybe_inspector_server.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      match cli_options.resolve_inspector_server() {
        Ok(server) => Ok(server.map(Arc::new)),
        Err(err) => Err(err),
      }
    })
  }

  pub async fn module_load_preparer(
    &self,
  ) -> Result<&Arc<ModuleLoadPreparer>, AnyError> {
    self
      .services
      .module_load_preparer
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(ModuleLoadPreparer::new(
            cli_options.clone(),
            self.maybe_lockfile().await?.cloned(),
            self.module_graph_builder().await?.clone(),
            self.text_only_progress_bar().clone(),
            self.type_checker().await?.clone(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub fn cjs_tracker(&self) -> Result<&Arc<CliCjsTracker>, AnyError> {
    self.services.cjs_tracker.get_or_try_init(|| {
      let options = self.cli_options()?;
      Ok(Arc::new(CliCjsTracker::new(
        self.in_npm_pkg_checker()?.clone(),
        self.pkg_json_resolver()?.clone(),
        if options.is_node_main() || options.unstable_detect_cjs() {
          IsCjsResolutionMode::ImplicitTypeCommonJs
        } else if options.detect_cjs() {
          IsCjsResolutionMode::ExplicitTypeCommonJs
        } else {
          IsCjsResolutionMode::Disabled
        },
      )))
    })
  }

  pub fn permission_desc_parser(
    &self,
  ) -> Result<&Arc<RuntimePermissionDescriptorParser<CliSys>>, AnyError> {
    self.services.permission_desc_parser.get_or_try_init(|| {
      Ok(Arc::new(RuntimePermissionDescriptorParser::new(self.sys())))
    })
  }

  pub fn feature_checker(&self) -> Result<&Arc<FeatureChecker>, AnyError> {
    self.services.feature_checker.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let mut checker = FeatureChecker::default();
      checker.set_exit_cb(Box::new(crate::unstable_exit_cb));
      let unstable_features = cli_options.unstable_features();
      for granular_flag in crate::UNSTABLE_GRANULAR_FLAGS {
        if unstable_features.contains(&granular_flag.name.to_string()) {
          checker.enable_feature(granular_flag.name);
        }
      }

      Ok(Arc::new(checker))
    })
  }

  pub async fn create_compile_binary_writer(
    &self,
  ) -> Result<DenoCompileBinaryWriter, AnyError> {
    let cli_options = self.cli_options()?;
    Ok(DenoCompileBinaryWriter::new(
      self.cjs_module_export_analyzer().await?,
      self.cjs_tracker()?,
      self.cli_options()?,
      self.deno_dir()?,
      self.emitter()?,
      self.http_client_provider(),
      self.npm_resolver().await?,
      self.workspace_resolver().await?.as_ref(),
      cli_options.npm_system_info(),
    ))
  }

  pub fn root_permissions_container(
    &self,
  ) -> Result<&PermissionsContainer, AnyError> {
    self
      .services
      .root_permissions_container
      .get_or_try_init(|| {
        let desc_parser = self.permission_desc_parser()?.clone();
        let permissions = Permissions::from_options(
          desc_parser.as_ref(),
          &self.cli_options()?.permissions_options(),
        )?;
        Ok(PermissionsContainer::new(desc_parser, permissions))
      })
  }

  fn workspace_external_import_map_loader(
    &self,
  ) -> Result<&Arc<WorkspaceExternalImportMapLoader>, AnyError> {
    self
      .services
      .workspace_external_import_map_loader
      .get_or_try_init(|| {
        Ok(Arc::new(WorkspaceExternalImportMapLoader::new(
          self.sys(),
          self.workspace_directory()?.workspace.clone(),
        )))
      })
  }

  pub async fn create_cli_main_worker_factory(
    &self,
  ) -> Result<CliMainWorkerFactory, AnyError> {
    let cli_options = self.cli_options()?;
    let fs = self.fs();
    let node_resolver = self.node_resolver().await?;
    let npm_resolver = self.npm_resolver().await?;
    let cli_npm_resolver = self.npm_resolver().await?.clone();
    let in_npm_pkg_checker = self.in_npm_pkg_checker()?;
    let maybe_file_watcher_communicator = if cli_options.has_hmr() {
      Some(self.watcher_communicator.clone().unwrap())
    } else {
      None
    };
    let node_code_translator = self.node_code_translator().await?;
    let cjs_tracker = self.cjs_tracker()?.clone();
    let pkg_json_resolver = self.pkg_json_resolver()?;
    let npm_req_resolver = self.npm_req_resolver()?;
    let workspace_factory = self.workspace_factory()?;
    let npm_registry_permission_checker = {
      let mode = if self.resolver_factory()?.use_byonm()? {
        NpmRegistryReadPermissionCheckerMode::Byonm
      } else if let Some(node_modules_dir) =
        workspace_factory.node_modules_dir_path()?
      {
        NpmRegistryReadPermissionCheckerMode::Local(
          node_modules_dir.to_path_buf(),
        )
      } else {
        NpmRegistryReadPermissionCheckerMode::Global(
          self.npm_cache_dir()?.root_dir().to_path_buf(),
        )
      };
      Arc::new(NpmRegistryReadPermissionChecker::new(self.sys(), mode))
    };

    let maybe_eszip_loader =
      self.eszip_module_loader_provider()?.get().await?.cloned();
    let module_loader_factory = CliModuleLoaderFactory::new(
      cli_options,
      cjs_tracker,
      if cli_options.code_cache_enabled() {
        Some(self.code_cache()?.clone())
      } else {
        None
      },
      self.emitter()?.clone(),
      in_npm_pkg_checker.clone(),
      self.main_module_graph_container().await?.clone(),
      self.module_load_preparer().await?.clone(),
      node_code_translator.clone(),
      node_resolver.clone(),
      NpmModuleLoader::new(
        self.cjs_tracker()?.clone(),
        node_code_translator.clone(),
        self.sys(),
      ),
      npm_registry_permission_checker,
      npm_req_resolver.clone(),
      cli_npm_resolver.clone(),
      self.parsed_source_cache().clone(),
      self.resolver().await?.clone(),
      self.sys(),
      maybe_eszip_loader,
    );

    let lib_main_worker_factory = LibMainWorkerFactory::new(
      self.blob_store().clone(),
      if cli_options.code_cache_enabled() {
        Some(self.code_cache()?.clone())
      } else {
        None
      },
      None, // DenoRtNativeAddonLoader
      self.feature_checker()?.clone(),
      fs.clone(),
      self.maybe_inspector_server()?.clone(),
      Box::new(module_loader_factory),
      node_resolver.clone(),
      create_npm_process_state_provider(npm_resolver),
      pkg_json_resolver.clone(),
      self.root_cert_store_provider().clone(),
      cli_options.resolve_storage_key_resolver(),
      self.sys(),
      self.create_lib_main_worker_options()?,
    );

    Ok(CliMainWorkerFactory::new(
      lib_main_worker_factory,
      maybe_file_watcher_communicator,
      self.maybe_lockfile().await?.cloned(),
      self.npm_installer_if_managed().await?.cloned(),
      npm_resolver.clone(),
      self.sys(),
      self.create_cli_main_worker_options()?,
      self.root_permissions_container()?.clone(),
    ))
  }

  pub fn create_lib_main_worker_options(
    &self,
  ) -> Result<LibMainWorkerOptions, AnyError> {
    let cli_options = self.cli_options()?;
    let workspace_factory = self.workspace_factory()?;
    Ok(LibMainWorkerOptions {
      argv: cli_options.argv().clone(),
      // This optimization is only available for "run" subcommand
      // because we need to register new ops for testing and jupyter
      // integration.
      skip_op_registration: cli_options.sub_command().is_run(),
      log_level: cli_options.log_level().unwrap_or(log::Level::Info).into(),
      enable_op_summary_metrics: cli_options.enable_op_summary_metrics(),
      enable_testing_features: cli_options.enable_testing_features(),
      has_node_modules_dir: workspace_factory
        .node_modules_dir_path()?
        .is_some(),
      inspect_brk: cli_options.inspect_brk().is_some(),
      inspect_wait: cli_options.inspect_wait().is_some(),
      strace_ops: cli_options.strace_ops().clone(),
      is_standalone: false,
      is_inspecting: cli_options.is_inspecting(),
      location: cli_options.location_flag().clone(),
      // if the user ran a binary command, we'll need to set process.argv[0]
      // to be the name of the binary command instead of deno
      argv0: cli_options
        .take_binary_npm_command_name()
        .or(std::env::args().next()),
      node_debug: std::env::var("NODE_DEBUG").ok(),
      origin_data_folder_path: Some(self.deno_dir()?.origin_data_folder_path()),
      seed: cli_options.seed(),
      unsafely_ignore_certificate_errors: cli_options
        .unsafely_ignore_certificate_errors()
        .clone(),
      node_ipc: cli_options.node_ipc_fd(),
      serve_port: cli_options.serve_port(),
      serve_host: cli_options.serve_host(),
      otel_config: cli_options.otel_config(),
      no_legacy_abort: cli_options.no_legacy_abort(),
      startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    })
  }

  fn create_cli_main_worker_options(
    &self,
  ) -> Result<CliMainWorkerOptions, AnyError> {
    let cli_options = self.cli_options()?;
    let create_hmr_runner = if cli_options.has_hmr() {
      let watcher_communicator = self.watcher_communicator.clone().unwrap();
      let emitter = self.emitter()?.clone();
      let fn_: crate::worker::CreateHmrRunnerCb = Box::new(move |session| {
        Box::new(HmrRunner::new(
          emitter.clone(),
          session,
          watcher_communicator.clone(),
        ))
      });
      Some(fn_)
    } else {
      None
    };
    let create_coverage_collector =
      if let Some(coverage_dir) = cli_options.coverage_dir() {
        let coverage_dir = PathBuf::from(coverage_dir);
        let fn_: crate::worker::CreateCoverageCollectorCb =
          Box::new(move |session| {
            Box::new(CoverageCollector::new(coverage_dir.clone(), session))
          });
        Some(fn_)
      } else {
        None
      };

    Ok(CliMainWorkerOptions {
      needs_test_modules: cli_options.sub_command().needs_test(),
      create_hmr_runner,
      create_coverage_collector,
      default_npm_caching_strategy: cli_options.default_npm_caching_strategy(),
    })
  }

  pub fn resolver_factory(&self) -> Result<&Arc<CliResolverFactory>, AnyError> {
    self.services.resolver_factory.get_or_try_init(|| {
      Ok(Arc::new(CliResolverFactory::new(
        self.workspace_factory()?.clone(),
        ResolverFactoryOptions {
          node_resolver_options: NodeResolverOptions {
            conditions_from_resolution_mode: Default::default(),
            typescript_version: Some(
              deno_semver::Version::parse_standard(
                deno_lib::version::DENO_VERSION_INFO.typescript,
              )
              .unwrap(),
            ),
          },
          node_resolution_cache: Some(Arc::new(NodeResolutionThreadLocalCache)),
          npm_system_info: self.flags.subcommand.npm_system_info(),
          specified_import_map: Some(Box::new(CliSpecifiedImportMapProvider {
            cli_options: self.cli_options()?.clone(),
            eszip_module_loader_provider: self
              .eszip_module_loader_provider()?
              .clone(),
            file_fetcher: self.file_fetcher()?.clone(),
            workspace_external_import_map_loader: self
              .workspace_external_import_map_loader()?
              .clone(),
          })),
          unstable_sloppy_imports: self.flags.unstable_config.sloppy_imports,
          package_json_cache: Some(Arc::new(
            node_resolver::PackageJsonThreadLocalCache,
          )),
          package_json_dep_resolution: match &self.flags.subcommand {
            DenoSubcommand::Publish(_) => {
              // the node_modules directory is not published to jsr, so resolve
              // dependencies via the package.json rather than using node resolution
              Some(deno_resolver::workspace::PackageJsonDepResolution::Enabled)
            }
            _ => None,
          },
        },
      )))
    })
  }
}

fn new_workspace_factory_options(
  initial_cwd: &Path,
  flags: &Flags,
  deno_dir_path_provider: Arc<CliDenoDirPathProvider>,
) -> deno_resolver::factory::WorkspaceFactoryOptions<CliSys> {
  deno_resolver::factory::WorkspaceFactoryOptions {
    additional_config_file_names: if matches!(
      flags.subcommand,
      DenoSubcommand::Publish(..)
    ) {
      &["jsr.json", "jsr.jsonc"]
    } else {
      &[]
    },
    config_discovery: match &flags.config_flag {
      ConfigFlag::Discover => {
        if let Some(start_paths) = flags.config_path_args(initial_cwd) {
          ConfigDiscoveryOption::Discover { start_paths }
        } else {
          ConfigDiscoveryOption::Disabled
        }
      }
      ConfigFlag::Path(path) => {
        ConfigDiscoveryOption::Path(PathBuf::from(path))
      }
      ConfigFlag::Disabled => ConfigDiscoveryOption::Disabled,
    },
    deno_dir_path_provider: Some(deno_dir_path_provider),
    // For `deno install/add/remove/init` we want to force the managed
    // resolver so it can set up the `node_modules/` directory.
    is_package_manager_subcommand: matches!(
      flags.subcommand,
      DenoSubcommand::Install(_)
        | DenoSubcommand::Add(_)
        | DenoSubcommand::Remove(_)
        | DenoSubcommand::Init(_)
        | DenoSubcommand::Outdated(_)
    ),
    no_npm: flags.no_npm,
    node_modules_dir: flags.node_modules_dir,

    npm_process_state: NPM_PROCESS_STATE.as_ref().map(|s| {
      NpmProcessStateOptions {
        node_modules_dir: s
          .local_node_modules_path
          .as_ref()
          .map(|s| Cow::Borrowed(s.as_str())),
        is_byonm: matches!(s.kind, NpmProcessStateKind::Byonm),
      }
    }),
    vendor: flags.vendor,
  }
}
