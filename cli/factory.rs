// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeSet;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::file_fetcher::File;
use deno_cache_dir::npm::NpmCacheDir;
use deno_config::glob::FilePatterns;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::FeatureChecker;
use deno_error::JsErrorBox;
use deno_lib::cache::DenoDir;
use deno_lib::cache::DenoDirProvider;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::npm::NpmRegistryReadPermissionCheckerMode;
use deno_lib::worker::LibMainWorkerFactory;
use deno_lib::worker::LibMainWorkerOptions;
use deno_npm_cache::NpmCacheSetting;
use deno_path_util::url_to_file_path;
use deno_resolver::cjs::IsCjsResolutionMode;
use deno_resolver::npm::managed::ManagedInNpmPkgCheckerCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolverOptions;
use deno_resolver::sloppy_imports::SloppyImportsCachedFs;
use deno_resolver::DenoResolverOptions;
use deno_resolver::NodeAndNpmReqResolver;
use deno_runtime::deno_fs;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_node::RealIsBuiltInNodeModuleChecker;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use log::warn;
use node_resolver::analyze::NodeCodeTranslator;
use once_cell::sync::OnceCell;

use crate::args::check_warn_tsconfig;
use crate::args::get_root_cert_store;
use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::NpmInstallDepsProvider;
use crate::args::ScopeOptions;
use crate::args::TsConfigType;
use crate::cache::Caches;
use crate::cache::CodeCache;
use crate::cache::EmitCache;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_util::FileWatcherReporter;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphCreator;
use crate::http_util::HttpClientProvider;
use crate::module_loader::CliModuleLoaderFactory;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadError;
use crate::node::CliCjsCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::create_npm_process_state_provider;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::NpmResolutionInstaller;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliManagedNpmResolverCreateOptions;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmCacheHttpClient;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::npm::CliNpmTarballCache;
use crate::npm::NpmResolutionInitializer;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliDenoResolver;
use crate::resolver::CliNpmGraphResolver;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::resolver::CliSloppyImportsResolver;
use crate::resolver::FoundPackageJsonDepFlag;
use crate::resolver::NpmModuleLoader;
use crate::standalone::binary::DenoCompileBinaryWriter;
use crate::sys::CliSys;
use crate::tools::check::CheckError;
use crate::tools::check::TypeChecker;
use crate::tools::coverage::CoverageCollector;
use crate::tools::lint::LintRuleProvider;
use crate::tools::run::hmr::HmrRunner;
use crate::tsc::Diagnostics;
use crate::tsc::TypeCheckingCjsTracker;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path;
use crate::util::fs::canonicalize_path_maybe_not_exists;
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

pub struct Deferred<T>(once_cell::unsync::OnceCell<T>);

impl<T> Default for Deferred<T> {
  fn default() -> Self {
    Self(once_cell::unsync::OnceCell::default())
  }
}

