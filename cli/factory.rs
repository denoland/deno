// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::check_warn_tsconfig;
use crate::args::get_root_cert_store;
use crate::args::CaData;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::NpmInstallDepsProvider;
use crate::args::StorageKeyResolver;
use crate::args::TsConfigType;
use crate::cache::Caches;
use crate::cache::CodeCache;
use crate::cache::DenoDir;
use crate::cache::DenoDirProvider;
use crate::cache::EmitCache;
use crate::cache::GlobalHttpCache;
use crate::cache::HttpCache;
use crate::cache::LocalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::file_fetcher::FileFetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_util::FileWatcherReporter;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphCreator;
use crate::http_util::HttpClientProvider;
use crate::module_loader::CliModuleLoaderFactory;
use crate::module_loader::ModuleLoadPreparer;
use crate::node::CliCjsCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::npm::create_cli_npm_resolver;
use crate::npm::CliByonmNpmResolverCreateOptions;
use crate::npm::CliNpmResolver;
use crate::npm::CliNpmResolverCreateOptions;
use crate::npm::CliNpmResolverManagedCreateOptions;
use crate::npm::CliNpmResolverManagedSnapshotOption;
use crate::resolver::CjsResolutionStore;
use crate::resolver::CliDenoResolverFs;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliGraphResolverOptions;
use crate::resolver::CliNodeResolver;
use crate::resolver::CliSloppyImportsResolver;
use crate::resolver::NpmModuleLoader;
use crate::resolver::SloppyImportsCachedFs;
use crate::standalone::DenoCompileBinaryWriter;
use crate::tools::check::TypeChecker;
use crate::tools::coverage::CoverageCollector;
use crate::tools::lint::LintRuleProvider;
use crate::tools::run::hmr::HmrRunner;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use std::path::PathBuf;

use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::WorkspaceResolver;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::FeatureChecker;

use deno_runtime::deno_fs;
use deno_runtime::deno_node::DenoFsNodeResolverEnv;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use log::warn;
use node_resolver::analyze::NodeCodeTranslator;
use once_cell::sync::OnceCell;
use std::future::Future;
use std::sync::Arc;

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
  fn get_or_try_init(&self) -> Result<&RootCertStore, AnyError> {
    self
      .cell
      .get_or_try_init(|| {
        get_root_cert_store(
          self.maybe_root_path.clone(),
          self.maybe_ca_stores.clone(),
          self.maybe_ca_data.clone(),
        )
      })
      .map_err(|e| e.into())
  }
}

