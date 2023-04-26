// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::cache::Caches;
use crate::cache::DenoDir;
use crate::cache::EmitCache;
use crate::cache::HttpCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::file_fetcher::FileFetcher;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphContainer;
use crate::http_util::HttpClient;
use crate::module_loader::ModuleLoadPreparer;
use crate::node::CliCjsEsmCodeAnalyzer;
use crate::node::CliNodeCodeTranslator;
use crate::npm::create_npm_fs_resolver;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::npm::PackageJsonDepsInstaller;
use crate::resolver::CliGraphResolver;
use crate::tools::check::TypeChecker;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;

use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_node;
use deno_runtime::deno_node::analyze::NodeCodeTranslator;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use import_map::ImportMap;
use log::warn;
use std::collections::HashSet;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[derive(Clone)]
pub struct ProcState(Arc<Inner>);

pub struct Inner {
  pub dir: DenoDir,
  pub caches: Arc<Caches>,
  pub file_fetcher: Arc<FileFetcher>,
  pub http_client: HttpClient,
  pub options: Arc<CliOptions>,
  pub emit_cache: EmitCache,
  pub emitter: Arc<Emitter>,
  pub graph_container: Arc<ModuleGraphContainer>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub root_cert_store: RootCertStore,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: SharedArrayBufferStore,
  pub compiled_wasm_module_store: CompiledWasmModuleStore,
  pub parsed_source_cache: Arc<ParsedSourceCache>,
  pub resolver: Arc<CliGraphResolver>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  pub module_graph_builder: Arc<ModuleGraphBuilder>,
  pub module_load_preparer: Arc<ModuleLoadPreparer>,
  pub node_code_translator: Arc<CliNodeCodeTranslator>,
  pub node_fs: Arc<dyn deno_node::NodeFs>,
  pub node_resolver: Arc<NodeResolver>,
  pub npm_api: Arc<CliNpmRegistryApi>,
  pub npm_cache: Arc<NpmCache>,
  pub npm_resolver: Arc<CliNpmResolver>,
  pub npm_resolution: Arc<NpmResolution>,
  pub package_json_deps_installer: Arc<PackageJsonDepsInstaller>,
  pub cjs_resolutions: Arc<CjsResolutionStore>,
  progress_bar: ProgressBar,
}

