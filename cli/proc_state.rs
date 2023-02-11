// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::Lockfile;
use crate::args::TsConfigType;
use crate::args::TsTypeLib;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::DenoDir;
use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::HttpCache;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::cache::TypeCheckCache;
use crate::emit::emit_parsed_source;
use crate::file_fetcher::FileFetcher;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::graph_valid_with_cli_options;
use crate::http_util::HttpClient;
use crate::node;
use crate::node::NodeResolution;
use crate::npm::resolve_graph_npm_info;
use crate::npm::NpmCache;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;
use crate::npm::RealNpmRegistryApi;
use crate::resolver::CliResolver;
use crate::tools::check;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;
use deno_graph::source::Loader;
use deno_graph::source::Resolver;
use deno_graph::ModuleGraph;
use deno_graph::ModuleKind;
use deno_graph::Resolution;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMap;
use log::warn;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[derive(Clone)]
pub struct ProcState(Arc<Inner>);

pub struct Inner {
  pub dir: DenoDir,
  pub file_fetcher: Arc<FileFetcher>,
  pub http_client: HttpClient,
  pub options: Arc<CliOptions>,
  pub emit_cache: EmitCache,
  pub emit_options: deno_ast::EmitOptions,
  pub emit_options_hash: u64,
  graph_data: Arc<RwLock<GraphData>>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub root_cert_store: RootCertStore,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: SharedArrayBufferStore,
  pub compiled_wasm_module_store: CompiledWasmModuleStore,
  pub parsed_source_cache: ParsedSourceCache,
  pub maybe_resolver: Option<Arc<CliResolver>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  pub node_analysis_cache: NodeAnalysisCache,
  pub npm_cache: NpmCache,
  pub npm_resolver: NpmPackageResolver,
  pub cjs_resolutions: Mutex<HashSet<ModuleSpecifier>>,
  progress_bar: ProgressBar,
  node_std_graph_prepared: AtomicBool,
}

