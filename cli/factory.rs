// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_bundle_runtime::BundlePlatform;
use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_graph::packages::JsrVersionResolver;
use deno_lib::args::CaData;
use deno_lib::args::get_root_cert_store;
use deno_lib::args::npm_process_state;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::npm::create_npm_process_state_provider;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::LibMainWorkerOptions;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::resolution::NpmVersionResolver;
use deno_npm_cache::NpmCacheSetting;
use deno_npm_installer::NpmInstallerFactoryOptions;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsExecutor;
use deno_npm_installer::lifecycle_scripts::NullLifecycleScriptsExecutor;
use deno_npm_installer::process_state::NpmProcessStateKind;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::deno_json::CompilerOptionsOverrides;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::factory::ConfigDiscoveryOption;
use deno_resolver::factory::NpmProcessStateOptions;
use deno_resolver::factory::ResolverFactoryOptions;
use deno_resolver::factory::SpecifiedImportMapProvider;
use deno_resolver::import_map::WorkspaceExternalImportMapLoader;
use deno_resolver::loader::MemoryFiles;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::FeatureChecker;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use node_resolver::NodeConditionOptions;
use node_resolver::NodeResolverOptions;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use once_cell::sync::OnceCell;
use sys_traits::EnvCurrentDir;

use crate::args::BundleFlags;
use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::ConfigFlag;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::InstallFlags;
use crate::cache::Caches;
use crate::cache::CodeCache;
use crate::cache::DenoDir;
use crate::cache::GlobalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::SqliteNodeAnalysisCache;
use crate::file_fetcher::CliFileFetcher;
use crate::file_fetcher::CreateCliFileFetcherOptions;
use crate::file_fetcher::TextDecodedFile;
use crate::file_fetcher::create_cli_file_fetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_util::FileWatcherReporter;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphCreator;
use crate::http_util::HttpClientProvider;
use crate::module_loader::CliEmitter;
use crate::module_loader::CliModuleLoaderFactory;
use crate::module_loader::EszipModuleLoader;
use crate::module_loader::ModuleLoadPreparer;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmGraphResolver;
use crate::npm::CliNpmInstaller;
use crate::npm::CliNpmInstallerFactory;
use crate::npm::CliNpmResolver;
use crate::npm::DenoTaskLifeCycleScriptsExecutor;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliResolver;
use crate::resolver::on_resolve_diagnostic;
use crate::standalone::binary::DenoCompileBinaryWriter;
use crate::sys::CliSys;
use crate::tools::installer::BinNameResolver;
use crate::tools::lint::LintRuleProvider;
use crate::tools::run::hmr::HmrRunnerState;
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
    if self.cli_options.eszip()
      && let DenoSubcommand::Run(run_flags) = self.cli_options.sub_command()
    {
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
    Ok(None)
  }
}

#[derive(Debug)]
struct CliSpecifiedImportMapProvider {
  cli_options: Arc<CliOptions>,
  file_fetcher: Arc<CliFileFetcher>,
  eszip_module_loader_provider: Arc<EszipModuleLoaderProvider>,
  workspace_external_import_map_loader:
    Arc<WorkspaceExternalImportMapLoader<CliSys>>,
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
  cli_options: Deferred<Arc<CliOptions>>,
  code_cache: Deferred<Arc<CodeCache>>,
  eszip_module_loader_provider: Deferred<Arc<EszipModuleLoaderProvider>>,
  feature_checker: Deferred<Arc<FeatureChecker>>,
  file_fetcher: Deferred<Arc<CliFileFetcher>>,
  fs: Deferred<Arc<dyn deno_fs::FileSystem>>,
  http_client_provider: Deferred<Arc<HttpClientProvider>>,
  main_graph_container: Deferred<Arc<MainModuleGraphContainer>>,
  graph_reporter: Deferred<Option<Arc<dyn deno_graph::source::Reporter>>>,
  maybe_inspector_server: Deferred<Option<Arc<InspectorServer>>>,
  memory_files: Arc<MemoryFiles>,
  module_graph_builder: Deferred<Arc<ModuleGraphBuilder>>,
  module_graph_creator: Deferred<Arc<ModuleGraphCreator>>,
  module_info_cache: Deferred<Arc<ModuleInfoCache>>,
  module_load_preparer: Deferred<Arc<ModuleLoadPreparer>>,
  npm_installer_factory: Deferred<CliNpmInstallerFactory>,
  permission_desc_parser:
    Deferred<Arc<RuntimePermissionDescriptorParser<CliSys>>>,
  resolver_factory: Deferred<Arc<CliResolverFactory>>,
  root_cert_store_provider: Deferred<Arc<dyn RootCertStoreProvider>>,
  root_permissions_container: Deferred<PermissionsContainer>,
  text_only_progress_bar: Deferred<ProgressBar>,
  type_checker: Deferred<Arc<TypeChecker>>,
  workspace_factory: Deferred<Arc<CliWorkspaceFactory>>,
  install_reporter:
    Deferred<Option<Arc<crate::tools::installer::InstallReporter>>>,
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
    self.npm_installer_factory()?.maybe_lockfile().await
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

