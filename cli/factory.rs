// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::npm_pkg_req_ref_to_binary_command;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::Lockfile;
use crate::args::PackageJsonDepsProvider;
use crate::args::StorageKeyResolver;
use crate::args::TsConfigType;
use crate::cache::Caches;
use crate::cache::DenoDir;
use crate::cache::DenoDirProvider;
use crate::cache::EmitCache;
use crate::cache::HttpCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::file_fetcher::FileFetcher;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphContainer;
use crate::http_util::HttpClient;
use crate::module_loader::CjsResolutionStore;
use crate::module_loader::CliModuleLoaderFactory;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::NpmModuleLoader;
use crate::node::CliCjsEsmCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::npm::create_npm_fs_resolver;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmPackageFsResolver;
use crate::npm::NpmResolution;
use crate::npm::PackageJsonDepsInstaller;
use crate::resolver::CliGraphResolver;
use crate::standalone::DenoCompileBinaryWriter;
use crate::tools::check::TypeChecker;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::watcher::FileWatcher;
use crate::watcher::FileWatcherReporter;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CliMainWorkerOptions;
use crate::worker::HasNodeSpecifierChecker;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;

use deno_runtime::deno_fs;
use deno_runtime::deno_node::analyze::NodeCodeTranslator;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_semver::npm::NpmPackageReqReference;
use import_map::ImportMap;
use log::warn;
use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CliFactoryBuilder {
  maybe_sender: Option<tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>>,
}

impl CliFactoryBuilder {
  pub fn new() -> Self {
    Self { maybe_sender: None }
  }

  pub fn with_watcher(
    mut self,
    sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  ) -> Self {
    self.maybe_sender = Some(sender);
    self
  }

  pub async fn build_from_flags(
    self,
    flags: Flags,
  ) -> Result<CliFactory, AnyError> {
    Ok(self.build_from_cli_options(Arc::new(CliOptions::from_flags(flags)?)))
  }

  pub fn build_from_cli_options(self, options: Arc<CliOptions>) -> CliFactory {
    CliFactory {
      maybe_sender: RefCell::new(self.maybe_sender),
      options,
      services: Default::default(),
    }
  }
}

struct Deferred<T>(once_cell::unsync::OnceCell<T>);

impl<T> Default for Deferred<T> {
  fn default() -> Self {
    Self(once_cell::unsync::OnceCell::default())
  }
}

impl<T> Deferred<T> {
  pub fn get_or_try_init(
    &self,
    create: impl FnOnce() -> Result<T, AnyError>,
  ) -> Result<&T, AnyError> {
    self.0.get_or_try_init(create)
  }

  pub fn get_or_init(&self, create: impl FnOnce() -> T) -> &T {
    self.0.get_or_init(create)
  }