impl Deref for ProcState {
  type Target = Arc<Inner>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ProcState {
  pub async fn build(flags: Flags) -> Result<Self, AnyError> {
    Self::from_options(Arc::new(CliOptions::from_flags(flags)?)).await
  }

  pub async fn from_options(
    options: Arc<CliOptions>,
  ) -> Result<Self, AnyError> {
    Self::build_with_sender(options, None).await
  }

  pub async fn build_for_file_watcher(
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
    self.0 = Arc::new(Inner {
      dir: self.dir.clone(),
      options: self.options.clone(),
      emit_cache: self.emit_cache.clone(),
      emit_options_hash: self.emit_options_hash,
      emit_options: self.emit_options.clone(),
      file_fetcher: self.file_fetcher.clone(),
      http_client: self.http_client.clone(),
      graph_data: Default::default(),
      lockfile: self.lockfile.clone(),
      maybe_import_map: self.maybe_import_map.clone(),
      maybe_inspector_server: self.maybe_inspector_server.clone(),
      root_cert_store: self.root_cert_store.clone(),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      parsed_source_cache: self.parsed_source_cache.reset_for_file_watcher(),
      maybe_resolver: self.maybe_resolver.clone(),
      maybe_file_watcher_reporter: self.maybe_file_watcher_reporter.clone(),
      node_analysis_cache: self.node_analysis_cache.clone(),
      npm_cache: self.npm_cache.clone(),
      npm_resolver: self.npm_resolver.clone(),
      cjs_resolutions: Default::default(),
      progress_bar: self.progress_bar.clone(),
      node_std_graph_prepared: AtomicBool::new(false),
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
    let blob_store = BlobStore::default();
    let broadcast_channel = InMemoryBroadcastChannel::default();
    let shared_array_buffer_store = SharedArrayBufferStore::default();
    let compiled_wasm_module_store = CompiledWasmModuleStore::default();
    let dir = cli_options.resolve_deno_dir()?;
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

    let maybe_import_map = cli_options
      .resolve_import_map(&file_fetcher)
      .await?
      .map(Arc::new);
    let maybe_inspector_server =
      cli_options.resolve_inspector_server().map(Arc::new);

    let maybe_cli_resolver = CliResolver::maybe_new(
      cli_options.to_maybe_jsx_import_source_config(),
      maybe_import_map.clone(),
    );
    let maybe_resolver = maybe_cli_resolver.map(Arc::new);

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
      ParsedSourceCache::new(Some(dir.dep_analysis_db_file_path()));
    let registry_url = RealNpmRegistryApi::default_url();
    let npm_cache = NpmCache::from_deno_dir(
      &dir,
      cli_options.cache_setting(),
      http_client.clone(),
      progress_bar.clone(),
    );
    let api = RealNpmRegistryApi::new(
      registry_url,
      npm_cache.clone(),
      http_client.clone(),
      progress_bar.clone(),
    );
    let npm_resolver = NpmPackageResolver::new_with_maybe_lockfile(
      npm_cache.clone(),
      api,
      cli_options.no_npm(),
      cli_options
        .resolve_local_node_modules_folder()
        .with_context(|| "Resolving local node_modules folder.")?,
      lockfile.as_ref().cloned(),
    )
    .await?;
    let node_analysis_cache =
      NodeAnalysisCache::new(Some(dir.node_analysis_db_file_path()));

    let emit_options: deno_ast::EmitOptions = ts_config_result.ts_config.into();
    Ok(ProcState(Arc::new(Inner {
      dir,
      options: cli_options,
      emit_cache,
      emit_options_hash: FastInsecureHasher::new()
        .write_hashable(&emit_options)
        .finish(),
      emit_options,
      file_fetcher: Arc::new(file_fetcher),
      http_client,
      graph_data: Default::default(),
      lockfile,
      maybe_import_map,
      maybe_inspector_server,
      root_cert_store,
      blob_store,
      broadcast_channel,
      shared_array_buffer_store,
      compiled_wasm_module_store,
      parsed_source_cache,
      maybe_resolver,
      maybe_file_watcher_reporter,
      node_analysis_cache,
      npm_cache,
      npm_resolver,
      cjs_resolutions: Default::default(),
      progress_bar,
      node_std_graph_prepared: AtomicBool::new(false),
    })))
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate `self.graph_data` in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  #[allow(clippy::too_many_arguments)]
  pub async fn prepare_module_load(
    &self,
    roots: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    lib: TsTypeLib,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Result<(), AnyError> {
    log::debug!("Preparing module load.");
    let _pb_clear_guard = self.progress_bar.clear_guard();

    let mut cache = cache::FetchCacher::new(
      self.emit_cache.clone(),
      self.file_fetcher.clone(),
      root_permissions,
      dynamic_permissions,
    );
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_resolver =
      self.maybe_resolver.as_ref().map(|r| r.as_graph_resolver());
    let maybe_file_watcher_reporter: Option<&dyn deno_graph::source::Reporter> =
      if let Some(reporter) = &self.maybe_file_watcher_reporter {
        Some(reporter)
      } else {
        None
      };

    let analyzer = self.parsed_source_cache.as_analyzer();

    log::debug!("Creating module graph.");
    let mut graph = self.graph_data.read().graph_inner_clone();

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> =
      graph.specifiers().map(|(s, _)| s.clone()).collect();

    graph
      .build(
        roots.clone(),
        &mut cache,
        deno_graph::BuildOptions {
          is_dynamic,
          imports: maybe_imports,
          resolver: maybe_resolver,
          module_analyzer: Some(&*analyzer),
          reporter: maybe_file_watcher_reporter,
        },
      )
      .await;

    // If there is a lockfile, validate the integrity of all the modules.
    if let Some(lockfile) = &self.lockfile {
      graph_lock_or_exit(&graph, &mut lockfile.lock());
    }

    let (npm_package_reqs, has_node_builtin_specifier) = {
      graph_valid_with_cli_options(&graph, &roots, &self.options)?;
      let mut graph_data = self.graph_data.write();
      graph_data.update_graph(Arc::new(graph));
      (
        graph_data.npm_packages.clone(),
        graph_data.has_node_builtin_specifier,
      )
    };

    if !npm_package_reqs.is_empty() {
      self.npm_resolver.add_package_reqs(npm_package_reqs).await?;
      self.prepare_node_std_graph().await?;
    }

    if has_node_builtin_specifier
      && self.options.type_check_mode() != TypeCheckMode::None
    {
      self
        .npm_resolver
        .inject_synthetic_types_node_package()
        .await?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    let is_std_node = roots.len() == 1 && roots[0] == *node::MODULE_ALL_URL;
    if self.options.type_check_mode() != TypeCheckMode::None
      && !is_std_node
      && !self.graph_data.read().is_type_checked(&roots, lib)
    {
      log::debug!("Type checking.");
      let maybe_config_specifier = self.options.maybe_config_file_specifier();
      let (graph, has_node_builtin_specifier) = {
        let graph_data = self.graph_data.read();
        (
          Arc::new(graph_data.graph.segment(&roots)),
          graph_data.has_node_builtin_specifier,
        )
      };
      let options = check::CheckOptions {
        type_check_mode: self.options.type_check_mode(),
        debug: self.options.log_level() == Some(log::Level::Debug),
        maybe_config_specifier,
        ts_config: self
          .options
          .resolve_ts_config_for_emit(TsConfigType::Check { lib })?
          .ts_config,
        log_checks: true,
        reload: self.options.reload_flag()
          && !roots.iter().all(|r| reload_exclusions.contains(r)),
        has_node_builtin_specifier,
      };
      let check_cache =
        TypeCheckCache::new(&self.dir.type_checking_cache_db_file_path());
      let check_result =
        check::check(graph, &check_cache, &self.npm_resolver, options)?;
      self.graph_data.write().set_type_checked(&roots, lib);
      if !check_result.diagnostics.is_empty() {
        return Err(anyhow!(check_result.diagnostics));
      }
      log::debug!("{}", check_result.stats);
    }

    // any updates to the lockfile should be updated now
    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock();
      g.write()?;
    }

    log::debug!("Prepared module load.");

    Ok(())
  }

  /// Helper around prepare_module_load that loads and type checks
  /// the provided files.
  pub async fn load_and_type_check_files(
    &self,
    files: &[String],
  ) -> Result<(), AnyError> {
    let lib = self.options.ts_type_lib_window();

    let specifiers = files
      .iter()
      .map(|file| resolve_url_or_path(file))
      .collect::<Result<Vec<_>, _>>()?;
    self
      .prepare_module_load(
        specifiers,
        false,
        lib,
        PermissionsContainer::allow_all(),
        PermissionsContainer::allow_all(),
      )
      .await
  }

  /// Add the builtin node modules to the graph data.
  pub async fn prepare_node_std_graph(&self) -> Result<(), AnyError> {
    if self.node_std_graph_prepared.load(Ordering::Relaxed) {
      return Ok(());
    }

    let mut graph = self.graph_data.read().graph_inner_clone();
    let mut loader = self.create_graph_loader();
    let analyzer = self.parsed_source_cache.as_analyzer();
    graph
      .build(
        vec![node::MODULE_ALL_URL.clone()],
        &mut loader,
        deno_graph::BuildOptions {
          module_analyzer: Some(&*analyzer),
          ..Default::default()
        },
      )
      .await;

    self.graph_data.write().update_graph(Arc::new(graph));
    self.node_std_graph_prepared.store(true, Ordering::Relaxed);
    Ok(())
  }

  fn handle_node_resolve_result(
    &self,
    result: Result<Option<node::NodeResolution>, AnyError>,
  ) -> Result<ModuleSpecifier, AnyError> {
    let response = match result? {
      Some(response) => response,
      None => return Err(generic_error("not found")),
    };
    if let NodeResolution::CommonJs(specifier) = &response {
      // remember that this was a common js resolution
      self.cjs_resolutions.lock().insert(specifier.clone());
    } else if let NodeResolution::BuiltIn(specifier) = &response {
      return node::resolve_builtin_node_module(specifier);
    }
    Ok(response.into_url())
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    permissions: &mut PermissionsContainer,
  ) -> Result<ModuleSpecifier, AnyError> {
    if let Ok(referrer) = deno_core::resolve_url_or_path(referrer) {
      if self.npm_resolver.in_npm_package(&referrer) {
        // we're in an npm package, so use node resolution
        return self
          .handle_node_resolve_result(node::node_resolve(
            specifier,
            &referrer,
            NodeResolutionMode::Execution,
            &self.npm_resolver,
            permissions,
          ))
          .with_context(|| {
            format!("Could not resolve '{specifier}' from '{referrer}'.")
          });
      }

      let graph_data = self.graph_data.read();
      let graph = &graph_data.graph;
      let maybe_resolved = match graph.get(&referrer) {
        Some(module) => module
          .dependencies
          .get(specifier)
          .map(|d| (&module.specifier, &d.maybe_code)),
        _ => None,
      };

      match maybe_resolved {
        Some((found_referrer, Resolution::Ok(resolved))) => {
          let specifier = &resolved.specifier;
          if let Ok(reference) = NpmPackageReference::from_specifier(specifier)
          {
            if !self.options.unstable()
              && matches!(found_referrer.scheme(), "http" | "https")
            {
              return Err(custom_error(
                "NotSupported",
                format!("importing npm specifiers in remote modules requires the --unstable flag (referrer: {found_referrer})"),
              ));
            }

            return self
              .handle_node_resolve_result(node::node_resolve_npm_reference(
                &reference,
                NodeResolutionMode::Execution,
                &self.npm_resolver,
                permissions,
              ))
              .with_context(|| format!("Could not resolve '{reference}'."));
          } else {
            return Ok(specifier.clone());
          }
        }
        Some((_, Resolution::Err(err))) => {
          return Err(custom_error(
            "TypeError",
            format!("{}\n", err.to_string_with_range()),
          ))
        }
        Some((_, Resolution::None)) | None => {}
      }
    }

    // Built-in Node modules
    if let Some(module_name) = specifier.strip_prefix("node:") {
      return node::resolve_builtin_node_module(module_name);
    }

    // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
    // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
    // but sadly that's not the case due to missing APIs in V8.
    let is_repl = matches!(self.options.sub_command(), DenoSubcommand::Repl(_));
    let referrer = if referrer.is_empty() && is_repl {
      deno_core::resolve_url_or_path("./$deno$repl.ts")?
    } else {
      deno_core::resolve_url_or_path(referrer)?
    };

    // FIXME(bartlomieju): this is another hack way to provide NPM specifier
    // support in REPL. This should be fixed.
    if is_repl {
      let specifier = self
        .maybe_resolver
        .as_ref()
        .and_then(|resolver| resolver.resolve(specifier, &referrer).ok())
        .or_else(|| ModuleSpecifier::parse(specifier).ok());
      if let Some(specifier) = specifier {
        if let Ok(reference) = NpmPackageReference::from_specifier(&specifier) {
          return self
            .handle_node_resolve_result(node::node_resolve_npm_reference(
              &reference,
              deno_runtime::deno_node::NodeResolutionMode::Execution,
              &self.npm_resolver,
              permissions,
            ))
            .with_context(|| format!("Could not resolve '{reference}'."));
        }
      }
    }

    if let Some(resolver) = &self.maybe_resolver {
      resolver.resolve(specifier, &referrer)
    } else {
      deno_core::resolve_import(specifier, referrer.as_str())
        .map_err(|err| err.into())
    }
  }

  pub fn cache_module_emits(&self) -> Result<(), AnyError> {
    let graph = self.graph();
    for module in graph.modules() {
      let is_emittable = module.kind != ModuleKind::External
        && matches!(
          module.media_type,
          MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Jsx
            | MediaType::Tsx
        );
      if is_emittable {
        if let Some(code) = &module.maybe_source {
          emit_parsed_source(
            &self.emit_cache,
            &self.parsed_source_cache,
            &module.specifier,
            module.media_type,
            code,
            &self.emit_options,
            self.emit_options_hash,
          )?;
        }
      }
    }
    Ok(())
  }

  /// Creates the default loader used for creating a graph.
  pub fn create_graph_loader(&self) -> cache::FetchCacher {
    cache::FetchCacher::new(
      self.emit_cache.clone(),
      self.file_fetcher.clone(),
      PermissionsContainer::allow_all(),
      PermissionsContainer::allow_all(),
    )
  }

  pub async fn create_graph(
    &self,
    roots: Vec<ModuleSpecifier>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = self.create_graph_loader();
    self.create_graph_with_loader(roots, &mut cache).await
  }

  pub async fn create_graph_with_loader(
    &self,
    roots: Vec<ModuleSpecifier>,
    loader: &mut dyn Loader,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let maybe_imports = self.options.to_maybe_imports()?;

    let maybe_cli_resolver = CliResolver::maybe_new(
      self.options.to_maybe_jsx_import_source_config(),
      self.maybe_import_map.clone(),
    );
    let maybe_graph_resolver =
      maybe_cli_resolver.as_ref().map(|r| r.as_graph_resolver());
    let analyzer = self.parsed_source_cache.as_analyzer();

    let mut graph = ModuleGraph::default();
    graph
      .build(
        roots,
        loader,
        deno_graph::BuildOptions {
          is_dynamic: false,
          imports: maybe_imports,
          resolver: maybe_graph_resolver,
          module_analyzer: Some(&*analyzer),
          reporter: None,
        },
      )
      .await;

    // add the found npm package requirements to the npm resolver and cache them
    let graph_npm_info = resolve_graph_npm_info(&graph);
    if !graph_npm_info.package_reqs.is_empty() {
      self
        .npm_resolver
        .add_package_reqs(graph_npm_info.package_reqs)
        .await?;
    }
    if graph_npm_info.has_node_builtin_specifier
      && self.options.type_check_mode() != TypeCheckMode::None
    {
      self
        .npm_resolver
        .inject_synthetic_types_node_package()
        .await?;
    }

    Ok(graph)
  }

  pub fn graph(&self) -> Arc<ModuleGraph> {
    self.graph_data.read().graph.clone()
  }
}

#[derive(Clone, Debug)]
struct FileWatcherReporter {
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

#[derive(Debug, Default)]
struct GraphData {
  graph: Arc<ModuleGraph>,
  /// The npm package requirements from all the encountered graphs
  /// in the order that they should be resolved.
  npm_packages: Vec<NpmPackageReq>,
  /// If the graph had a "node:" specifier.
  has_node_builtin_specifier: bool,
  checked_libs: HashMap<TsTypeLib, HashSet<ModuleSpecifier>>,
}

impl GraphData {
  /// Store data from `graph` into `self`.
  pub fn update_graph(&mut self, graph: Arc<ModuleGraph>) {
    let mut has_npm_specifier_in_graph = false;

    for (specifier, _) in graph.specifiers() {
      match specifier.scheme() {
        "node" => {
          // We don't ever set this back to false because once it's
          // on then it's on globally.
          self.has_node_builtin_specifier = true;
        }
        "npm" => {
          if !has_npm_specifier_in_graph
            && NpmPackageReference::from_specifier(specifier).is_ok()
          {
            has_npm_specifier_in_graph = true;
          }
        }
        _ => {}
      }

      if has_npm_specifier_in_graph && self.has_node_builtin_specifier {
        break; // exit early
      }
    }

    if has_npm_specifier_in_graph {
      self.npm_packages = resolve_graph_npm_info(&graph).package_reqs;
    }
    self.graph = graph;
  }

  // todo(dsherret): remove the need for cloning this (maybe if we used an AsyncRefCell)
  pub fn graph_inner_clone(&self) -> ModuleGraph {
    (*self.graph).clone()
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub fn set_type_checked(
    &mut self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) {
    let entries = self.graph.walk(
      roots,
      deno_graph::WalkOptions {
        check_js: true,
        follow_dynamic: true,
        follow_type_only: true,
      },
    );
    let checked_lib_set = self.checked_libs.entry(lib).or_default();
    for (specifier, _) in entries {
      checked_lib_set.insert(specifier.clone());
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) -> bool {
    match self.checked_libs.get(&lib) {
      Some(checked_lib_set) => roots.iter().all(|r| {
        let found = self.graph.resolve(r);
        checked_lib_set.contains(&found)
      }),
      None => false,
    }
  }
}