  pub fn deno_dir(&self) -> Result<&DenoDir, AnyError> {
    Ok(
      self
        .workspace_factory()?
        .deno_dir_provider()
        .get_or_create()?,
    )
  }

  pub fn caches(&self) -> Result<&Arc<Caches>, AnyError> {
    self.services.caches.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let caches = Arc::new(Caches::new(
        self.workspace_factory()?.deno_dir_provider().clone(),
      ));
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

  pub fn bin_name_resolver(&self) -> Result<BinNameResolver<'_>, AnyError> {
    let http_client = self.http_client_provider();
    let npm_api = self.npm_installer_factory()?.registry_info_provider()?;
    Ok(BinNameResolver::new(
      http_client,
      npm_api.as_ref(),
      self.npm_version_resolver()?,
    ))
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

  pub fn http_cache(
    &self,
  ) -> Result<&GlobalOrLocalHttpCache<CliSys>, AnyError> {
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
      Ok(Arc::new(create_cli_file_fetcher(
        self.blob_store().clone(),
        self.http_cache()?.clone(),
        self.http_client_provider().clone(),
        self.services.memory_files.clone(),
        self.sys(),
        CreateCliFileFetcherOptions {
          allow_remote: !cli_options.no_remote(),
          cache_setting: cli_options.cache_setting(),
          download_log_level: log::Level::Info,
          progress_bar: Some(self.text_only_progress_bar().clone()),
        },
      )))
    })
  }

  pub fn fs(&self) -> &Arc<dyn deno_fs::FileSystem> {
    self.services.fs.get_or_init(|| Arc::new(RealFs))
  }

  pub fn memory_files(&self) -> &Arc<MemoryFiles> {
    &self.services.memory_files
  }

  pub fn sys(&self) -> CliSys {
    CliSys::default() // very cheap to make
  }

  pub fn in_npm_pkg_checker(
    &self,
  ) -> Result<&DenoInNpmPackageChecker, AnyError> {
    self.resolver_factory()?.in_npm_package_checker()
  }

  pub async fn tsgo_path(&self) -> Result<Option<&PathBuf>, AnyError> {
    if self.cli_options()?.unstable_tsgo() {
      Ok(Some(
        crate::tsc::ensure_tsgo(
          self.deno_dir()?,
          self.http_client_provider().clone(),
        )
        .await?,
      ))
    } else {
      Ok(None)
    }
  }

  pub fn jsr_version_resolver(
    &self,
  ) -> Result<&Arc<JsrVersionResolver>, AnyError> {
    self.resolver_factory()?.jsr_version_resolver()
  }

  pub fn npm_cache(&self) -> Result<&Arc<CliNpmCache>, AnyError> {
    self.npm_installer_factory()?.npm_cache()
  }

  pub fn npm_cache_dir(&self) -> Result<&Arc<NpmCacheDir>, AnyError> {
    Ok(self.workspace_factory()?.npm_cache_dir()?)
  }

  pub fn npmrc(&self) -> Result<&Arc<ResolvedNpmRc>, AnyError> {
    Ok(self.workspace_factory()?.npmrc()?)
  }

  pub async fn npm_graph_resolver(
    &self,
  ) -> Result<&Arc<CliNpmGraphResolver>, AnyError> {
    self
      .npm_installer_factory()?
      .npm_deno_graph_resolver()
      .await
  }

  pub async fn npm_installer_if_managed(
    &self,
  ) -> Result<Option<&Arc<CliNpmInstaller>>, AnyError> {
    self
      .npm_installer_factory()?
      .npm_installer_if_managed()
      .await
  }

  pub fn npm_installer_factory(
    &self,
  ) -> Result<&CliNpmInstallerFactory, AnyError> {
    self.services.npm_installer_factory.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let resolver_factory = self.resolver_factory()?;
      Ok(CliNpmInstallerFactory::new(
        resolver_factory.clone(),
        Arc::new(CliNpmCacheHttpClient::new(
          self.http_client_provider().clone(),
          self.text_only_progress_bar().clone(),
        )),
        match resolver_factory.npm_resolver()?.as_managed() {
          Some(managed_npm_resolver) => {
            Arc::new(DenoTaskLifeCycleScriptsExecutor::new(
              managed_npm_resolver.clone(),
              self.text_only_progress_bar().clone(),
            )) as Arc<dyn LifecycleScriptsExecutor>
          }
          None => Arc::new(NullLifecycleScriptsExecutor),
        },
        self.text_only_progress_bar().clone(),
        self
          .install_reporter()?
          .cloned()
          .map(|r| r as Arc<dyn deno_npm_installer::InstallReporter>),
        NpmInstallerFactoryOptions {
          cache_setting: NpmCacheSetting::from_cache_setting(
            &cli_options.cache_setting(),
          ),
          caching_strategy: cli_options.default_npm_caching_strategy(),
          lifecycle_scripts_config: cli_options.lifecycle_scripts_config(),
          resolve_npm_resolution_snapshot: Box::new(|| {
            deno_lib::args::resolve_npm_resolution_snapshot(&CliSys::default())
          }),
        },
      ))
    })
  }

  pub fn npm_version_resolver(
    &self,
  ) -> Result<&Arc<NpmVersionResolver>, AnyError> {
    self.resolver_factory()?.npm_version_resolver()
  }

  pub fn install_reporter(
    &self,
  ) -> Result<Option<&Arc<crate::tools::installer::InstallReporter>>, AnyError>
  {
    self
      .services
      .install_reporter
      .get_or_try_init(|| match self.cli_options()?.sub_command() {
        DenoSubcommand::Install(InstallFlags::Local(_))
        | DenoSubcommand::Add(_)
        | DenoSubcommand::Cache(_) => Ok(Some(Arc::new(
          crate::tools::installer::InstallReporter::new(),
        ))),
        _ => Ok(None),
      })
      .map(|opt| opt.as_ref())
  }

  pub async fn npm_installer(&self) -> Result<&Arc<CliNpmInstaller>, AnyError> {
    self.npm_installer_factory()?.npm_installer().await
  }

  pub async fn npm_resolver(&self) -> Result<&CliNpmResolver, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.npm_resolver()
  }

  fn workspace_factory(&self) -> Result<&Arc<CliWorkspaceFactory>, AnyError> {
    self.services.workspace_factory.get_or_try_init(|| {
      let initial_cwd = match self.overrides.initial_cwd.clone() {
        Some(v) => v,
        None => {
          if let Some(initial_cwd) = self.flags.initial_cwd.clone() {
            initial_cwd
          } else {
            self
              .sys()
              .env_current_dir()
              .with_context(|| "Failed getting cwd.")?
          }
        }
      };
      let options = new_workspace_factory_options(&initial_cwd, &self.flags);
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

  pub async fn resolver(&self) -> Result<&Arc<CliResolver>, AnyError> {
    self.initialize_npm_resolution_if_managed().await?;
    self.resolver_factory()?.deno_resolver().await
  }

  pub fn graph_reporter(
    &self,
  ) -> Result<&Option<Arc<dyn deno_graph::source::Reporter>>, AnyError> {
    match self.cli_options()?.sub_command() {
      DenoSubcommand::Install(_) => {
        self.services.graph_reporter.get_or_try_init(|| {
          self.install_reporter().map(|opt| {
            opt.map(|r| r.clone() as Arc<dyn deno_graph::source::Reporter>)
          })
        })
      }
      _ => Ok(self.services.graph_reporter.get_or_init(|| {
        self
          .watcher_communicator
          .as_ref()
          .map(|i| FileWatcherReporter::new(i.clone()))
          .map(|i| Arc::new(i) as Arc<dyn deno_graph::source::Reporter>)
      })),
    }
  }

  pub fn module_info_cache(&self) -> Result<&Arc<ModuleInfoCache>, AnyError> {
    self.services.module_info_cache.get_or_try_init(|| {
      Ok(Arc::new(ModuleInfoCache::new(
        self.caches()?.dep_analysis_db(),
        self.resolver_factory()?.parsed_source_cache().clone(),
      )))
    })
  }

  pub fn code_cache(&self) -> Result<&Arc<CodeCache>, AnyError> {
    self.services.code_cache.get_or_try_init(|| {
      Ok(Arc::new(CodeCache::new(self.caches()?.code_cache_db())))
    })
  }

  pub fn parsed_source_cache(
    &self,
  ) -> Result<&Arc<ParsedSourceCache>, AnyError> {
    Ok(self.resolver_factory()?.parsed_source_cache())
  }

  pub fn emitter(&self) -> Result<&Arc<CliEmitter>, AnyError> {
    self.resolver_factory()?.emitter()
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
    self
      .npm_installer_factory()?
      .initialize_npm_resolution_if_managed()
      .await
  }

  pub fn pkg_json_resolver(
    &self,
  ) -> Result<&Arc<CliPackageJsonResolver>, AnyError> {
    Ok(self.resolver_factory()?.pkg_json_resolver())
  }

  pub fn compiler_options_resolver(
    &self,
  ) -> Result<&Arc<CompilerOptionsResolver>, AnyError> {
    self.resolver_factory()?.compiler_options_resolver()
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
            self.npm_resolver().await?.clone(),
            self.resolver_factory()?.pkg_json_resolver().clone(),
            self.sys(),
            self.compiler_options_resolver()?.clone(),
            if cli_options.code_cache_enabled() {
              Some(self.code_cache()?.clone())
            } else {
              None
            },
            self.tsgo_path().await?.cloned(),
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
            self.jsr_version_resolver()?.clone(),
            self.maybe_lockfile().await?.cloned(),
            self.graph_reporter()?.clone(),
            self.module_info_cache()?.clone(),
            self.npm_graph_resolver().await?.clone(),
            self.npm_installer_if_managed().await?.cloned(),
            self.npm_resolver().await?.clone(),
            self.resolver_factory()?.parsed_source_cache().clone(),
            self.text_only_progress_bar().clone(),
            self.resolver().await?.clone(),
            self.root_permissions_container()?.clone(),
            self.sys(),
            self.compiler_options_resolver()?.clone(),
            self.install_reporter()?.cloned().map(|r| {
              r as Arc<dyn deno_resolver::file_fetcher::GraphLoaderReporter>
            }),
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
    self.resolver_factory()?.cjs_tracker()
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
      for feature in deno_runtime::UNSTABLE_FEATURES {
        if unstable_features.contains(&feature.name) {
          checker.enable_feature(feature.name);
        }
      }

      Ok(Arc::new(checker))
    })
  }

  pub async fn create_compile_binary_writer(
    &self,
  ) -> Result<DenoCompileBinaryWriter<'_>, AnyError> {
    let cli_options = self.cli_options()?;
    Ok(DenoCompileBinaryWriter::new(
      self.resolver_factory()?.cjs_module_export_analyzer()?,
      self.cjs_tracker()?,
      self.cli_options()?,
      self.deno_dir()?,
      self.emitter()?,
      self.file_fetcher()?,
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
          &self.cli_options()?.permissions_options()?,
        )?;

        Ok(PermissionsContainer::new(desc_parser, permissions))
      })
  }

  fn workspace_external_import_map_loader(
    &self,
  ) -> Result<&Arc<WorkspaceExternalImportMapLoader<CliSys>>, AnyError> {
    Ok(
      self
        .workspace_factory()?
        .workspace_external_import_map_loader()?,
    )
  }

  pub async fn create_cli_main_worker_factory(
    &self,
  ) -> Result<CliMainWorkerFactory, AnyError> {
    self
      .create_cli_main_worker_factory_with_roots(Default::default())
      .await
  }

  pub async fn create_module_loader_factory(
    &self,
  ) -> Result<CliModuleLoaderFactory, AnyError> {
    let cli_options = self.cli_options()?;
    let cli_npm_resolver = self.npm_resolver().await?.clone();
    let in_npm_pkg_checker = self.in_npm_pkg_checker()?;
    let workspace_factory = self.workspace_factory()?;
    let resolver_factory = self.resolver_factory()?;
    let npm_installer_factory = self.npm_installer_factory()?;
    let cjs_tracker = self.cjs_tracker()?.clone();
    let npm_registry_permission_checker = {
      let mode = if resolver_factory.use_byonm()? {
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
      self.file_fetcher()?.clone(),
      npm_installer_factory
        .has_js_execution_started_flag()
        .clone(),
      in_npm_pkg_checker.clone(),
      self.main_module_graph_container().await?.clone(),
      self.memory_files().clone(),
      self.module_load_preparer().await?.clone(),
      npm_registry_permission_checker,
      cli_npm_resolver.clone(),
      resolver_factory.parsed_source_cache().clone(),
      resolver_factory.module_loader()?.clone(),
      self.resolver().await?.clone(),
      self.sys(),
      maybe_eszip_loader,
    );

    Ok(module_loader_factory)
  }

  pub async fn create_cli_main_worker_factory_with_roots(
    &self,
    roots: LibWorkerFactoryRoots,
  ) -> Result<CliMainWorkerFactory, AnyError> {
    let cli_options = self.cli_options()?;
    let fs = self.fs();
    let node_resolver = self.node_resolver().await?;
    let npm_resolver = self.npm_resolver().await?;
    let maybe_file_watcher_communicator = if cli_options.has_hmr() {
      Some(self.watcher_communicator.clone().unwrap())
    } else {
      None
    };
    let pkg_json_resolver = self.pkg_json_resolver()?;
    let module_loader_factory = self.create_module_loader_factory().await?;

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
      cli_options.coverage_dir(),
      self.maybe_inspector_server()?.clone(),
      Box::new(module_loader_factory),
      node_resolver.clone(),
      create_npm_process_state_provider(npm_resolver),
      pkg_json_resolver.clone(),
      self.root_cert_store_provider().clone(),
      cli_options.resolve_storage_key_resolver(),
      self.sys(),
      self.create_lib_main_worker_options()?,
      roots,
      Some(Arc::new(crate::tools::bundle::CliBundleProvider::new(
        self.flags.clone(),
      ))),
    );

    Ok(CliMainWorkerFactory::new(
      lib_main_worker_factory,
      maybe_file_watcher_communicator,
      self.maybe_lockfile().await?.cloned(),
      self.npm_installer_if_managed().await?.cloned(),
      npm_resolver.clone(),
      self.text_only_progress_bar().clone(),
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
      trace_ops: cli_options.trace_ops().clone(),
      is_standalone: false,
      auto_serve: std::env::var("DENO_AUTO_SERVE").is_ok(),
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
      node_ipc_init: cli_options.node_ipc_init()?,
      serve_port: cli_options.serve_port(),
      serve_host: cli_options.serve_host(),
      otel_config: cli_options.otel_config(),
      no_legacy_abort: cli_options.no_legacy_abort(),
      startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
      enable_raw_imports: cli_options.unstable_raw_imports(),
      maybe_initial_cwd: Some(deno_path_util::url_from_directory_path(
        cli_options.initial_cwd(),
      )?),
    })
  }

  fn create_cli_main_worker_options(
    &self,
  ) -> Result<CliMainWorkerOptions, AnyError> {
    let cli_options = self.cli_options()?;
    let create_hmr_runner = if cli_options.has_hmr() {
      let watcher_communicator = self.watcher_communicator.clone().unwrap();
      let emitter = self.emitter()?.clone();
      let fn_: crate::worker::CreateHmrRunnerCb = Box::new(move || {
        HmrRunnerState::new(emitter.clone(), watcher_communicator.clone())
      });
      Some(fn_)
    } else {
      None
    };
    let maybe_coverage_dir = cli_options.coverage_dir();

    let initial_cwd =
      deno_path_util::url_from_directory_path(cli_options.initial_cwd())?;

    Ok(CliMainWorkerOptions {
      needs_test_modules: cli_options.sub_command().needs_test(),
      create_hmr_runner,
      maybe_coverage_dir,
      default_npm_caching_strategy: cli_options.default_npm_caching_strategy(),
      maybe_initial_cwd: Some(Arc::new(initial_cwd)),
    })
  }

  pub fn resolver_factory(&self) -> Result<&Arc<CliResolverFactory>, AnyError> {
    self.services.resolver_factory.get_or_try_init(|| {
      let options = self.cli_options()?;
      let caches = self.caches()?;
      let node_analysis_cache =
        Arc::new(SqliteNodeAnalysisCache::new(caches.node_analysis_db()));
      Ok(Arc::new(CliResolverFactory::new(
        self.workspace_factory()?.clone(),
        ResolverFactoryOptions {
          compiler_options_overrides: CompilerOptionsOverrides {
            no_transpile: false,
            source_map_base: None,
            preserve_jsx: false,
          },
          is_cjs_resolution_mode: if options.is_node_main()
            || options.unstable_detect_cjs()
          {
            IsCjsResolutionMode::ImplicitTypeCommonJs
          } else if options.detect_cjs() {
            IsCjsResolutionMode::ExplicitTypeCommonJs
          } else {
            IsCjsResolutionMode::Disabled
          },
          newest_dependency_date: self.flags.minimum_dependency_age,
          node_analysis_cache: Some(node_analysis_cache),
          node_resolver_options: NodeResolverOptions {
            conditions: NodeConditionOptions {
              conditions: options
                .node_conditions()
                .iter()
                .map(|c| Cow::Owned(c.clone()))
                .collect(),
              import_conditions_override: None,
              require_conditions_override: None,
            },
            typescript_version: Some(
              deno_semver::Version::parse_standard(
                deno_lib::version::DENO_VERSION_INFO.typescript,
              )
              .unwrap(),
            ),
            bundle_mode: matches!(
              self.flags.subcommand,
              DenoSubcommand::Bundle(_)
            ),
            is_browser_platform: matches!(
              self.flags.subcommand,
              DenoSubcommand::Bundle(BundleFlags {
                platform: BundlePlatform::Browser,
                ..
              })
            ),
          },
          node_code_translator_mode: match options.sub_command() {
            DenoSubcommand::Bundle(_) => {
              node_resolver::analyze::NodeCodeTranslatorMode::Disabled
            }
            _ => node_resolver::analyze::NodeCodeTranslatorMode::ModuleLoader,
          },
          node_resolution_cache: Some(Arc::new(NodeResolutionThreadLocalCache)),
          npm_system_info: self.flags.subcommand.npm_system_info(),
          specified_import_map: Some(Box::new(CliSpecifiedImportMapProvider {
            cli_options: options.clone(),
            eszip_module_loader_provider: self
              .eszip_module_loader_provider()?
              .clone(),
            file_fetcher: self.file_fetcher()?.clone(),
            workspace_external_import_map_loader: self
              .workspace_external_import_map_loader()?
              .clone(),
          })),
          bare_node_builtins: options.unstable_bare_node_builtins(),
          unstable_sloppy_imports: options.unstable_sloppy_imports(),
          on_mapped_resolution_diagnostic: Some(Arc::new(
            on_resolve_diagnostic,
          )),
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
          allow_json_imports: if matches!(
            self.flags.subcommand,
            DenoSubcommand::Bundle(_)
          ) {
            deno_resolver::loader::AllowJsonImports::Always
          } else {
            deno_resolver::loader::AllowJsonImports::WithAttribute
          },
          require_modules: options.require_modules()?,
        },
      )))
    })
  }
}