  pub async fn get_or_try_init_async(
    &self,
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
  deno_dir_provider: Deferred<Arc<DenoDirProvider>>,
  caches: Deferred<Arc<Caches>>,
  file_fetcher: Deferred<Arc<FileFetcher>>,
  http_client: Deferred<Arc<HttpClient>>,
  emit_cache: Deferred<EmitCache>,
  emitter: Deferred<Arc<Emitter>>,
  fs: Deferred<Arc<dyn deno_fs::FileSystem>>,
  graph_container: Deferred<Arc<ModuleGraphContainer>>,
  lockfile: Deferred<Option<Arc<Mutex<Lockfile>>>>,
  maybe_import_map: Deferred<Option<Arc<ImportMap>>>,
  maybe_inspector_server: Deferred<Option<Arc<InspectorServer>>>,
  root_cert_store_provider: Deferred<Arc<dyn RootCertStoreProvider>>,
  blob_store: Deferred<BlobStore>,
  parsed_source_cache: Deferred<Arc<ParsedSourceCache>>,
  resolver: Deferred<Arc<CliGraphResolver>>,
  file_watcher: Deferred<Arc<FileWatcher>>,
  maybe_file_watcher_reporter: Deferred<Option<FileWatcherReporter>>,
  module_graph_builder: Deferred<Arc<ModuleGraphBuilder>>,
  module_load_preparer: Deferred<Arc<ModuleLoadPreparer>>,
  node_code_translator: Deferred<Arc<CliNodeCodeTranslator>>,
  node_resolver: Deferred<Arc<NodeResolver>>,
  npm_api: Deferred<Arc<CliNpmRegistryApi>>,
  npm_cache: Deferred<Arc<NpmCache>>,
  npm_resolver: Deferred<Arc<CliNpmResolver>>,
  npm_resolution: Deferred<Arc<NpmResolution>>,
  package_json_deps_provider: Deferred<Arc<PackageJsonDepsProvider>>,
  package_json_deps_installer: Deferred<Arc<PackageJsonDepsInstaller>>,
  text_only_progress_bar: Deferred<ProgressBar>,
  type_checker: Deferred<Arc<TypeChecker>>,
  cjs_resolutions: Deferred<Arc<CjsResolutionStore>>,
}

pub struct CliFactory {
  maybe_sender:
    RefCell<Option<tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>>>,
  options: Arc<CliOptions>,
  services: CliFactoryServices,
}

impl CliFactory {
  pub async fn from_flags(flags: Flags) -> Result<Self, AnyError> {
    CliFactoryBuilder::new().build_from_flags(flags).await
  }

  pub fn from_cli_options(options: Arc<CliOptions>) -> Self {
    CliFactoryBuilder::new().build_from_cli_options(options)
  }

  pub fn cli_options(&self) -> &Arc<CliOptions> {
    &self.options
  }

  pub fn deno_dir_provider(&self) -> &Arc<DenoDirProvider> {
    self.services.deno_dir_provider.get_or_init(|| {
      Arc::new(DenoDirProvider::new(
        self.options.maybe_custom_root().clone(),
      ))
    })
  }

  pub fn deno_dir(&self) -> Result<&DenoDir, AnyError> {
    Ok(self.deno_dir_provider().get_or_create()?)
  }

  pub fn caches(&self) -> Result<&Arc<Caches>, AnyError> {
    self.services.caches.get_or_try_init(|| {
      let caches = Arc::new(Caches::new(self.deno_dir_provider().clone()));
      // Warm up the caches we know we'll likely need based on the CLI mode
      match self.options.sub_command() {
        DenoSubcommand::Run(_) => {
          _ = caches.dep_analysis_db();
          _ = caches.node_analysis_db();
        }
        DenoSubcommand::Check(_) => {
          _ = caches.dep_analysis_db();
          _ = caches.node_analysis_db();
          _ = caches.type_checking_cache_db();
        }
        _ => {}
      }
      Ok(caches)
    })
  }

  pub fn blob_store(&self) -> &BlobStore {
    self.services.blob_store.get_or_init(BlobStore::default)
  }

  pub fn root_cert_store_provider(&self) -> &Arc<dyn RootCertStoreProvider> {
    self
      .services
      .root_cert_store_provider
      .get_or_init(|| self.options.resolve_root_cert_store_provider())
  }

  pub fn text_only_progress_bar(&self) -> &ProgressBar {
    self
      .services
      .text_only_progress_bar
      .get_or_init(|| ProgressBar::new(ProgressBarStyle::TextOnly))
  }

  pub fn http_client(&self) -> &Arc<HttpClient> {
    self.services.http_client.get_or_init(|| {
      Arc::new(HttpClient::new(
        Some(self.root_cert_store_provider().clone()),
        self.options.unsafely_ignore_certificate_errors().clone(),
      ))
    })
  }