impl Deref for ProcState {
  type Target = Arc<Inner>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ProcState {
  pub async fn from_cli_options(
    options: Arc<CliOptions>,
  ) -> Result<Self, AnyError> {
    Self::build_with_sender(options, None).await
  }

  pub async fn from_flags(flags: Flags) -> Result<Self, AnyError> {
    Self::from_cli_options(Arc::new(CliOptions::from_flags(flags)?)).await
  }

  pub async fn from_flags_for_file_watcher(
    flags: Flags,
    files_to_watch_sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  ) -> Result<Self, AnyError> {
    // resolve the config each time
    let cli_options = Arc::new(CliOptions::from_flags(flags)?);
    let ps =
      Self::build_with_sender(cli_options, Some(files_to_watch_sender.clone()))
        .await?;
    ps.init_watcher();
    Ok(ps)
  }

  /// Reset all runtime state to its default. This should be used on file
  /// watcher restarts.
  pub fn reset_for_file_watcher(&mut self) {
    self.cjs_resolutions.clear();
    self.parsed_source_cache.clear();
    self.graph_container.clear();

    self.0 = Arc::new(Inner {
      dir: self.dir.clone(),
      caches: self.caches.clone(),
      options: self.options.clone(),
      emit_cache: self.emit_cache.clone(),
      emitter: self.emitter.clone(),
      file_fetcher: self.file_fetcher.clone(),
      http_client: self.http_client.clone(),
      graph_container: self.graph_container.clone(),
      lockfile: self.lockfile.clone(),
      maybe_import_map: self.maybe_import_map.clone(),
      maybe_inspector_server: self.maybe_inspector_server.clone(),
      root_cert_store: self.root_cert_store.clone(),
      blob_store: self.blob_store.clone(),
      broadcast_channel: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      parsed_source_cache: self.parsed_source_cache.clone(),
      resolver: self.resolver.clone(),
      maybe_file_watcher_reporter: self.maybe_file_watcher_reporter.clone(),
      module_graph_builder: self.module_graph_builder.clone(),
      module_load_preparer: self.module_load_preparer.clone(),
      node_code_translator: self.node_code_translator.clone(),
      node_fs: self.node_fs.clone(),
      node_resolver: self.node_resolver.clone(),
      npm_api: self.npm_api.clone(),
      npm_cache: self.npm_cache.clone(),
      npm_resolver: self.npm_resolver.clone(),
      npm_resolution: self.npm_resolution.clone(),
      package_json_deps_installer: self.package_json_deps_installer.clone(),
      cjs_resolutions: self.cjs_resolutions.clone(),
      progress_bar: self.progress_bar.clone(),
    });
    self.init_watcher();
  }

  // Add invariant files like the import map and explicit watch flag list to
  // the watcher. Dedup for build_for_file_watcher and reset_for_file_watcher.
  fn init_watcher(&self) {
    let files_to_watch_sender = match &self.0.maybe_file_watcher_reporter {
      Some(reporter) => &reporter.sender,
      None => return,
    };
    if let Some(watch_paths) = self.options.watch_paths() {
      files_to_watch_sender.send(watch_paths.clone()).unwrap();
    }
    if let Ok(Some(import_map_path)) = self
      .options
      .resolve_import_map_specifier()
      .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
    {
      files_to_watch_sender.send(vec![import_map_path]).unwrap();
    }
  }

  async fn build_with_sender(
    cli_options: Arc<CliOptions>,
    maybe_sender: Option<tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>>,
  ) -> Result<Self, AnyError> {
    let dir = cli_options.resolve_deno_dir()?;
    let caches = Arc::new(Caches::default());
    // Warm up the caches we know we'll likely need based on the CLI mode
    match cli_options.sub_command() {
      DenoSubcommand::Run(_) => {
        _ = caches.dep_analysis_db(&dir);
        _ = caches.node_analysis_db(&dir);
      }
      DenoSubcommand::Check(_) => {
        _ = caches.dep_analysis_db(&dir);
        _ = caches.node_analysis_db(&dir);
        _ = caches.type_checking_cache_db(&dir);
      }
      _ => {}
    }
    let blob_store = BlobStore::default();
    let broadcast_channel = InMemoryBroadcastChannel::default();
    let shared_array_buffer_store = SharedArrayBufferStore::default();
    let compiled_wasm_module_store = CompiledWasmModuleStore::default();
    let deps_cache_location = dir.deps_folder_path();
    let http_cache = HttpCache::new(&deps_cache_location);
    let root_cert_store = cli_options.resolve_root_cert_store()?;
    let cache_usage = cli_options.cache_setting();
    let progress_bar = ProgressBar::new(ProgressBarStyle::TextOnly);
    let http_client = HttpClient::new(
      Some(root_cert_store.clone()),
      cli_options.unsafely_ignore_certificate_errors().clone(),
    )?;
    let file_fetcher = FileFetcher::new(
      http_cache,
      cache_usage,
      !cli_options.no_remote(),
      http_client.clone(),
      blob_store.clone(),
      Some(progress_bar.clone()),
    );

    let lockfile = cli_options.maybe_lock_file();

    let npm_registry_url = CliNpmRegistryApi::default_url().to_owned();
    let npm_cache = Arc::new(NpmCache::from_deno_dir(
      &dir,
      cli_options.cache_setting(),
      http_client.clone(),
      progress_bar.clone(),
    ));
    let npm_api = Arc::new(CliNpmRegistryApi::new(
      npm_registry_url.clone(),
      npm_cache.clone(),
      http_client.clone(),
      progress_bar.clone(),
    ));
    let npm_snapshot = cli_options
      .resolve_npm_resolution_snapshot(&npm_api)
      .await?;
    let npm_resolution = Arc::new(NpmResolution::from_serialized(
      npm_api.clone(),
      npm_snapshot,
      lockfile.as_ref().cloned(),
    ));
    let node_fs = Arc::new(deno_node::RealFs);
    let npm_fs_resolver = create_npm_fs_resolver(
      node_fs.clone(),
      npm_cache,
      &progress_bar,
      npm_registry_url,
      npm_resolution.clone(),
      cli_options.node_modules_dir_path(),
    );
    let npm_resolver = Arc::new(CliNpmResolver::new(
      npm_resolution.clone(),
      npm_fs_resolver,
      lockfile.as_ref().cloned(),
    ));
    let package_json_deps_installer = Arc::new(PackageJsonDepsInstaller::new(
      npm_api.clone(),
      npm_resolution.clone(),
      cli_options.maybe_package_json_deps(),
    ));
    let maybe_import_map = cli_options
      .resolve_import_map(&file_fetcher)
      .await?
      .map(Arc::new);
    let maybe_inspector_server =
      cli_options.resolve_inspector_server().map(Arc::new);

    let resolver = Arc::new(CliGraphResolver::new(
      cli_options.to_maybe_jsx_import_source_config(),
      maybe_import_map.clone(),
      cli_options.no_npm(),
      npm_api.clone(),
      npm_resolution.clone(),
      package_json_deps_installer.clone(),
    ));

    let maybe_file_watcher_reporter =
      maybe_sender.map(|sender| FileWatcherReporter {
        sender,
        file_paths: Arc::new(Mutex::new(vec![])),
      });

    let ts_config_result =
      cli_options.resolve_ts_config_for_emit(TsConfigType::Emit)?;
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      warn!("{}", ignored_options);
    }
    let emit_cache = EmitCache::new(dir.gen_cache.clone());
    let parsed_source_cache =
      Arc::new(ParsedSourceCache::new(caches.dep_analysis_db(&dir)));
    let emit_options: deno_ast::EmitOptions = ts_config_result.ts_config.into();
    let emitter = Arc::new(Emitter::new(
      emit_cache.clone(),
      parsed_source_cache.clone(),
      emit_options,
    ));
    let npm_cache = Arc::new(NpmCache::from_deno_dir(
      &dir,
      cli_options.cache_setting(),
      http_client.clone(),
      progress_bar.clone(),
    ));
    let file_fetcher = Arc::new(file_fetcher);
    let node_analysis_cache =
      NodeAnalysisCache::new(caches.node_analysis_db(&dir));
    let cjs_esm_analyzer = CliCjsEsmCodeAnalyzer::new(node_analysis_cache);
    let node_resolver =
      Arc::new(NodeResolver::new(node_fs.clone(), npm_resolver.clone()));
    let node_code_translator = Arc::new(NodeCodeTranslator::new(
      cjs_esm_analyzer,
      node_fs.clone(),
      node_resolver.clone(),
      npm_resolver.clone(),
    ));
    let type_checker = Arc::new(TypeChecker::new(
      dir.clone(),
      caches.clone(),
      cli_options.clone(),
      node_resolver.clone(),
      npm_resolver.clone(),
    ));
    let module_graph_builder = Arc::new(ModuleGraphBuilder::new(
      cli_options.clone(),
      resolver.clone(),
      npm_resolver.clone(),
      parsed_source_cache.clone(),
      lockfile.clone(),
      emit_cache.clone(),
      file_fetcher.clone(),
      type_checker.clone(),
    ));
    let graph_container: Arc<ModuleGraphContainer> = Default::default();
    let module_load_preparer = Arc::new(ModuleLoadPreparer::new(
      cli_options.clone(),
      graph_container.clone(),
      lockfile.clone(),
      maybe_file_watcher_reporter.clone(),
      module_graph_builder.clone(),
      parsed_source_cache.clone(),
      progress_bar.clone(),
      resolver.clone(),
      type_checker,
    ));

    Ok(ProcState(Arc::new(Inner {
      dir,
      caches,
      options: cli_options,
      emit_cache,
      emitter,
      file_fetcher,
      http_client,
      graph_container,
      lockfile,
      maybe_import_map,
      maybe_inspector_server,
      root_cert_store,
      blob_store,
      broadcast_channel,
      shared_array_buffer_store,
      compiled_wasm_module_store,
      parsed_source_cache,
      resolver,
      maybe_file_watcher_reporter,
      module_graph_builder,
      node_code_translator,
      node_fs,
      node_resolver,
      npm_api,
      npm_cache,
      npm_resolver,
      npm_resolution,
      package_json_deps_installer,
      cjs_resolutions: Default::default(),
      module_load_preparer,
      progress_bar,
    })))
  }
}

/// Keeps track of what module specifiers were resolved as CJS.
#[derive(Default)]
pub struct CjsResolutionStore(Mutex<HashSet<ModuleSpecifier>>);

impl CjsResolutionStore {
  pub fn clear(&self) {
    self.0.lock().clear();
  }

  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().contains(specifier)
  }

  pub fn insert(&self, specifier: ModuleSpecifier) {
    self.0.lock().insert(specifier);
  }
}

#[derive(Clone, Debug)]
pub struct FileWatcherReporter {
  sender: tokio::sync::mpsc::UnboundedSender<Vec<PathBuf>>,
  file_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl deno_graph::source::Reporter for FileWatcherReporter {
  fn on_load(
    &self,
    specifier: &ModuleSpecifier,
    modules_done: usize,
    modules_total: usize,
  ) {
    let mut file_paths = self.file_paths.lock();
    if specifier.scheme() == "file" {
      file_paths.push(specifier.to_file_path().unwrap());
    }

    if modules_done == modules_total {
      self.sender.send(file_paths.drain(..).collect()).unwrap();
    }
  }
}
