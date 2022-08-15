// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::TypeCheckMode;
use crate::cache;
use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::TypeCheckCache;
use crate::compat;
use crate::compat::NodeEsmResolver;
use crate::deno_dir;
use crate::emit;
use crate::emit::emit_parsed_source;
use crate::emit::TsConfigType;
use crate::emit::TsTypeLib;
use crate::file_fetcher::FileFetcher;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;
use crate::http_cache;
use crate::lockfile::as_maybe_locker;
use crate::lockfile::Lockfile;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::url::Url;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;
use deno_graph::create_graph;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::Loader;
use deno_graph::source::ResolveResponse;
use deno_graph::ModuleKind;
use deno_graph::Resolved;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::Permissions;
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
  pub dir: deno_dir::DenoDir,
  pub file_fetcher: FileFetcher,
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
  maybe_resolver: Option<Arc<dyn deno_graph::source::Resolver + Send + Sync>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
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
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);
    let root_cert_store = cli_options.resolve_root_cert_store()?;
    let cache_usage = cli_options.cache_setting();
    let file_fetcher = FileFetcher::new(
      http_cache,
      cache_usage,
      !cli_options.no_remote(),
      Some(root_cert_store.clone()),
      blob_store.clone(),
      cli_options
        .unsafely_ignore_certificate_errors()
        .map(ToOwned::to_owned),
    )?;

    let lockfile = cli_options
      .resolve_lock_file()?
      .map(|f| Arc::new(Mutex::new(f)));
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

    // FIXME(bartlomieju): `NodeEsmResolver` is not aware of JSX resolver
    // created below
    let node_resolver = NodeEsmResolver::new(
      maybe_import_map.clone().map(ImportMapResolver::new),
    );
    let maybe_import_map_resolver =
      maybe_import_map.clone().map(ImportMapResolver::new);
    let maybe_jsx_resolver = cli_options
      .to_maybe_jsx_import_source_module()
      .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()));
    let maybe_resolver: Option<
      Arc<dyn deno_graph::source::Resolver + Send + Sync>,
    > = if cli_options.compat() {
      Some(Arc::new(node_resolver))
    } else if let Some(jsx_resolver) = maybe_jsx_resolver {
      // the JSX resolver offloads to the import map if present, otherwise uses
      // the default Deno explicit import resolution.
      Some(Arc::new(jsx_resolver))
    } else if let Some(import_map_resolver) = maybe_import_map_resolver {
      Some(Arc::new(import_map_resolver))
    } else {
      None
    };

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

    Ok(ProcState(Arc::new(Inner {
      dir,
      options: cli_options,
      emit_cache,
      emit_options_hash: FastInsecureHasher::new()
        // todo(dsherret): use hash of emit options instead as it's more specific
        .write(&ts_config_result.ts_config.as_bytes())
        .finish(),
      emit_options: ts_config_result.ts_config.into(),
      file_fetcher,
      graph_data: Default::default(),
      lockfile,
      maybe_import_map,
      maybe_inspector_server,
      root_cert_store,
      blob_store,
      broadcast_channel,
      shared_array_buffer_store,
      compiled_wasm_module_store,
      maybe_resolver,
      maybe_file_watcher_reporter,
    })))
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate `self.graph_data` in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  pub async fn prepare_module_load(
    &self,
    roots: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    lib: TsTypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    reload_on_watch: bool,
  ) -> Result<(), AnyError> {
    let maybe_resolver: Option<&dyn deno_graph::source::Resolver> =
      if let Some(resolver) = &self.maybe_resolver {
        Some(resolver.as_ref())
      } else {
        None
      };

    // NOTE(@bartlomieju):
    // Even though `roots` are fully resolved at this point, we are going
    // to resolve them through `maybe_resolver` to get module kind for the graph
    // or default to ESM.
    //
    // One might argue that this is a code smell, and I would agree. However
    // due to flux in "Node compatibility" it's not clear where it should be
    // decided what `ModuleKind` is decided for root specifier.
    let roots = roots
      .into_iter()
      .map(|r| {
        if let Some(resolver) = &maybe_resolver {
          let response =
            resolver.resolve(r.as_str(), &Url::parse("unused:").unwrap());
          // TODO(bartlomieju): this should be implemented in `deno_graph`
          match response {
            ResolveResponse::CommonJs(_) => (r, ModuleKind::CommonJs),
            ResolveResponse::Err(_) => unreachable!(),
            _ => (r, ModuleKind::Esm),
          }
        } else {
          (r, ModuleKind::Esm)
        }
      })
      .collect();

    // TODO(bartlomieju): this is very make-shift, is there an existing API
    // that we could include it like with "maybe_imports"?
    let roots = if self.options.compat() {
      let mut r = vec![(compat::GLOBAL_URL.clone(), ModuleKind::Esm)];
      r.extend(roots);
      r
    } else {
      roots
    };
    if !reload_on_watch {
      let graph_data = self.graph_data.read();
      if self.options.type_check_mode() == TypeCheckMode::None
        || graph_data.is_type_checked(&roots, &lib)
      {
        if let Some(result) = graph_data.check(
          &roots,
          self.options.type_check_mode() != TypeCheckMode::None,
          false,
        ) {
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
    let maybe_locker = as_maybe_locker(self.lockfile.clone());
    let maybe_imports = self.options.to_maybe_imports()?;

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

    let graph = create_graph(
      roots.clone(),
      is_dynamic,
      maybe_imports,
      &mut loader,
      maybe_resolver,
      maybe_locker,
      None,
      maybe_file_watcher_reporter,
    )
    .await;

    let needs_cjs_esm_translation = graph
      .modules()
      .iter()
      .any(|m| m.kind == ModuleKind::CommonJs);

    if needs_cjs_esm_translation {
      for module in graph.modules() {
        // TODO(bartlomieju): this is overly simplistic heuristic, once we are
        // in compat mode, all files ending with plain `.js` extension are
        // considered CommonJs modules. Which leads to situation where valid
        // ESM modules with `.js` extension might undergo translation (it won't
        // work in this situation).
        if module.kind == ModuleKind::CommonJs {
          let translated_source = compat::translate_cjs_to_esm(
            &self.file_fetcher,
            &module.specifier,
            module.maybe_source.as_ref().unwrap().to_string(),
            module.media_type,
          )
          .await?;
          let mut graph_data = self.graph_data.write();
          graph_data
            .add_cjs_esm_translation(&module.specifier, translated_source);
        }
      }
    }

    // If there was a locker, validate the integrity of all the modules in the
    // locker.
    graph_lock_or_exit(&graph);

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> = {
      let graph_data = self.graph_data.read();
      graph_data.entries().map(|(s, _)| s).cloned().collect()
    };

    {
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
    }

    // type check if necessary
    if self.options.type_check_mode() != TypeCheckMode::None {
      let maybe_config_specifier = self.options.maybe_config_file_specifier();
      let roots = roots.clone();
      let options = emit::CheckOptions {
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
      let check_result =
        emit::check(&roots, graph_data, &check_cache, options)?;
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

    Ok(())
  }

  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<ModuleSpecifier, AnyError> {
    if let Ok(referrer) = deno_core::resolve_url_or_path(referrer) {
      let graph_data = self.graph_data.read();
      let found_referrer = graph_data.follow_redirect(&referrer);
      let maybe_resolved = match graph_data.get(&found_referrer) {
        Some(ModuleEntry::Module { dependencies, .. }) => {
          dependencies.get(specifier).map(|d| &d.maybe_code)
        }
        _ => None,
      };

      match maybe_resolved {
        Some(Resolved::Ok { specifier, .. }) => return Ok(specifier.clone()),
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
    let referrer = if referrer.is_empty()
      && matches!(self.options.sub_command(), DenoSubcommand::Repl(_))
    {
      deno_core::resolve_url_or_path("./$deno$repl.ts").unwrap()
    } else {
      deno_core::resolve_url_or_path(referrer).unwrap()
    };

    let maybe_resolver: Option<&dyn deno_graph::source::Resolver> =
      if let Some(resolver) = &self.maybe_resolver {
        Some(resolver.as_ref())
      } else {
        None
      };
    if let Some(resolver) = &maybe_resolver {
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
        maybe_parsed_source: Some(parsed_source),
        ..
      } = entry
      {
        emit_parsed_source(
          &self.emit_cache,
          specifier,
          parsed_source,
          &self.emit_options,
          self.emit_options_hash,
        )?;
      }
    }
    Ok(())
  }

  pub async fn create_graph(
    &self,
    roots: Vec<(ModuleSpecifier, ModuleKind)>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = cache::FetchCacher::new(
      self.emit_cache.clone(),
      self.file_fetcher.clone(),
      Permissions::allow_all(),
      Permissions::allow_all(),
    );
    let maybe_locker = as_maybe_locker(self.lockfile.clone());
    let maybe_import_map_resolver =
      self.maybe_import_map.clone().map(ImportMapResolver::new);
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_jsx_resolver = self
      .options
      .to_maybe_jsx_import_source_module()
      .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()));
    let maybe_resolver = if maybe_jsx_resolver.is_some() {
      maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
    } else {
      maybe_import_map_resolver
        .as_ref()
        .map(|im| im.as_resolver())
    };

    Ok(
      create_graph(
        roots,
        false,
        maybe_imports,
        &mut cache,
        maybe_resolver,
        maybe_locker,
        None,
        None,
      )
      .await,
    )
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