  pub fn file_fetcher(&self) -> Result<&Arc<FileFetcher>, AnyError> {
    self.services.file_fetcher.get_or_try_init(|| {
      Ok(Arc::new(FileFetcher::new(
        HttpCache::new(&self.deno_dir()?.deps_folder_path()),
        self.options.cache_setting(),
        !self.options.no_remote(),
        self.http_client().clone(),
        self.blob_store().clone(),
        Some(self.text_only_progress_bar().clone()),
      )))
    })
  }

  pub fn fs(&self) -> &Arc<dyn deno_fs::FileSystem> {
    self.services.fs.get_or_init(|| Arc::new(deno_fs::RealFs))
  }

  pub fn maybe_lockfile(&self) -> &Option<Arc<Mutex<Lockfile>>> {
    self
      .services
      .lockfile
      .get_or_init(|| self.options.maybe_lockfile())
  }

  pub fn npm_cache(&self) -> Result<&Arc<NpmCache>, AnyError> {
    self.services.npm_cache.get_or_try_init(|| {
      Ok(Arc::new(NpmCache::new(
        self.deno_dir()?.npm_folder_path(),
        self.options.cache_setting(),
        self.http_client().clone(),
        self.text_only_progress_bar().clone(),
      )))
    })
  }

  pub fn npm_api(&self) -> Result<&Arc<CliNpmRegistryApi>, AnyError> {
    self.services.npm_api.get_or_try_init(|| {
      Ok(Arc::new(CliNpmRegistryApi::new(
        CliNpmRegistryApi::default_url().to_owned(),
        self.npm_cache()?.clone(),
        self.http_client().clone(),
        self.text_only_progress_bar().clone(),
      )))
    })
  }

  pub async fn npm_resolution(&self) -> Result<&Arc<NpmResolution>, AnyError> {
    self
      .services
      .npm_resolution
      .get_or_try_init_async(async {
        let npm_api = self.npm_api()?;
        Ok(Arc::new(NpmResolution::from_serialized(
          npm_api.clone(),
          self
            .options
            .resolve_npm_resolution_snapshot(npm_api)
            .await?,
          self.maybe_lockfile().as_ref().cloned(),
        )))
      })
      .await
  }

  pub async fn npm_resolver(&self) -> Result<&Arc<CliNpmResolver>, AnyError> {
    self
      .services
      .npm_resolver
      .get_or_try_init_async(async {
        let npm_resolution = self.npm_resolution().await?;
        let fs = self.fs().clone();
        let npm_fs_resolver = create_npm_fs_resolver(
          fs.clone(),
          self.npm_cache()?.clone(),
          self.text_only_progress_bar(),
          CliNpmRegistryApi::default_url().to_owned(),
          npm_resolution.clone(),
          self.options.node_modules_dir_path(),
          self.options.npm_system_info(),
        );
        Ok(Arc::new(CliNpmResolver::new(
          fs.clone(),
          npm_resolution.clone(),
          npm_fs_resolver,
          self.maybe_lockfile().as_ref().cloned(),
        )))
      })
      .await
  }

  pub async fn create_node_modules_npm_fs_resolver(
    &self,
    node_modules_dir_path: PathBuf,
  ) -> Result<Arc<dyn NpmPackageFsResolver>, AnyError> {
    Ok(create_npm_fs_resolver(
      self.fs().clone(),
      self.npm_cache()?.clone(),
      self.text_only_progress_bar(),
      CliNpmRegistryApi::default_url().to_owned(),
      self.npm_resolution().await?.clone(),
      // when an explicit path is provided here, it will create the
      // local node_modules variant of an npm fs resolver
      Some(node_modules_dir_path),
      self.options.npm_system_info(),
    ))
  }

  pub fn package_json_deps_provider(&self) -> &Arc<PackageJsonDepsProvider> {
    self.services.package_json_deps_provider.get_or_init(|| {
      Arc::new(PackageJsonDepsProvider::new(
        self.options.maybe_package_json_deps(),
      ))
    })
  }

