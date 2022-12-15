// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

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
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;
use crate::http_util::HttpClient;
use crate::node;
use crate::node::NodeResolution;
use crate::npm::resolve_npm_package_reqs;
use crate::npm::NpmCache;
use crate::npm::NpmPackageReference;
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
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;
use deno_graph::create_graph;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::Loader;
use deno_graph::source::Resolver;
use deno_graph::ModuleKind;
use deno_graph::Resolved;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use log::warn;
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
  pub file_fetcher: FileFetcher,
  pub http_client: HttpClient,
  pub options: Arc<CliOptions>,
  pub emit_cache: EmitCache,
  pub emit_options: deno_ast::EmitOptions,
  pub emit_options_hash: u64,
  pub graph_data: Arc<RwLock<GraphData>>,
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

    // Add the extra files listed in the watch flag
    if let Some(watch_paths) = ps.options.watch_paths() {
      files_to_watch_sender.send(watch_paths.clone()).unwrap();
    }

    if let Ok(Some(import_map_path)) = ps
      .options
      .resolve_import_map_specifier()
      .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
    {
      files_to_watch_sender.send(vec![import_map_path]).unwrap();
    }

    Ok(ps)
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
    )?;

    let lockfile = cli_options.maybe_lock_file();
    let maybe_import_map_specifier =
      cli_options.resolve_import_map_specifier()?;

    let maybe_import_map =
      if let Some(import_map_specifier) = maybe_import_map_specifier {
        let file = file_fetcher
          .fetch(&import_map_specifier, &mut Permissions::allow_all())
          .await
          .context(format!(
            "Unable to load '{}' import map",
            import_map_specifier
          ))?;
        let import_map =
          import_map_from_text(&import_map_specifier, &file.source)?;
        Some(Arc::new(import_map))
      } else {
        None
      };

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
    let maybe_lockfile = lockfile.as_ref().cloned();
    let mut npm_resolver = NpmPackageResolver::new(
      npm_cache.clone(),
      api,
      cli_options.no_npm(),
      cli_options
        .resolve_local_node_modules_folder()
        .with_context(|| "Resolving local node_modules folder.")?,
    );
    if let Some(lockfile) = maybe_lockfile.clone() {
      npm_resolver
        .add_lockfile_and_maybe_regenerate_snapshot(lockfile)
        .await?;
    }
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
      file_fetcher,
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
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    reload_on_watch: bool,
  ) -> Result<(), AnyError> {
    log::debug!("Preparing module load.");
    let _pb_clear_guard = self.progress_bar.clear_guard();

    let has_root_npm_specifier = roots.iter().any(|r| {
      r.scheme() == "npm" && NpmPackageReference::from_specifier(r).is_ok()
    });
    let roots = roots
      .into_iter()
      .map(|s| (s, ModuleKind::Esm))
      .collect::<Vec<_>>();

    if !reload_on_watch && !has_root_npm_specifier {
      let graph_data = self.graph_data.read();
      if self.options.type_check_mode() == TypeCheckMode::None
        || graph_data.is_type_checked(&roots, &lib)
      {
        if let Some(result) = graph_data.check(
          &roots,
          self.options.type_check_mode() != TypeCheckMode::None,
          false,
        ) {
          // TODO(bartlomieju): this is strange... ideally there should be only
          // one codepath in `prepare_module_load` so we don't forget things
          // like writing a lockfile. Figure a way to refactor this function.
          if let Some(ref lockfile) = self.lockfile {
            let g = lockfile.lock();
            g.write()?;
          }
          return result;
        }
      }
    }
    let mut cache = cache::FetchCacher::new(
      self.emit_cache.clone(),
      self.file_fetcher.clone(),
      root_permissions.clone(),
      dynamic_permissions.clone(),
    );
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_resolver =
      self.maybe_resolver.as_ref().map(|r| r.as_graph_resolver());

    struct ProcStateLoader<'a> {
      inner: &'a mut cache::FetchCacher,
      graph_data: Arc<RwLock<GraphData>>,
      reload: bool,
    }
    impl Loader for ProcStateLoader<'_> {
      fn get_cache_info(
        &self,
        specifier: &ModuleSpecifier,
      ) -> Option<CacheInfo> {
        self.inner.get_cache_info(specifier)
      }
      fn load(
        &mut self,
        specifier: &ModuleSpecifier,
        is_dynamic: bool,
      ) -> LoadFuture {
        let graph_data = self.graph_data.read();
        let found_specifier = graph_data.follow_redirect(specifier);
        match graph_data.get(&found_specifier) {
          Some(_) if !self.reload => {
            Box::pin(futures::future::ready(Err(anyhow!(""))))
          }
          _ => self.inner.load(specifier, is_dynamic),
        }
      }
    }
    let mut loader = ProcStateLoader {
      inner: &mut cache,
      graph_data: self.graph_data.clone(),
      reload: reload_on_watch,
    };

    let maybe_file_watcher_reporter: Option<&dyn deno_graph::source::Reporter> =
      if let Some(reporter) = &self.maybe_file_watcher_reporter {
        Some(reporter)
      } else {
        None
      };

    let analyzer = self.parsed_source_cache.as_analyzer();
    log::debug!("Creating module graph.");
    let graph = create_graph(
      roots.clone(),
      &mut loader,
      deno_graph::GraphOptions {
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

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> = {
      let graph_data = self.graph_data.read();
      graph_data.entries().map(|(s, _)| s).cloned().collect()
    };

    let npm_package_reqs = {
      let mut graph_data = self.graph_data.write();
      graph_data.add_graph(&graph, reload_on_watch);
      let check_js = self.options.check_js();
      graph_data
        .check(
          &roots,
          self.options.type_check_mode() != TypeCheckMode::None,
          check_js,
        )
        .unwrap()?;
      graph_data.npm_package_reqs().clone()
    };

    if !npm_package_reqs.is_empty() {
      self.npm_resolver.add_package_reqs(npm_package_reqs).await?;
      self.prepare_node_std_graph().await?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    let is_std_node = roots.len() == 1 && roots[0].0 == *node::MODULE_ALL_URL;
    if self.options.type_check_mode() != TypeCheckMode::None && !is_std_node {
      log::debug!("Type checking.");
      let maybe_config_specifier = self.options.maybe_config_file_specifier();
      let roots = roots.clone();
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
          && !roots.iter().all(|r| reload_exclusions.contains(&r.0)),
      };
      let check_cache =
        TypeCheckCache::new(&self.dir.type_checking_cache_db_file_path());
      let graph_data = self.graph_data.clone();
      let check_result = check::check(
        &roots,
        graph_data,
        &check_cache,
        self.npm_resolver.clone(),
        options,
      )?;
      if !check_result.diagnostics.is_empty() {
        return Err(anyhow!(check_result.diagnostics));
      }
      log::debug!("{}", check_result.stats);
    }

    if self.options.type_check_mode() != TypeCheckMode::None {
      let mut graph_data = self.graph_data.write();
      graph_data.set_type_checked(&roots, lib);
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
        Permissions::allow_all(),
        Permissions::allow_all(),
        false,
      )
      .await
  }

  /// Add the builtin node modules to the graph data.
  pub async fn prepare_node_std_graph(&self) -> Result<(), AnyError> {
    if self.node_std_graph_prepared.load(Ordering::Relaxed) {
      return Ok(());
    }

    let node_std_graph = self
      .create_graph(vec![(node::MODULE_ALL_URL.clone(), ModuleKind::Esm)])
      .await?;
    self.graph_data.write().add_graph(&node_std_graph, false);
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
          ))
          .with_context(|| {
            format!("Could not resolve '{}' from '{}'.", specifier, referrer)
          });
      }

      let graph_data = self.graph_data.read();
      let found_referrer = graph_data.follow_redirect(&referrer);
      let maybe_resolved = match graph_data.get(&found_referrer) {
        Some(ModuleEntry::Module { dependencies, .. }) => {
          dependencies.get(specifier).map(|d| &d.maybe_code)
        }
        _ => None,
      };

      match maybe_resolved {
        Some(Resolved::Ok { specifier, .. }) => {
          if let Ok(reference) = NpmPackageReference::from_specifier(specifier)
          {
            if !self.options.unstable()
              && matches!(found_referrer.scheme(), "http" | "https")
            {
              return Err(custom_error(
                "NotSupported",
                format!("importing npm specifiers in remote modules requires the --unstable flag (referrer: {})", found_referrer),
              ));
            }

            return self
              .handle_node_resolve_result(node::node_resolve_npm_reference(
                &reference,
                NodeResolutionMode::Execution,
                &self.npm_resolver,
              ))
              .with_context(|| format!("Could not resolve '{}'.", reference));
          } else {
            return Ok(specifier.clone());
          }
        }
        Some(Resolved::Err(err)) => {
          return Err(custom_error(
            "TypeError",
            format!("{}\n", err.to_string_with_range()),
          ))
        }
        Some(Resolved::None) | None => {}
      }
    }

    // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
    // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
    // but sadly that's not the case due to missing APIs in V8.
    let is_repl = matches!(self.options.sub_command(), DenoSubcommand::Repl(_));
    let referrer = if referrer.is_empty() && is_repl {
      deno_core::resolve_url_or_path("./$deno$repl.ts").unwrap()
    } else {
      deno_core::resolve_url_or_path(referrer).unwrap()
    };

    // FIXME(bartlomieju): this is another hack way to provide NPM specifier
    // support in REPL. This should be fixed.
    if is_repl {
      let specifier = self
        .maybe_resolver
        .as_ref()
        .and_then(|resolver| {
          resolver.resolve(specifier, &referrer).to_result().ok()
        })
        .or_else(|| ModuleSpecifier::parse(specifier).ok());
      if let Some(specifier) = specifier {
        if let Ok(reference) = NpmPackageReference::from_specifier(&specifier) {
          return self
            .handle_node_resolve_result(node::node_resolve_npm_reference(
              &reference,
              deno_runtime::deno_node::NodeResolutionMode::Execution,
              &self.npm_resolver,
            ))
            .with_context(|| format!("Could not resolve '{}'.", reference));
        }
      }
    }

    if let Some(resolver) = &self.maybe_resolver {
      resolver.resolve(specifier, &referrer).to_result()
    } else {
      deno_core::resolve_import(specifier, referrer.as_str())
        .map_err(|err| err.into())
    }
  }

  pub fn cache_module_emits(&self) -> Result<(), AnyError> {
    let graph_data = self.graph_data.read();
    for (specifier, entry) in graph_data.entries() {
      if let ModuleEntry::Module {
        code, media_type, ..
      } = entry
      {
        let is_emittable = matches!(
          media_type,
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
            specifier,
            *media_type,
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
      Permissions::allow_all(),
      Permissions::allow_all(),
    )
  }

  pub async fn create_graph(
    &self,
    roots: Vec<(ModuleSpecifier, ModuleKind)>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = self.create_graph_loader();
    self.create_graph_with_loader(roots, &mut cache).await
  }

  pub async fn create_graph_with_loader(
    &self,
    roots: Vec<(ModuleSpecifier, ModuleKind)>,
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

    let graph = create_graph(
      roots,
      loader,
      deno_graph::GraphOptions {
        is_dynamic: false,
        imports: maybe_imports,
        resolver: maybe_graph_resolver,
        module_analyzer: Some(&*analyzer),
        reporter: None,
      },
    )
    .await;

    // add the found npm package requirements to the npm resolver and cache them
    let npm_package_reqs = resolve_npm_package_reqs(&graph);
    if !npm_package_reqs.is_empty() {
      self.npm_resolver.add_package_reqs(npm_package_reqs).await?;
    }

    Ok(graph)
  }
}

pub fn import_map_from_text(
  specifier: &Url,
  json_text: &str,
) -> Result<ImportMap, AnyError> {
  debug_assert!(
    !specifier.as_str().contains("../"),
    "Import map specifier incorrectly contained ../: {}",
    specifier.as_str()
  );
  let result = import_map::parse_from_json(specifier, json_text)?;
  if !result.diagnostics.is_empty() {
    warn!(
      "Import map diagnostics:\n{}",
      result
        .diagnostics
        .into_iter()
        .map(|d| format!("  - {}", d))
        .collect::<Vec<_>>()
        .join("\n")
    );
  }
  Ok(result.import_map)
}

#[derive(Debug)]
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