impl<T> Deferred<T> {
  pub fn from_value(value: T) -> Self {
    Self(once_cell::unsync::OnceCell::from(value))
  }

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
  cjs_tracker: Deferred<Arc<CliCjsTracker>>,
  cli_options: Deferred<Arc<CliOptions>>,
  code_cache: Deferred<Arc<CodeCache>>,
  deno_resolver: Deferred<Arc<CliDenoResolver>>,
  emit_cache: Deferred<Arc<EmitCache>>,
  emitter: Deferred<Arc<Emitter>>,
  feature_checker: Deferred<Arc<FeatureChecker>>,
  file_fetcher: Deferred<Arc<CliFileFetcher>>,
  found_pkg_json_dep_flag: Arc<FoundPackageJsonDepFlag>,
  fs: Deferred<Arc<dyn deno_fs::FileSystem>>,
  global_http_cache: Deferred<Arc<GlobalHttpCache>>,
  http_cache: Deferred<Arc<dyn HttpCache>>,
  http_client_provider: Deferred<Arc<HttpClientProvider>>,
  in_npm_pkg_checker: Deferred<DenoInNpmPackageChecker>,
  main_graph_container: Deferred<Arc<MainModuleGraphContainer>>,
  maybe_file_watcher_reporter: Deferred<Option<FileWatcherReporter>>,
  maybe_inspector_server: Deferred<Option<Arc<InspectorServer>>>,
  module_graph_builder: Deferred<Arc<ModuleGraphBuilder>>,
  module_graph_creator: Deferred<Arc<ModuleGraphCreator>>,
  module_info_cache: Deferred<Arc<ModuleInfoCache>>,
  module_load_preparer: Deferred<Arc<ModuleLoadPreparer>>,
  node_code_translator: Deferred<Arc<CliNodeCodeTranslator>>,
  node_resolver: Deferred<Arc<CliNodeResolver>>,
  npm_cache: Deferred<Arc<CliNpmCache>>,
  npm_cache_dir: Deferred<Arc<NpmCacheDir>>,
  npm_cache_http_client: Deferred<Arc<CliNpmCacheHttpClient>>,
  npm_graph_resolver: Deferred<Arc<CliNpmGraphResolver>>,
  npm_installer: Deferred<Arc<NpmInstaller>>,
  npm_registry_info_provider: Deferred<Arc<CliNpmRegistryInfoProvider>>,
  npm_req_resolver: Deferred<Arc<CliNpmReqResolver>>,
  npm_resolution: Arc<NpmResolutionCell>,
  npm_resolution_initializer: Deferred<Arc<NpmResolutionInitializer>>,
  npm_resolution_installer: Deferred<Arc<NpmResolutionInstaller>>,
  npm_resolver: Deferred<CliNpmResolver>,
  npm_tarball_cache: Deferred<Arc<CliNpmTarballCache>>,
  parsed_source_cache: Deferred<Arc<ParsedSourceCache>>,
  permission_desc_parser:
    Deferred<Arc<RuntimePermissionDescriptorParser<CliSys>>>,
  pkg_json_resolver: Deferred<Arc<CliPackageJsonResolver>>,
  resolver: Deferred<Arc<CliResolver>>,
  root_cert_store_provider: Deferred<Arc<dyn RootCertStoreProvider>>,
  root_permissions_container: Deferred<PermissionsContainer>,
  sloppy_imports_resolver: Deferred<Option<Arc<CliSloppyImportsResolver>>>,
  text_only_progress_bar: Deferred<ProgressBar>,
  type_checker: Deferred<Arc<TypeChecker>>,
  workspace_resolver: Deferred<Arc<WorkspaceResolver>>,
}

pub struct CliFactory {
  watcher_communicator: Option<Arc<WatcherCommunicator>>,
  flags: Arc<Flags>,
  services: CliFactoryServices,
}

impl CliFactory {
  pub fn from_flags(flags: Arc<Flags>) -> Self {
    Self {
      flags,
      watcher_communicator: None,
      services: Default::default(),
    }
  }

  pub fn from_cli_options(cli_options: Arc<CliOptions>) -> Self {
    let (cli_options, flags) = cli_options.into_self_and_flags();
    CliFactory {
      watcher_communicator: None,
      flags,
      services: CliFactoryServices {
        cli_options: Deferred::from_value(cli_options),
        ..Default::default()
      },
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
    }
  }

  pub fn cli_options(&self) -> Result<&Arc<CliOptions>, AnyError> {
    self.services.cli_options.get_or_try_init(|| {
      CliOptions::from_flags(&self.sys(), self.flags.clone()).map(Arc::new)
    })
  }

  pub fn deno_dir_provider(
    &self,
  ) -> Result<&Arc<DenoDirProvider<CliSys>>, AnyError> {
    Ok(&self.cli_options()?.deno_dir_provider)
  }

  pub fn deno_dir(&self) -> Result<&DenoDir<CliSys>, AnyError> {
    Ok(self.deno_dir_provider()?.get_or_create()?)
  }

  pub fn caches(&self) -> Result<&Arc<Caches>, AnyError> {
    self.services.caches.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let caches = Arc::new(Caches::new(self.deno_dir_provider()?.clone()));
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
    self.services.global_http_cache.get_or_try_init(|| {
      Ok(Arc::new(GlobalHttpCache::new(
        self.sys(),
        self.deno_dir()?.remote_folder_path(),
      )))
    })
  }

  pub fn http_cache(&self) -> Result<&Arc<dyn HttpCache>, AnyError> {
    self.services.http_cache.get_or_try_init(|| {
      let global_cache = self.global_http_cache()?.clone();
      match self.cli_options()?.vendor_dir_path() {
        Some(local_path) => {
          let local_cache = LocalHttpCache::new(
            local_path.clone(),
            global_cache,
            deno_cache_dir::GlobalToLocalCopy::Allow,
          );
          Ok(Arc::new(local_cache))
        }
        None => Ok(global_cache),
      }
    })
  }