  pub async fn package_json_deps_installer(
    &self,
  ) -> Result<&Arc<PackageJsonDepsInstaller>, AnyError> {
    self
      .services
      .package_json_deps_installer
      .get_or_try_init_async(async {
        Ok(Arc::new(PackageJsonDepsInstaller::new(
          self.package_json_deps_provider().clone(),
          self.npm_api()?.clone(),
          self.npm_resolution().await?.clone(),
        )))
      })
      .await
  }

  pub async fn maybe_import_map(
    &self,
  ) -> Result<&Option<Arc<ImportMap>>, AnyError> {
    self
      .services
      .maybe_import_map
      .get_or_try_init_async(async {
        Ok(
          self
            .options
            .resolve_import_map(self.file_fetcher()?)
            .await?
            .map(Arc::new),
        )
      })
      .await
  }

  pub async fn resolver(&self) -> Result<&Arc<CliGraphResolver>, AnyError> {
    self
      .services
      .resolver
      .get_or_try_init_async(async {
        Ok(Arc::new(CliGraphResolver::new(
          self.options.to_maybe_jsx_import_source_config(),
          self.maybe_import_map().await?.clone(),
          self.options.no_npm(),
          self.npm_api()?.clone(),
          self.npm_resolution().await?.clone(),
          self.package_json_deps_provider().clone(),
          self.package_json_deps_installer().await?.clone(),
        )))
      })
      .await
  }

  pub fn file_watcher(&self) -> Result<&Arc<FileWatcher>, AnyError> {
    self.services.file_watcher.get_or_try_init(|| {
      let watcher = FileWatcher::new(
        self.options.clone(),
        self.cjs_resolutions().clone(),
        self.graph_container().clone(),
        self.maybe_file_watcher_reporter().clone(),
        self.parsed_source_cache()?.clone(),
      );
      watcher.init_watcher();
      Ok(Arc::new(watcher))
    })
  }

  pub fn maybe_file_watcher_reporter(&self) -> &Option<FileWatcherReporter> {
    let maybe_sender = self.maybe_sender.borrow_mut().take();
    self
      .services
      .maybe_file_watcher_reporter
      .get_or_init(|| maybe_sender.map(FileWatcherReporter::new))
  }

  pub fn emit_cache(&self) -> Result<&EmitCache, AnyError> {
    self.services.emit_cache.get_or_try_init(|| {
      Ok(EmitCache::new(self.deno_dir()?.gen_cache.clone()))
    })
  }

  pub fn parsed_source_cache(
    &self,
  ) -> Result<&Arc<ParsedSourceCache>, AnyError> {
    self.services.parsed_source_cache.get_or_try_init(|| {
      Ok(Arc::new(ParsedSourceCache::new(
        self.caches()?.dep_analysis_db(),
      )))
    })
  }

  pub fn emitter(&self) -> Result<&Arc<Emitter>, AnyError> {
    self.services.emitter.get_or_try_init(|| {
      let ts_config_result = self
        .options
        .resolve_ts_config_for_emit(TsConfigType::Emit)?;
      if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
        warn!("{}", ignored_options);
      }
      let emit_options: deno_ast::EmitOptions =
        ts_config_result.ts_config.into();
      Ok(Arc::new(Emitter::new(
        self.emit_cache()?.clone(),
        self.parsed_source_cache()?.clone(),
        emit_options,
      )))
    })
  }