fn new_workspace_factory_options(
  initial_cwd: &Path,
  flags: &Flags,
) -> deno_resolver::factory::WorkspaceFactoryOptions {
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
        ConfigDiscoveryOption::Path(initial_cwd.join(path))
      }
      ConfigFlag::Disabled => ConfigDiscoveryOption::Disabled,
    },
    maybe_custom_deno_dir_root: flags.internal.cache_path.clone(),
    // For `deno install/add/remove/init` we want to force the managed
    // resolver so it can set up the `node_modules/` directory.
    is_package_manager_subcommand: matches!(
      flags.subcommand,
      DenoSubcommand::Add(_)
        | DenoSubcommand::Audit(_)
        | DenoSubcommand::Clean(_)
        | DenoSubcommand::Init(_)
        | DenoSubcommand::Install(_)
        | DenoSubcommand::Outdated(_)
        | DenoSubcommand::Remove(_)
        | DenoSubcommand::Uninstall(_)
        | DenoSubcommand::ApproveScripts(_)
    ),
    no_lock: flags.no_lock
      || matches!(
        flags.subcommand,
        DenoSubcommand::Install(InstallFlags::Global(..))
          | DenoSubcommand::Uninstall(_)
      ),
    frozen_lockfile: flags.frozen_lockfile,
    lock_arg: flags.lock.as_ref().map(|l| initial_cwd.join(l)),
    lockfile_skip_write: flags.internal.lockfile_skip_write,
    no_npm: flags.no_npm,
    node_modules_dir: flags.node_modules_dir,
    npm_process_state: npm_process_state(&CliSys::default()).as_ref().map(
      |s| NpmProcessStateOptions {
        node_modules_dir: s
          .local_node_modules_path
          .as_ref()
          .map(|s| Cow::Borrowed(s.as_str())),
        is_byonm: matches!(s.kind, NpmProcessStateKind::Byonm),
      },
    ),
    root_node_modules_dir_override: flags
      .internal
      .root_node_modules_dir_override
      .clone(),
    vendor: flags.vendor,
  }
}