  pub fn http_client_provider(&self) -> &Arc<HttpClientProvider> {
    self.services.http_client_provider.get_or_init(|| {
      Arc::new(HttpClientProvider::new(
        Some(self.root_cert_store_provider().clone()),
        self.flags.unsafely_ignore_certificate_errors.clone(),
      ))
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
    self.services.in_npm_pkg_checker.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      let options = if cli_options.use_byonm() {
        CreateInNpmPkgCheckerOptions::Byonm
      } else {
        CreateInNpmPkgCheckerOptions::Managed(
          ManagedInNpmPkgCheckerCreateOptions {
            root_cache_dir_url: self.npm_cache_dir()?.root_dir_url(),
            maybe_node_modules_path: cli_options
              .node_modules_dir_path()
              .map(|p| p.as_path()),
          },
        )
      };
      Ok(DenoInNpmPackageChecker::new(options))
    })
  }

  pub fn npm_cache(&self) -> Result<&Arc<CliNpmCache>, AnyError> {
    self.services.npm_cache.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(CliNpmCache::new(
        self.npm_cache_dir()?.clone(),
        self.sys(),
        NpmCacheSetting::from_cache_setting(&cli_options.cache_setting()),
        cli_options.npmrc().clone(),
      )))
    })
  }

  pub fn npm_cache_dir(&self) -> Result<&Arc<NpmCacheDir>, AnyError> {
    self.services.npm_cache_dir.get_or_try_init(|| {
      let global_path = self.deno_dir()?.npm_folder_path();
      let cli_options = self.cli_options()?;
      Ok(Arc::new(NpmCacheDir::new(
        &self.sys(),
        global_path,
        cli_options.npmrc().get_all_known_registries_urls(),
      )))
    })
  }

  pub fn npm_cache_http_client(&self) -> &Arc<CliNpmCacheHttpClient> {
    self.services.npm_cache_http_client.get_or_init(|| {
      Arc::new(CliNpmCacheHttpClient::new(
        self.http_client_provider().clone(),
        self.text_only_progress_bar().clone(),
      ))
    })
  }

  pub fn npm_graph_resolver(
    &self,
  ) -> Result<&Arc<CliNpmGraphResolver>, AnyError> {
    self.services.npm_graph_resolver.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(CliNpmGraphResolver::new(
        self.npm_installer_if_managed()?.cloned(),
        self.services.found_pkg_json_dep_flag.clone(),
        cli_options.unstable_bare_node_builtins(),
        cli_options.default_npm_caching_strategy(),
      )))
    })
  }

  pub fn npm_installer_if_managed(
    &self,
  ) -> Result<Option<&Arc<NpmInstaller>>, AnyError> {
    let options = self.cli_options()?;
    if options.use_byonm() || options.no_npm() {
      Ok(None)
    } else {
      Ok(Some(self.npm_installer()?))
    }
  }

  pub fn npm_installer(&self) -> Result<&Arc<NpmInstaller>, AnyError> {
    self.services.npm_installer.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(NpmInstaller::new(
        self.npm_cache()?.clone(),
        Arc::new(NpmInstallDepsProvider::from_workspace(
          cli_options.workspace(),
        )),
        self.npm_resolution().clone(),
        self.npm_resolution_initializer()?.clone(),
        self.npm_resolution_installer()?.clone(),
        self.text_only_progress_bar(),
        self.sys(),
        self.npm_tarball_cache()?.clone(),
        cli_options.maybe_lockfile().cloned(),
        cli_options.node_modules_dir_path().cloned(),
        cli_options.lifecycle_scripts_config(),
        cli_options.npm_system_info(),
      )))
    })
  }

  pub fn npm_registry_info_provider(
    &self,
  ) -> Result<&Arc<CliNpmRegistryInfoProvider>, AnyError> {
    self
      .services
      .npm_registry_info_provider
      .get_or_try_init(|| {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(CliNpmRegistryInfoProvider::new(
          self.npm_cache()?.clone(),
          self.npm_cache_http_client().clone(),
          cli_options.npmrc().clone(),
        )))
      })
  }

  pub fn npm_resolution(&self) -> &Arc<NpmResolutionCell> {
    &self.services.npm_resolution
  }

  pub fn npm_resolution_initializer(
    &self,
  ) -> Result<&Arc<NpmResolutionInitializer>, AnyError> {
    self
      .services
      .npm_resolution_initializer
      .get_or_try_init(|| {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(NpmResolutionInitializer::new(
          self.npm_registry_info_provider()?.clone(),
          self.npm_resolution().clone(),
          match cli_options.resolve_npm_resolution_snapshot()? {
            Some(snapshot) => {
              CliNpmResolverManagedSnapshotOption::Specified(Some(snapshot))
            }
            None => match cli_options.maybe_lockfile() {
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
  }

  pub fn npm_resolution_installer(
    &self,
  ) -> Result<&Arc<NpmResolutionInstaller>, AnyError> {
    self.services.npm_resolution_installer.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(NpmResolutionInstaller::new(
        self.npm_registry_info_provider()?.clone(),
        self.npm_resolution().clone(),
        cli_options.maybe_lockfile().cloned(),
      )))
    })
  }

  pub async fn npm_resolver(&self) -> Result<&CliNpmResolver, AnyError> {
    self
      .services
      .npm_resolver
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(CliNpmResolver::new(if cli_options.use_byonm() {
            CliNpmResolverCreateOptions::Byonm(
              CliByonmNpmResolverCreateOptions {
                sys: self.sys(),
                pkg_json_resolver: self.pkg_json_resolver().clone(),
                root_node_modules_dir: Some(
                  match cli_options.node_modules_dir_path() {
                    Some(node_modules_path) => node_modules_path.to_path_buf(),
                    // path needs to be canonicalized for node resolution
                    // (node_modules_dir_path above is already canonicalized)
                    None => canonicalize_path_maybe_not_exists(
                      cli_options.initial_cwd(),
                    )?
                    .join("node_modules"),
                  },
                ),
              },
            )
          } else {
            self
              .npm_resolution_initializer()?
              .ensure_initialized()
              .await?;
            CliNpmResolverCreateOptions::Managed(
              CliManagedNpmResolverCreateOptions {
                sys: self.sys(),
                npm_resolution: self.npm_resolution().clone(),
                npm_cache_dir: self.npm_cache_dir()?.clone(),
                maybe_node_modules_path: cli_options
                  .node_modules_dir_path()
                  .cloned(),
                npm_system_info: cli_options.npm_system_info(),
                npmrc: cli_options.npmrc().clone(),
              },
            )
          }))
        }
        .boxed_local(),
      )
      .await
  }

  pub fn npm_tarball_cache(
    &self,
  ) -> Result<&Arc<CliNpmTarballCache>, AnyError> {
    self.services.npm_tarball_cache.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(CliNpmTarballCache::new(
        self.npm_cache()?.clone(),
        self.npm_cache_http_client().clone(),
        self.sys(),
        cli_options.npmrc().clone(),
      )))
    })
  }

  pub fn sloppy_imports_resolver(
    &self,
  ) -> Result<Option<&Arc<CliSloppyImportsResolver>>, AnyError> {
    self
      .services
      .sloppy_imports_resolver
      .get_or_try_init(|| {
        Ok(self.cli_options()?.unstable_sloppy_imports().then(|| {
          Arc::new(CliSloppyImportsResolver::new(SloppyImportsCachedFs::new(
            self.sys(),
          )))
        }))
      })
      .map(|maybe| maybe.as_ref())
  }

  pub async fn workspace_resolver(
    &self,
  ) -> Result<&Arc<WorkspaceResolver>, AnyError> {
    self
      .services
      .workspace_resolver
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        let resolver = cli_options
          .create_workspace_resolver(
            self.file_fetcher()?,
            if cli_options.use_byonm()
              && !matches!(
                cli_options.sub_command(),
                DenoSubcommand::Publish(_)
              )
            {
              PackageJsonDepResolution::Disabled
            } else {
              // todo(dsherret): this should be false for nodeModulesDir: true
              PackageJsonDepResolution::Enabled
            },
          )
          .await?;
        if !resolver.diagnostics().is_empty() {
          warn!(
            "Import map diagnostics:\n{}",
            resolver
              .diagnostics()
              .iter()
              .map(|d| format!("  - {d}"))
              .collect::<Vec<_>>()
              .join("\n")
          );
        }
        Ok(Arc::new(resolver))
      })
      .await
  }

  pub async fn deno_resolver(&self) -> Result<&Arc<CliDenoResolver>, AnyError> {
    self
      .services
      .deno_resolver
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(CliDenoResolver::new(DenoResolverOptions {
          in_npm_pkg_checker: self.in_npm_pkg_checker()?.clone(),
          node_and_req_resolver: if cli_options.no_npm() {
            None
          } else {
            Some(NodeAndNpmReqResolver {
              node_resolver: self.node_resolver().await?.clone(),
              npm_req_resolver: self.npm_req_resolver().await?.clone(),
            })
          },
          sloppy_imports_resolver: self.sloppy_imports_resolver()?.cloned(),
          workspace_resolver: self.workspace_resolver().await?.clone(),
          is_byonm: cli_options.use_byonm(),
          maybe_vendor_dir: cli_options.vendor_dir_path(),
        })))
      })
      .await
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
      let cli_options = self.cli_options()?;
      let ts_config_result =
        cli_options.resolve_ts_config_for_emit(TsConfigType::Emit)?;
      check_warn_tsconfig(&ts_config_result);
      let (transpile_options, emit_options) =
        crate::args::ts_config_to_transpile_and_emit_options(
          ts_config_result.ts_config,
        )?;
      Ok(Arc::new(Emitter::new(
        self.cjs_tracker()?.clone(),
        self.emit_cache()?.clone(),
        self.parsed_source_cache().clone(),
        transpile_options,
        emit_options,
      )))
    })
  }

  pub async fn lint_rule_provider(&self) -> Result<LintRuleProvider, AnyError> {
    Ok(LintRuleProvider::new(
      self.sloppy_imports_resolver()?.cloned(),
      Some(self.workspace_resolver().await?.clone()),
    ))
  }

  pub async fn node_resolver(&self) -> Result<&Arc<CliNodeResolver>, AnyError> {
    self
      .services
      .node_resolver
      .get_or_try_init_async(
        async {
          Ok(Arc::new(CliNodeResolver::new(
            self.in_npm_pkg_checker()?.clone(),
            RealIsBuiltInNodeModuleChecker,
            self.npm_resolver().await?.clone(),
            self.pkg_json_resolver().clone(),
            self.sys(),
            node_resolver::ConditionsFromResolutionMode::default(),
          )))
        }
        .boxed_local(),
      )
      .await
  }

  pub async fn node_code_translator(
    &self,
  ) -> Result<&Arc<CliNodeCodeTranslator>, AnyError> {
    self
      .services
      .node_code_translator
      .get_or_try_init_async(async {
        let caches = self.caches()?;
        let node_analysis_cache =
          NodeAnalysisCache::new(caches.node_analysis_db());
        let node_resolver = self.node_resolver().await?.clone();
        let cjs_esm_analyzer = CliCjsCodeAnalyzer::new(
          node_analysis_cache,
          self.cjs_tracker()?.clone(),
          self.fs().clone(),
          Some(self.parsed_source_cache().clone()),
        );

        Ok(Arc::new(NodeCodeTranslator::new(
          cjs_esm_analyzer,
          self.in_npm_pkg_checker()?.clone(),
          node_resolver,
          self.npm_resolver().await?.clone(),
          self.pkg_json_resolver().clone(),
          self.sys(),
        )))
      })
      .await
  }

  pub async fn npm_req_resolver(
    &self,
  ) -> Result<&Arc<CliNpmReqResolver>, AnyError> {
    self
      .services
      .npm_req_resolver
      .get_or_try_init_async(async {
        let npm_resolver = self.npm_resolver().await?;
        Ok(Arc::new(CliNpmReqResolver::new(NpmReqResolverOptions {
          sys: self.sys(),
          in_npm_pkg_checker: self.in_npm_pkg_checker()?.clone(),
          node_resolver: self.node_resolver().await?.clone(),
          npm_resolver: npm_resolver.clone(),
        })))
      })
      .await
  }

  pub fn pkg_json_resolver(&self) -> &Arc<CliPackageJsonResolver> {
    self
      .services
      .pkg_json_resolver
      .get_or_init(|| Arc::new(CliPackageJsonResolver::new(self.sys())))
  }

  pub async fn type_checker(&self) -> Result<&Arc<TypeChecker>, AnyError> {
    self
      .services
      .type_checker
      .get_or_try_init_async(async {
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
          self.npm_installer_if_managed()?.cloned(),
          self.npm_resolver().await?.clone(),
          self.sys(),
        )))
      })
      .await
  }

  pub async fn module_graph_builder(
    &self,
  ) -> Result<&Arc<ModuleGraphBuilder>, AnyError> {
    self
      .services
      .module_graph_builder
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(ModuleGraphBuilder::new(
          self.caches()?.clone(),
          self.cjs_tracker()?.clone(),
          cli_options.clone(),
          self.file_fetcher()?.clone(),
          self.global_http_cache()?.clone(),
          self.in_npm_pkg_checker()?.clone(),
          cli_options.maybe_lockfile().cloned(),
          self.maybe_file_watcher_reporter().clone(),
          self.module_info_cache()?.clone(),
          self.npm_graph_resolver()?.clone(),
          self.npm_installer_if_managed()?.cloned(),
          self.npm_resolver().await?.clone(),
          self.parsed_source_cache().clone(),
          self.resolver().await?.clone(),
          self.root_permissions_container()?.clone(),
          self.sys(),
        )))
      })
      .await
  }

  pub async fn module_graph_creator(
    &self,
  ) -> Result<&Arc<ModuleGraphCreator>, AnyError> {
    self
      .services
      .module_graph_creator
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(ModuleGraphCreator::new(
          cli_options.clone(),
          self.npm_installer_if_managed()?.cloned(),
          self.module_graph_builder().await?.clone(),
          self.type_checker().await?.clone(),
        )))
      })
      .await
  }

  pub async fn main_module_graph_container(
    &self,
  ) -> Result<&Arc<MainModuleGraphContainer>, AnyError> {
    self
      .services
      .main_graph_container
      .get_or_try_init_async(async {
        Ok(Arc::new(MainModuleGraphContainer::new(
          self.cli_options()?.clone(),
          self.module_load_preparer().await?.clone(),
          self.root_permissions_container()?.clone(),
        )))
      })
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
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(ModuleLoadPreparer::new(
          cli_options.clone(),
          cli_options.maybe_lockfile().cloned(),
          self.module_graph_builder().await?.clone(),
          self.text_only_progress_bar().clone(),
          self.type_checker().await?.clone(),
        )))
      })
      .await
  }

  pub fn cjs_tracker(&self) -> Result<&Arc<CliCjsTracker>, AnyError> {
    self.services.cjs_tracker.get_or_try_init(|| {
      let options = self.cli_options()?;
      Ok(Arc::new(CliCjsTracker::new(
        self.in_npm_pkg_checker()?.clone(),
        self.pkg_json_resolver().clone(),
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
          &self.cli_options()?.permissions_options(),
        )?;
        Ok(PermissionsContainer::new(desc_parser, permissions))
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
    let pkg_json_resolver = self.pkg_json_resolver().clone();
    let npm_req_resolver = self.npm_req_resolver().await?;
    let npm_registry_permission_checker = {
      let mode = if cli_options.use_byonm() {
        NpmRegistryReadPermissionCheckerMode::Byonm
      } else if let Some(node_modules_dir) = cli_options.node_modules_dir_path()
      {
        NpmRegistryReadPermissionCheckerMode::Local(node_modules_dir.clone())
      } else {
        NpmRegistryReadPermissionCheckerMode::Global(
          self.npm_cache_dir()?.root_dir().to_path_buf(),
        )
      };
      Arc::new(NpmRegistryReadPermissionChecker::new(self.sys(), mode))
    };

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
        fs.clone(),
        node_code_translator.clone(),
      ),
      npm_registry_permission_checker,
      npm_req_resolver.clone(),
      cli_npm_resolver.clone(),
      self.parsed_source_cache().clone(),
      self.resolver().await?.clone(),
      self.sys(),
    );

    let lib_main_worker_factory = LibMainWorkerFactory::new(
      self.blob_store().clone(),
      if cli_options.code_cache_enabled() {
        Some(self.code_cache()?.clone())
      } else {
        None
      },
      self.feature_checker()?.clone(),
      fs.clone(),
      self.maybe_inspector_server()?.clone(),
      Box::new(module_loader_factory),
      node_resolver.clone(),
      create_npm_process_state_provider(npm_resolver),
      pkg_json_resolver,
      self.root_cert_store_provider().clone(),
      cli_options.resolve_storage_key_resolver(),
      self.sys(),
      self.create_lib_main_worker_options()?,
    );

    Ok(CliMainWorkerFactory::new(
      lib_main_worker_factory,
      maybe_file_watcher_communicator,
      cli_options.maybe_lockfile().cloned(),
      node_resolver.clone(),
      self.npm_installer_if_managed()?.cloned(),
      npm_resolver.clone(),
      self.sys(),
      self.create_cli_main_worker_options()?,
      self.root_permissions_container()?.clone(),
    ))
  }

  fn create_lib_main_worker_options(
    &self,
  ) -> Result<LibMainWorkerOptions, AnyError> {
    let cli_options = self.cli_options()?;
    Ok(LibMainWorkerOptions {
      argv: cli_options.argv().clone(),
      // This optimization is only available for "run" subcommand
      // because we need to register new ops for testing and jupyter
      // integration.
      skip_op_registration: cli_options.sub_command().is_run(),
      log_level: cli_options.log_level().unwrap_or(log::Level::Info).into(),
      enable_op_summary_metrics: cli_options.enable_op_summary_metrics(),
      enable_testing_features: cli_options.enable_testing_features(),
      has_node_modules_dir: cli_options.has_node_modules_dir(),
      inspect_brk: cli_options.inspect_brk().is_some(),
      inspect_wait: cli_options.inspect_wait().is_some(),
      strace_ops: cli_options.strace_ops().clone(),
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
      deno_version: crate::version::DENO_VERSION_INFO.deno,
      deno_user_agent: crate::version::DENO_VERSION_INFO.user_agent,
      otel_config: self.cli_options()?.otel_config(),
      startup_snapshot: crate::js::deno_isolate_init(),
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
}

#[derive(Debug, Copy, Clone)]
pub struct SpecifierInfo {
  /// Type check as an ES module.
  pub check: bool,
  /// Type check virtual modules from doc snippets. If this is set but `check`
  /// is not, this may be a markdown file for example.
  pub check_doc: bool,
}

pub struct WorkspaceDirFilesFactory {
  specifiers: Vec<(ModuleSpecifier, SpecifierInfo)>,
  doc_snippet_specifiers: Vec<ModuleSpecifier>,
  cli_options: Arc<CliOptions>,
  cli_factory: CliFactory,
  permissions_options: Deferred<PermissionsOptions>,
}

impl WorkspaceDirFilesFactory {
  pub fn checked_specifiers(&self) -> impl Iterator<Item = &ModuleSpecifier> {
    self
      .specifiers
      .iter()
      .filter_map(|(s, i)| i.check.then_some(s))
      .chain(self.doc_snippet_specifiers.iter())
  }

  pub async fn dependent_checked_specifiers(
    &self,
    canonicalized_dep_paths: &HashSet<PathBuf>,
  ) -> Result<Vec<&ModuleSpecifier>, AnyError> {
    let graph_kind = self
      .cli_factory
      .cli_options()?
      .type_check_mode()
      .as_graph_kind();
    let module_graph_creator = self.cli_factory.module_graph_creator().await?;
    let specifiers = self.checked_specifiers().cloned().collect::<Vec<_>>();
    let graph = module_graph_creator
      .create_graph(
        graph_kind,
        specifiers.clone(),
        crate::graph_util::NpmCachingStrategy::Eager,
      )
      .await?;
    module_graph_creator.graph_valid(&graph)?;
    let dependent_specifiers = self
      .checked_specifiers()
      .filter(|s| {
        let mut dependency_specifiers = graph.walk(
          std::iter::once(*s),
          deno_graph::WalkOptions {
            follow_dynamic: true,
            kind: deno_graph::GraphKind::All,
            prefer_fast_check_graph: true,
            check_js: true,
          },
        );
        while let Some((s, _)) = dependency_specifiers.next() {
          if let Ok(path) = url_to_file_path(s) {
            if let Ok(path) = canonicalize_path(&path) {
              if canonicalized_dep_paths.contains(&path) {
                return true;
              }
            }
          } else {
            // skip walking this remote module's dependencies
            dependency_specifiers.skip_previous_dependencies();
          }
        }
        false
      })
      .collect();
    Ok(dependent_specifiers)
  }

  pub fn permissions_options(&self) -> &PermissionsOptions {
    self
      .permissions_options
      .get_or_init(|| self.cli_options.permissions_options())
  }

  pub fn permission_desc_parser(
    &self,
  ) -> Result<&Arc<RuntimePermissionDescriptorParser<CliSys>>, AnyError> {
    self.cli_factory.permission_desc_parser()
  }

  pub async fn create_cli_main_worker_factory(
    &self,
  ) -> Result<CliMainWorkerFactory, AnyError> {
    self.cli_factory.create_cli_main_worker_factory().await
  }
}

pub struct WorkspaceFilesFactory {
  dirs: Vec<WorkspaceDirFilesFactory>,
  initial_cwd: PathBuf,
}

impl WorkspaceFilesFactory {
  #[allow(clippy::type_complexity)]
  pub async fn from_workspace_dirs_with_files<T: Clone>(
    mut workspace_dirs_with_files: Vec<(Arc<WorkspaceDirectory>, FilePatterns)>,
    collect_specifiers: fn(
      FilePatterns,
      Arc<CliOptions>,
      Arc<CliFileFetcher>,
      T,
    ) -> std::pin::Pin<
      Box<
        dyn Future<
          Output = Result<Vec<(ModuleSpecifier, SpecifierInfo)>, AnyError>,
        >,
      >,
    >,
    args: T,
    extract_doc_files: Option<fn(File) -> Result<Vec<File>, AnyError>>,
    cli_options: &CliOptions,
    watcher_communicator: Option<&Arc<WatcherCommunicator>>,
  ) -> Result<Self, AnyError> {
    let initial_cwd = cli_options.initial_cwd().to_path_buf();
    if let Some(watcher_communicator) = watcher_communicator {
      let _ = watcher_communicator.watch_paths(cli_options.watch_paths());
    }
    workspace_dirs_with_files.sort_by_cached_key(|(d, _)| d.dir_url().clone());
    let all_scopes = Arc::new(
      workspace_dirs_with_files
        .iter()
        .filter(|(d, _)| d.has_deno_or_pkg_json())
        .map(|(d, _)| d.dir_url().clone())
        .collect::<BTreeSet<_>>(),
    );
    let dir_count = workspace_dirs_with_files.len();
    let mut dirs = Vec::with_capacity(dir_count);
    for (workspace_dir, files) in workspace_dirs_with_files {
      if let Some(watcher_communicator) = watcher_communicator {
        let _ = watcher_communicator.watch_paths(
          files
            .include
            .iter()
            .flat_map(|set| set.base_paths())
            .collect(),
        );
      }
      let scope_options = (dir_count > 1).then(|| ScopeOptions {
        scope: workspace_dir
          .has_deno_or_pkg_json()
          .then(|| workspace_dir.dir_url().clone()),
        all_scopes: all_scopes.clone(),
      });
      let cli_options = Arc::new(
        cli_options
          .with_new_start_dir_and_scope_options(workspace_dir, scope_options)?,
      );
      let mut factory = CliFactory::from_cli_options(cli_options.clone());
      factory.watcher_communicator = watcher_communicator.cloned();
      let file_fetcher = factory.file_fetcher()?;
      let specifiers = collect_specifiers(
        files,
        cli_options.clone(),
        file_fetcher.clone(),
        args.clone(),
      )
      .await?;
      let mut doc_snippet_specifiers = vec![];
      if let Some(extract_doc_files) = extract_doc_files {
        let root_permissions = factory.root_permissions_container()?;
        for (s, _) in specifiers.iter().filter(|(_, i)| i.check_doc) {
          let file = file_fetcher.fetch(s, root_permissions).await?;
          let snippet_files = extract_doc_files(file)?;
          for snippet_file in snippet_files {
            doc_snippet_specifiers.push(snippet_file.url.clone());
            file_fetcher.insert_memory_files(snippet_file);
          }
        }
      }
      dirs.push(WorkspaceDirFilesFactory {
        specifiers,
        doc_snippet_specifiers,
        cli_options,
        cli_factory: factory,
        permissions_options: Default::default(),
      });
    }
    Ok(Self { dirs, initial_cwd })
  }

  pub fn dirs(&self) -> &Vec<WorkspaceDirFilesFactory> {
    &self.dirs
  }

  pub fn initial_cwd(&self) -> &PathBuf {
    &self.initial_cwd
  }

  pub fn found_specifiers(&self) -> bool {
    self.dirs.iter().any(|e| !e.specifiers.is_empty())
  }

  pub async fn check(&self) -> Result<(), AnyError> {
    let mut diagnostics = vec![];
    let mut all_errors = vec![];
    for entry in &self.dirs {
      let main_graph_container = entry
        .cli_factory
        .main_module_graph_container()
        .await?
        .clone();
      let specifiers_for_typecheck =
        entry.checked_specifiers().cloned().collect::<Vec<_>>();
      if specifiers_for_typecheck.is_empty() {
        continue;
      }
      let ext_flag = entry.cli_factory.cli_options()?.ext_flag().as_ref();
      if let Err(err) = main_graph_container
        .check_specifiers(&specifiers_for_typecheck, ext_flag)
        .await
      {
        match err {
          PrepareModuleLoadError::Check(CheckError::Diagnostics(
            Diagnostics(d),
          )) => diagnostics.extend(d),
          err => all_errors.push(err),
        }
      }
    }
    if !diagnostics.is_empty() {
      all_errors.push(PrepareModuleLoadError::Check(CheckError::Diagnostics(
        Diagnostics(diagnostics),
      )));
    }
    if !all_errors.is_empty() {
      return Err(anyhow!(
        "{}",
        all_errors
          .into_iter()
          .map(|e| e.to_string())
          .collect::<Vec<_>>()
          .join("\n\n"),
      ));
    }
    Ok(())
  }
}