  pub async fn node_resolver(&self) -> Result<&Arc<NodeResolver>, AnyError> {
    self
      .services
      .node_resolver
      .get_or_try_init_async(async {
        Ok(Arc::new(NodeResolver::new(
          self.fs().clone(),
          self.npm_resolver().await?.clone(),
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
      .get_or_try_init_async(async {
        let caches = self.caches()?;
        let node_analysis_cache =
          NodeAnalysisCache::new(caches.node_analysis_db());
        let cjs_esm_analyzer = CliCjsEsmCodeAnalyzer::new(node_analysis_cache);

        Ok(Arc::new(NodeCodeTranslator::new(
          cjs_esm_analyzer,
          self.fs().clone(),
          self.node_resolver().await?.clone(),
          self.npm_resolver().await?.clone(),
        )))
      })
      .await
  }

  pub async fn type_checker(&self) -> Result<&Arc<TypeChecker>, AnyError> {
    self
      .services
      .type_checker
      .get_or_try_init_async(async {
        Ok(Arc::new(TypeChecker::new(
          self.caches()?.clone(),
          self.options.clone(),
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
        Ok(Arc::new(ModuleGraphBuilder::new(
          self.options.clone(),
          self.resolver().await?.clone(),
          self.npm_resolver().await?.clone(),
          self.parsed_source_cache()?.clone(),
          self.maybe_lockfile().clone(),
          self.emit_cache()?.clone(),
          self.file_fetcher()?.clone(),
          self.type_checker().await?.clone(),
        )))
      })
      .await
  }

  pub fn graph_container(&self) -> &Arc<ModuleGraphContainer> {
    self.services.graph_container.get_or_init(Default::default)
  }

  pub fn maybe_inspector_server(&self) -> &Option<Arc<InspectorServer>> {
    self
      .services
      .maybe_inspector_server
      .get_or_init(|| self.options.resolve_inspector_server().map(Arc::new))
  }

  pub async fn module_load_preparer(
    &self,
  ) -> Result<&Arc<ModuleLoadPreparer>, AnyError> {
    self
      .services
      .module_load_preparer
      .get_or_try_init_async(async {
        Ok(Arc::new(ModuleLoadPreparer::new(
          self.options.clone(),
          self.graph_container().clone(),
          self.maybe_lockfile().clone(),
          self.maybe_file_watcher_reporter().clone(),
          self.module_graph_builder().await?.clone(),
          self.parsed_source_cache()?.clone(),
          self.text_only_progress_bar().clone(),
          self.resolver().await?.clone(),
          self.type_checker().await?.clone(),
        )))
      })
      .await
  }

  pub fn cjs_resolutions(&self) -> &Arc<CjsResolutionStore> {
    self.services.cjs_resolutions.get_or_init(Default::default)
  }

  pub async fn create_compile_binary_writer(
    &self,
  ) -> Result<DenoCompileBinaryWriter, AnyError> {
    Ok(DenoCompileBinaryWriter::new(
      self.file_fetcher()?,
      self.http_client(),
      self.deno_dir()?,
      self.npm_api()?,
      self.npm_cache()?,
      self.npm_resolution().await?,
      self.npm_resolver().await?,
      self.options.npm_system_info(),
      self.package_json_deps_provider(),
    ))
  }

  /// Gets a function that can be used to create a CliMainWorkerFactory
  /// for a file watcher.
  pub async fn create_cli_main_worker_factory_func(
    &self,
  ) -> Result<Arc<dyn Fn() -> CliMainWorkerFactory>, AnyError> {
    let emitter = self.emitter()?.clone();
    let graph_container = self.graph_container().clone();
    let module_load_preparer = self.module_load_preparer().await?.clone();
    let parsed_source_cache = self.parsed_source_cache()?.clone();
    let resolver = self.resolver().await?.clone();
    let blob_store = self.blob_store().clone();
    let cjs_resolutions = self.cjs_resolutions().clone();
    let node_code_translator = self.node_code_translator().await?.clone();
    let options = self.cli_options().clone();
    let main_worker_options = self.create_cli_main_worker_options()?;
    let fs = self.fs().clone();
    let root_cert_store_provider = self.root_cert_store_provider().clone();
    let node_resolver = self.node_resolver().await?.clone();
    let npm_resolver = self.npm_resolver().await?.clone();
    let maybe_inspector_server = self.maybe_inspector_server().clone();
    let maybe_lockfile = self.maybe_lockfile().clone();
    Ok(Arc::new(move || {
      CliMainWorkerFactory::new(
        StorageKeyResolver::from_options(&options),
        npm_resolver.clone(),
        node_resolver.clone(),
        Box::new(CliHasNodeSpecifierChecker(graph_container.clone())),
        blob_store.clone(),
        Box::new(CliModuleLoaderFactory::new(
          &options,
          emitter.clone(),
          graph_container.clone(),
          module_load_preparer.clone(),
          parsed_source_cache.clone(),
          resolver.clone(),
          NpmModuleLoader::new(
            cjs_resolutions.clone(),
            node_code_translator.clone(),
            fs.clone(),
            node_resolver.clone(),
          ),
        )),
        root_cert_store_provider.clone(),
        fs.clone(),
        maybe_inspector_server.clone(),
        maybe_lockfile.clone(),
        main_worker_options.clone(),
      )
    }))
  }

  pub async fn create_cli_main_worker_factory(
    &self,
  ) -> Result<CliMainWorkerFactory, AnyError> {
    let node_resolver = self.node_resolver().await?;
    let fs = self.fs();
    Ok(CliMainWorkerFactory::new(
      StorageKeyResolver::from_options(&self.options),
      self.npm_resolver().await?.clone(),
      node_resolver.clone(),
      Box::new(CliHasNodeSpecifierChecker(self.graph_container().clone())),
      self.blob_store().clone(),
      Box::new(CliModuleLoaderFactory::new(
        &self.options,
        self.emitter()?.clone(),
        self.graph_container().clone(),
        self.module_load_preparer().await?.clone(),
        self.parsed_source_cache()?.clone(),
        self.resolver().await?.clone(),
        NpmModuleLoader::new(
          self.cjs_resolutions().clone(),
          self.node_code_translator().await?.clone(),
          fs.clone(),
          node_resolver.clone(),
        ),
      )),
      self.root_cert_store_provider().clone(),
      self.fs().clone(),
      self.maybe_inspector_server().clone(),
      self.maybe_lockfile().clone(),
      self.create_cli_main_worker_options()?,
    ))
  }

  fn create_cli_main_worker_options(
    &self,
  ) -> Result<CliMainWorkerOptions, AnyError> {
    Ok(CliMainWorkerOptions {
      argv: self.options.argv().clone(),
      log_level: self.options.log_level().unwrap_or(log::Level::Info).into(),
      coverage_dir: self.options.coverage_dir(),
      enable_testing_features: self.options.enable_testing_features(),
      has_node_modules_dir: self.options.has_node_modules_dir(),
      inspect_brk: self.options.inspect_brk().is_some(),
      inspect_wait: self.options.inspect_wait().is_some(),
      is_inspecting: self.options.is_inspecting(),
      is_npm_main: self.options.is_npm_main(),
      location: self.options.location_flag().clone(),
      maybe_binary_npm_command_name: {
        let mut maybe_binary_command_name = None;
        if let DenoSubcommand::Run(flags) = self.options.sub_command() {
          if let Ok(pkg_ref) = NpmPackageReqReference::from_str(&flags.script) {
            // if the user ran a binary command, we'll need to set process.argv[0]
            // to be the name of the binary command instead of deno
            maybe_binary_command_name =
              Some(npm_pkg_req_ref_to_binary_command(&pkg_ref));
          }
        }
        maybe_binary_command_name
      },
      origin_data_folder_path: Some(self.deno_dir()?.origin_data_folder_path()),
      seed: self.options.seed(),
      unsafely_ignore_certificate_errors: self
        .options
        .unsafely_ignore_certificate_errors()
        .clone(),
      unstable: self.options.unstable(),
      startup_snapshot: self.options.snapshot_path().clone(),
    })
  }
}

struct CliHasNodeSpecifierChecker(Arc<ModuleGraphContainer>);

impl HasNodeSpecifierChecker for CliHasNodeSpecifierChecker {
  fn has_node_specifier(&self) -> bool {
    self.0.graph().has_node_specifier
  }
}
