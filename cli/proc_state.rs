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
use crate::graph_util::build_graph_with_npm_resolution;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::graph_valid_with_cli_options;
use crate::graph_util::ModuleGraphContainer;
use crate::http_util::HttpClient;
use crate::node;
use crate::node::NodeResolution;
use crate::npm::create_npm_fs_resolver;
use crate::npm::NpmCache;
use crate::npm::NpmPackageResolver;
use crate::npm::NpmRegistryApi;
use crate::npm::NpmResolution;
use crate::npm::PackageJsonDepsInstaller;
use crate::resolver::CliGraphResolver;
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
use deno_core::resolve_url_or_path;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;
use deno_graph::npm::NpmPackageReqReference;
use deno_graph::source::Loader;
use deno_graph::source::Resolver;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Resolution;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMap;
use log::warn;
use std::borrow::Cow;
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
  pub file_fetcher: Arc<FileFetcher>,
  pub http_client: HttpClient,
  pub options: Arc<CliOptions>,
  pub emit_cache: EmitCache,
  pub emit_options: deno_ast::EmitOptions,
  pub emit_options_hash: u64,
  graph_container: ModuleGraphContainer,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub root_cert_store: RootCertStore,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: SharedArrayBufferStore,
  pub compiled_wasm_module_store: CompiledWasmModuleStore,
  pub parsed_source_cache: ParsedSourceCache,
  pub resolver: Arc<CliGraphResolver>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  pub node_analysis_cache: NodeAnalysisCache,
  pub npm_api: NpmRegistryApi,
  pub npm_cache: NpmCache,
  pub npm_resolver: NpmPackageResolver,
  pub npm_resolution: NpmResolution,
  pub package_json_deps_installer: PackageJsonDepsInstaller,
  pub cjs_resolutions: Mutex<HashSet<ModuleSpecifier>>,
  progress_bar: ProgressBar,
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
      graph_container: Default::default(),
      lockfile: self.lockfile.clone(),
      maybe_import_map: self.maybe_import_map.clone(),
      maybe_inspector_server: self.maybe_inspector_server.clone(),
      root_cert_store: self.root_cert_store.clone(),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      parsed_source_cache: self.parsed_source_cache.reset_for_file_watcher(),
      resolver: self.resolver.clone(),
      maybe_file_watcher_reporter: self.maybe_file_watcher_reporter.clone(),
      node_analysis_cache: self.node_analysis_cache.clone(),
      npm_api: self.npm_api.clone(),
      npm_cache: self.npm_cache.clone(),
      npm_resolver: self.npm_resolver.clone(),
      npm_resolution: self.npm_resolution.clone(),
      package_json_deps_installer: self.package_json_deps_installer.clone(),
      cjs_resolutions: Default::default(),
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

    let npm_registry_url = NpmRegistryApi::default_url().to_owned();
    let npm_cache = NpmCache::from_deno_dir(
      &dir,
      cli_options.cache_setting(),
      http_client.clone(),
      progress_bar.clone(),
    );
    let npm_api = NpmRegistryApi::new(
      npm_registry_url.clone(),
      npm_cache.clone(),
      http_client.clone(),
      progress_bar.clone(),
    );
    let npm_snapshot = cli_options
      .resolve_npm_resolution_snapshot(&npm_api)
      .await?;
    let npm_resolution = NpmResolution::new(
      npm_api.clone(),
      npm_snapshot,
      lockfile.as_ref().cloned(),
    );
    let npm_fs_resolver = create_npm_fs_resolver(
      npm_cache,
      &progress_bar,
      npm_registry_url,
      npm_resolution.clone(),
      cli_options.node_modules_dir_path(),
    );
    let npm_resolver = NpmPackageResolver::new(
      npm_resolution.clone(),
      npm_fs_resolver,
      lockfile.as_ref().cloned(),
    );
    let package_json_deps_installer = PackageJsonDepsInstaller::new(
      npm_api.clone(),
      npm_resolution.clone(),
      cli_options.maybe_package_json_deps(),
    );
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
      ParsedSourceCache::new(Some(dir.dep_analysis_db_file_path()));
    let npm_cache = NpmCache::from_deno_dir(
      &dir,
      cli_options.cache_setting(),
      http_client.clone(),
      progress_bar.clone(),
    );
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
      graph_container: Default::default(),
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
      node_analysis_cache,
      npm_api,
      npm_cache,
      npm_resolver,
      npm_resolution,
      package_json_deps_installer,
      cjs_resolutions: Default::default(),
      progress_bar,
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
      self.options.node_modules_dir_specifier(),
    );
    let maybe_imports = self.options.to_maybe_imports()?;
    let graph_resolver = self.resolver.as_graph_resolver();
    let graph_npm_resolver = self.resolver.as_graph_npm_resolver();
    let maybe_file_watcher_reporter: Option<&dyn deno_graph::source::Reporter> =
      if let Some(reporter) = &self.maybe_file_watcher_reporter {
        Some(reporter)
      } else {
        None
      };

    let analyzer = self.parsed_source_cache.as_analyzer();

    log::debug!("Creating module graph.");
    let mut graph_update_permit =
      self.graph_container.acquire_update_permit().await;
    let graph = graph_update_permit.graph_mut();

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> =
      graph.specifiers().map(|(s, _)| s.clone()).collect();

    build_graph_with_npm_resolution(
      graph,
      &self.npm_resolver,
      roots.clone(),
      &mut cache,
      deno_graph::BuildOptions {
        is_dynamic,
        imports: maybe_imports,
        resolver: Some(graph_resolver),
        npm_resolver: Some(graph_npm_resolver),
        module_analyzer: Some(&*analyzer),
        reporter: maybe_file_watcher_reporter,
      },
    )
    .await?;

    // If there is a lockfile, validate the integrity of all the modules.
    if let Some(lockfile) = &self.lockfile {
      graph_lock_or_exit(graph, &mut lockfile.lock());
    }

    graph_valid_with_cli_options(graph, &roots, &self.options)?;
    // save the graph and get a reference to the new graph
    let graph = graph_update_permit.commit();

    if graph.has_node_specifier
      && self.options.type_check_mode() != TypeCheckMode::None
    {
      self
        .npm_resolver
        .inject_synthetic_types_node_package()
        .await?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    if self.options.type_check_mode() != TypeCheckMode::None
      && !self.graph_container.is_type_checked(&roots, lib)
    {
      log::debug!("Type checking.");
      let maybe_config_specifier = self.options.maybe_config_file_specifier();
      let graph = Arc::new(graph.segment(&roots));
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
      };
      let check_cache =
        TypeCheckCache::new(&self.dir.type_checking_cache_db_file_path());
      let check_result =
        check::check(graph, &check_cache, &self.npm_resolver, options)?;
      self.graph_container.set_type_checked(&roots, lib);
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
      .map(|file| resolve_url_or_path(file, self.options.initial_cwd()))
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
    // TODO(bartlomieju): ideally we shouldn't need to call `current_dir()` on each
    // call - maybe it should be caller's responsibility to pass it as an arg?
    let cwd = std::env::current_dir().context("Unable to get CWD")?;
    let referrer_result = deno_core::resolve_url_or_path(referrer, &cwd);

    if let Ok(referrer) = referrer_result.as_ref() {
      if self.npm_resolver.in_npm_package(referrer) {
        // we're in an npm package, so use node resolution
        return self
          .handle_node_resolve_result(node::node_resolve(
            specifier,
            referrer,
            NodeResolutionMode::Execution,
            &self.npm_resolver,
            permissions,
          ))
          .with_context(|| {
            format!("Could not resolve '{specifier}' from '{referrer}'.")
          });
      }

      let graph = self.graph_container.graph();
      let maybe_resolved = match graph.get(referrer) {
        Some(Module::Esm(module)) => {
          module.dependencies.get(specifier).map(|d| &d.maybe_code)
        }
        _ => None,
      };

      match maybe_resolved {
        Some(Resolution::Ok(resolved)) => {
          let specifier = &resolved.specifier;

          return match graph.get(specifier) {
            Some(Module::Npm(module)) => self
              .handle_node_resolve_result(node::node_resolve_npm_reference(
                &module.nv_reference,
                NodeResolutionMode::Execution,
                &self.npm_resolver,
                permissions,
              ))
              .with_context(|| {
                format!("Could not resolve '{}'.", module.nv_reference)
              }),
            Some(Module::Node(module)) => {
              node::resolve_builtin_node_module(&module.module_name)
            }
            Some(Module::Esm(module)) => Ok(module.specifier.clone()),
            Some(Module::Json(module)) => Ok(module.specifier.clone()),
            Some(Module::External(module)) => {
              Ok(node::resolve_specifier_into_node_modules(&module.specifier))
            }
            None => Ok(specifier.clone()),
          };
        }
        Some(Resolution::Err(err)) => {
          return Err(custom_error(
            "TypeError",
            format!("{}\n", err.to_string_with_range()),
          ))
        }
        Some(Resolution::None) | None => {}
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
      deno_core::resolve_path("./$deno$repl.ts", &cwd)?
    } else {
      referrer_result?
    };

    // FIXME(bartlomieju): this is another hack way to provide NPM specifier
    // support in REPL. This should be fixed.
    let resolution = self.resolver.resolve(specifier, &referrer);

    if is_repl {
      let specifier = resolution
        .as_ref()
        .ok()
        .map(Cow::Borrowed)
        .or_else(|| ModuleSpecifier::parse(specifier).ok().map(Cow::Owned));
      if let Some(specifier) = specifier {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          let reference =
            self.npm_resolution.pkg_req_ref_to_nv_ref(reference)?;
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

    resolution
  }

  pub fn cache_module_emits(&self) -> Result<(), AnyError> {
    let graph = self.graph();
    for module in graph.modules() {
      if let Module::Esm(module) = module {
        let is_emittable = matches!(
          module.media_type,
          MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Jsx
            | MediaType::Tsx
        );
        if is_emittable {
          emit_parsed_source(
            &self.emit_cache,
            &self.parsed_source_cache,
            &module.specifier,
            module.media_type,
            &module.source,
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
      self.options.node_modules_dir_specifier(),
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

    let cli_resolver = CliGraphResolver::new(
      self.options.to_maybe_jsx_import_source_config(),
      self.maybe_import_map.clone(),
      self.options.no_npm(),
      self.npm_api.clone(),
      self.npm_resolution.clone(),
      self.package_json_deps_installer.clone(),
    );
    let graph_resolver = cli_resolver.as_graph_resolver();
    let graph_npm_resolver = cli_resolver.as_graph_npm_resolver();
    let analyzer = self.parsed_source_cache.as_analyzer();

    let mut graph = ModuleGraph::default();
    build_graph_with_npm_resolution(
      &mut graph,
      &self.npm_resolver,
      roots,
      loader,
      deno_graph::BuildOptions {
        is_dynamic: false,
        imports: maybe_imports,
        resolver: Some(graph_resolver),
        npm_resolver: Some(graph_npm_resolver),
        module_analyzer: Some(&*analyzer),
        reporter: None,
      },
    )
    .await?;

    if graph.has_node_specifier
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
    self.graph_container.graph()
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