struct Deferred<T>(once_cell::unsync::OnceCell<T>);

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
  cli_options: Deferred<Arc<CliOptions>>,
  caches: Deferred<Arc<Caches>>,
  file_fetcher: Deferred<Arc<FileFetcher>>,
  global_http_cache: Deferred<Arc<GlobalHttpCache>>,
  http_cache: Deferred<Arc<dyn HttpCache>>,
  http_client_provider: Deferred<Arc<HttpClientProvider>>,
  emit_cache: Deferred<Arc<EmitCache>>,
  emitter: Deferred<Arc<Emitter>>,
  fs: Deferred<Arc<dyn deno_fs::FileSystem>>,
  main_graph_container: Deferred<Arc<MainModuleGraphContainer>>,
  maybe_inspector_server: Deferred<Option<Arc<InspectorServer>>>,
  root_cert_store_provider: Deferred<Arc<dyn RootCertStoreProvider>>,
  blob_store: Deferred<Arc<BlobStore>>,
  module_info_cache: Deferred<Arc<ModuleInfoCache>>,
  parsed_source_cache: Deferred<Arc<ParsedSourceCache>>,
  resolver: Deferred<Arc<CliGraphResolver>>,
  maybe_file_watcher_reporter: Deferred<Option<FileWatcherReporter>>,
  module_graph_builder: Deferred<Arc<ModuleGraphBuilder>>,
  module_graph_creator: Deferred<Arc<ModuleGraphCreator>>,
  module_load_preparer: Deferred<Arc<ModuleLoadPreparer>>,
  node_code_translator: Deferred<Arc<CliNodeCodeTranslator>>,
  node_resolver: Deferred<Arc<NodeResolver>>,
  npm_resolver: Deferred<Arc<dyn CliNpmResolver>>,
  permission_desc_parser: Deferred<Arc<RuntimePermissionDescriptorParser>>,
  root_permissions_container: Deferred<PermissionsContainer>,
  sloppy_imports_resolver: Deferred<Option<Arc<CliSloppyImportsResolver>>>,
  text_only_progress_bar: Deferred<ProgressBar>,
  type_checker: Deferred<Arc<TypeChecker>>,
  cjs_resolutions: Deferred<Arc<CjsResolutionStore>>,
  cli_node_resolver: Deferred<Arc<CliNodeResolver>>,
  feature_checker: Deferred<Arc<FeatureChecker>>,
  code_cache: Deferred<Arc<CodeCache>>,
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
      CliOptions::from_flags(self.flags.clone()).map(Arc::new)
    })
  }

  pub fn deno_dir_provider(&self) -> Result<&Arc<DenoDirProvider>, AnyError> {
    Ok(&self.cli_options()?.deno_dir_provider)
  }

  pub fn deno_dir(&self) -> Result<&DenoDir, AnyError> {
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
        self.deno_dir()?.remote_folder_path(),
        crate::cache::RealDenoCacheEnv,
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

  pub fn file_fetcher(&self) -> Result<&Arc<FileFetcher>, AnyError> {
    self.services.file_fetcher.get_or_try_init(|| {
      let cli_options = self.cli_options()?;
      Ok(Arc::new(FileFetcher::new(
        self.http_cache()?.clone(),
        cli_options.cache_setting(),
        !cli_options.no_remote(),
        self.http_client_provider().clone(),
        self.blob_store().clone(),
        Some(self.text_only_progress_bar().clone()),
      )))
    })
  }

  pub fn fs(&self) -> &Arc<dyn deno_fs::FileSystem> {
    self.services.fs.get_or_init(|| Arc::new(deno_fs::RealFs))
  }

  pub async fn npm_resolver(
    &self,
  ) -> Result<&Arc<dyn CliNpmResolver>, AnyError> {
    self
      .services
      .npm_resolver
      .get_or_try_init_async(async {
        let fs = self.fs();
        let cli_options = self.cli_options()?;
        // For `deno install` we want to force the managed resolver so it can set up `node_modules/` directory.
        create_cli_npm_resolver(if cli_options.use_byonm() && !matches!(cli_options.sub_command(), DenoSubcommand::Install(_) | DenoSubcommand::Add(_) | DenoSubcommand::Remove(_)) {
          CliNpmResolverCreateOptions::Byonm(CliByonmNpmResolverCreateOptions {
            fs: CliDenoResolverFs(fs.clone()),
            root_node_modules_dir: Some(match cli_options.node_modules_dir_path() {
              Some(node_modules_path) => node_modules_path.to_path_buf(),
              // path needs to be canonicalized for node resolution
              // (node_modules_dir_path above is already canonicalized)
              None => canonicalize_path_maybe_not_exists(cli_options.initial_cwd())?
                .join("node_modules"),
            }),
          })
        } else {
          CliNpmResolverCreateOptions::Managed(CliNpmResolverManagedCreateOptions {
            snapshot: match cli_options.resolve_npm_resolution_snapshot()? {
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
            maybe_lockfile: cli_options.maybe_lockfile().cloned(),
            fs: fs.clone(),
            http_client_provider: self.http_client_provider().clone(),
            npm_global_cache_dir: self.deno_dir()?.npm_folder_path(),
            cache_setting: cli_options.cache_setting(),
            text_only_progress_bar: self.text_only_progress_bar().clone(),
            maybe_node_modules_path: cli_options.node_modules_dir_path().cloned(),
            npm_install_deps_provider: Arc::new(NpmInstallDepsProvider::from_workspace(cli_options.workspace())),
            npm_system_info: cli_options.npm_system_info(),
            npmrc: cli_options.npmrc().clone(),
            lifecycle_scripts: cli_options.lifecycle_scripts_config(),
          })
        }).await
      }.boxed_local())
      .await
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
            self.fs().clone(),
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
            if cli_options.use_byonm() {
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

  pub async fn resolver(&self) -> Result<&Arc<CliGraphResolver>, AnyError> {
    self
      .services
      .resolver
      .get_or_try_init_async(
        async {
          let cli_options = self.cli_options()?;
          Ok(Arc::new(CliGraphResolver::new(CliGraphResolverOptions {
            sloppy_imports_resolver: self.sloppy_imports_resolver()?.cloned(),
            node_resolver: Some(self.cli_node_resolver().await?.clone()),
            npm_resolver: if cli_options.no_npm() {
              None
            } else {
              Some(self.npm_resolver().await?.clone())
            },
            workspace_resolver: self.workspace_resolver().await?.clone(),
            bare_node_builtins_enabled: cli_options
              .unstable_bare_node_builtins(),
            maybe_jsx_import_source_config: cli_options
              .workspace()
              .to_maybe_jsx_import_source_config()?,
            maybe_vendor_dir: cli_options.vendor_dir_path(),
          })))
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

  pub async fn node_resolver(&self) -> Result<&Arc<NodeResolver>, AnyError> {
    self
      .services
      .node_resolver
      .get_or_try_init_async(
        async {
          Ok(Arc::new(NodeResolver::new(
            DenoFsNodeResolverEnv::new(self.fs().clone()),
            self.npm_resolver().await?.clone().into_npm_resolver(),
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
        let node_resolver = self.cli_node_resolver().await?.clone();
        let cjs_esm_analyzer = CliCjsCodeAnalyzer::new(
          node_analysis_cache,
          self.fs().clone(),
          node_resolver,
        );

        Ok(Arc::new(NodeCodeTranslator::new(
          cjs_esm_analyzer,
          DenoFsNodeResolverEnv::new(self.fs().clone()),
          self.node_resolver().await?.clone(),
          self.npm_resolver().await?.clone().into_npm_resolver(),
        )))
      })
      .await
  }

  pub async fn type_checker(&self) -> Result<&Arc<TypeChecker>, AnyError> {
    self
      .services
      .type_checker
      .get_or_try_init_async(async {
        let cli_options = self.cli_options()?;
        Ok(Arc::new(TypeChecker::new(
          self.caches()?.clone(),
          cli_options.clone(),
          self.module_graph_builder().await?.clone(),
          self.node_resolver().await?.clone(),
          self.npm_resolver().await?.clone(),
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
          cli_options.clone(),
          self.caches()?.clone(),
          self.fs().clone(),
          self.resolver().await?.clone(),
          self.npm_resolver().await?.clone(),
          self.module_info_cache()?.clone(),
          self.parsed_source_cache().clone(),
          cli_options.maybe_lockfile().cloned(),
          self.maybe_file_watcher_reporter().clone(),
          self.file_fetcher()?.clone(),
          self.global_http_cache()?.clone(),
          self.root_permissions_container()?.clone(),
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
          self.npm_resolver().await?.clone(),
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

  pub fn cjs_resolutions(&self) -> &Arc<CjsResolutionStore> {
    self.services.cjs_resolutions.get_or_init(Default::default)
  }

  pub async fn cli_node_resolver(
    &self,
  ) -> Result<&Arc<CliNodeResolver>, AnyError> {
    self
      .services
      .cli_node_resolver
      .get_or_try_init_async(async {
        Ok(Arc::new(CliNodeResolver::new(
          self.cjs_resolutions().clone(),
          self.fs().clone(),
          self.node_resolver().await?.clone(),
          self.npm_resolver().await?.clone(),
        )))
      })
      .await
  }

  pub fn permission_desc_parser(
    &self,
  ) -> Result<&Arc<RuntimePermissionDescriptorParser>, AnyError> {
    self.services.permission_desc_parser.get_or_try_init(|| {
      let fs = self.fs().clone();
      Ok(Arc::new(RuntimePermissionDescriptorParser::new(fs)))
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
      self.deno_dir()?,
      self.file_fetcher()?,
      self.http_client_provider(),
      self.npm_resolver().await?.as_ref(),
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
    let node_resolver = self.node_resolver().await?;
    let npm_resolver = self.npm_resolver().await?;
    let fs = self.fs();
    let cli_node_resolver = self.cli_node_resolver().await?;
    let cli_npm_resolver = self.npm_resolver().await?.clone();
    let maybe_file_watcher_communicator = if cli_options.has_hmr() {
      Some(self.watcher_communicator.clone().unwrap())
    } else {
      None
    };

    Ok(CliMainWorkerFactory::new(
      self.blob_store().clone(),
      if cli_options.code_cache_enabled() {
        Some(self.code_cache()?.clone())
      } else {
        None
      },
      self.feature_checker()?.clone(),
      self.fs().clone(),
      maybe_file_watcher_communicator,
      self.maybe_inspector_server()?.clone(),
      cli_options.maybe_lockfile().cloned(),
      Box::new(CliModuleLoaderFactory::new(
        cli_options,
        if cli_options.code_cache_enabled() {
          Some(self.code_cache()?.clone())
        } else {
          None
        },
        self.emitter()?.clone(),
        self.main_module_graph_container().await?.clone(),
        self.module_load_preparer().await?.clone(),
        cli_node_resolver.clone(),
        cli_npm_resolver.clone(),
        NpmModuleLoader::new(
          self.cjs_resolutions().clone(),
          self.node_code_translator().await?.clone(),
          fs.clone(),
          cli_node_resolver.clone(),
        ),
        self.parsed_source_cache().clone(),
        self.resolver().await?.clone(),
      )),
      node_resolver.clone(),
      npm_resolver.clone(),
      self.root_cert_store_provider().clone(),
      self.root_permissions_container()?.clone(),
      StorageKeyResolver::from_options(cli_options),
      cli_options.sub_command().clone(),
      self.create_cli_main_worker_options()?,
    ))
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
      argv: cli_options.argv().clone(),
      // This optimization is only available for "run" subcommand
      // because we need to register new ops for testing and jupyter
      // integration.
      skip_op_registration: cli_options.sub_command().is_run(),
      log_level: cli_options.log_level().unwrap_or(log::Level::Info).into(),
      enable_op_summary_metrics: cli_options.enable_op_summary_metrics(),
      enable_testing_features: cli_options.enable_testing_features(),
      has_node_modules_dir: cli_options.has_node_modules_dir(),
      hmr: cli_options.has_hmr(),
      inspect_brk: cli_options.inspect_brk().is_some(),
      inspect_wait: cli_options.inspect_wait().is_some(),
      strace_ops: cli_options.strace_ops().clone(),
      is_inspecting: cli_options.is_inspecting(),
      is_npm_main: cli_options.is_npm_main(),
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
      create_hmr_runner,
      create_coverage_collector,
      node_ipc: cli_options.node_ipc_fd(),
      serve_port: cli_options.serve_port(),
      serve_host: cli_options.serve_host(),
    })
  }
}
