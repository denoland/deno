// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::colors;
use crate::compat;
use crate::compat::NodeEsmResolver;
use crate::config_file::ConfigFile;
use crate::config_file::MaybeImportsResult;
use crate::deno_dir;
use crate::emit;
use crate::file_fetcher::get_root_cert_store;
use crate::file_fetcher::CacheSetting;
use crate::file_fetcher::FileFetcher;
use crate::flags;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;
use crate::http_cache;
use crate::lockfile::as_maybe_locker;
use crate::lockfile::Lockfile;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;
use crate::source_maps::SourceMapGetter;
use crate::version;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url;
use deno_core::url::Url;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::SharedArrayBufferStore;
use deno_graph::create_graph;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::Loader;
use deno_graph::ModuleKind;
use deno_graph::Resolved;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::Permissions;
use import_map::parse_from_json;
use import_map::ImportMap;
use log::warn;
use std::collections::HashSet;
use std::env;
use std::ops::Deref;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[derive(Clone)]
pub struct ProcState(Arc<Inner>);

pub struct Inner {
  /// Flags parsed from `argv` contents.
  pub flags: Arc<flags::Flags>,
  pub dir: deno_dir::DenoDir,
  pub coverage_dir: Option<String>,
  pub file_fetcher: FileFetcher,
  graph_data: Arc<RwLock<GraphData>>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub maybe_config_file: Option<ConfigFile>,
  pub maybe_import_map: Option<Arc<ImportMap>>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub root_cert_store: Option<RootCertStore>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: SharedArrayBufferStore,
  pub compiled_wasm_module_store: CompiledWasmModuleStore,
  maybe_resolver: Option<Arc<dyn deno_graph::source::Resolver + Send + Sync>>,
}

impl Deref for ProcState {
  type Target = Arc<Inner>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ProcState {
  pub async fn build(flags: Arc<flags::Flags>) -> Result<Self, AnyError> {
    let maybe_custom_root = flags
      .cache_path
      .clone()
      .or_else(|| env::var("DENO_DIR").map(String::into).ok());
    let dir = deno_dir::DenoDir::new(maybe_custom_root)?;
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);

    let root_cert_store = get_root_cert_store(
      None,
      flags.ca_stores.clone(),
      flags.ca_file.clone(),
    )?;

    if let Some(insecure_allowlist) =
      flags.unsafely_ignore_certificate_errors.as_ref()
    {
      let domains = if insecure_allowlist.is_empty() {
        "for all hostnames".to_string()
      } else {
        format!("for: {}", insecure_allowlist.join(", "))
      };
      let msg =
        format!("DANGER: TLS certificate validation is disabled {}", domains);
      eprintln!("{}", colors::yellow(msg));
    }

    let cache_usage = if flags.cached_only {
      CacheSetting::Only
    } else if !flags.cache_blocklist.is_empty() {
      CacheSetting::ReloadSome(flags.cache_blocklist.clone())
    } else if flags.reload {
      CacheSetting::ReloadAll
    } else {
      CacheSetting::Use
    };

    let blob_store = BlobStore::default();
    let broadcast_channel = InMemoryBroadcastChannel::default();
    let shared_array_buffer_store = SharedArrayBufferStore::default();
    let compiled_wasm_module_store = CompiledWasmModuleStore::default();

    let file_fetcher = FileFetcher::new(
      http_cache,
      cache_usage,
      !flags.no_remote,
      Some(root_cert_store.clone()),
      blob_store.clone(),
      flags.unsafely_ignore_certificate_errors.clone(),
    )?;

    let lockfile = if let Some(filename) = &flags.lock {
      let lockfile = Lockfile::new(filename.clone(), flags.lock_write)?;
      Some(Arc::new(Mutex::new(lockfile)))
    } else {
      None
    };

    let maybe_config_file = crate::config_file::discover(&flags)?;

    let maybe_import_map_specifier =
      crate::config_file::resolve_import_map_specifier(
        flags.import_map_path.as_deref(),
        maybe_config_file.as_ref(),
      )?;

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

    let maybe_inspect_host = flags.inspect.or(flags.inspect_brk);
    let maybe_inspector_server = maybe_inspect_host.map(|host| {
      Arc::new(InspectorServer::new(host, version::get_user_agent()))
    });

    let coverage_dir = flags
      .coverage_dir
      .clone()
      .or_else(|| env::var("DENO_UNSTABLE_COVERAGE_DIR").ok());

    // FIXME(bartlomieju): `NodeEsmResolver` is not aware of JSX resolver
    // created below
    let node_resolver = NodeEsmResolver::new(
      maybe_import_map.clone().map(ImportMapResolver::new),
    );
    let maybe_import_map_resolver =
      maybe_import_map.clone().map(ImportMapResolver::new);
    let maybe_jsx_resolver = maybe_config_file.as_ref().and_then(|cf| {
      cf.to_maybe_jsx_import_source_module()
        .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
    });
    let maybe_resolver: Option<
      Arc<dyn deno_graph::source::Resolver + Send + Sync>,
    > = if flags.compat {
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

    Ok(ProcState(Arc::new(Inner {
      dir,
      coverage_dir,
      flags,
      file_fetcher,
      graph_data: Default::default(),
      lockfile,
      maybe_config_file,
      maybe_import_map,
      maybe_inspector_server,
      root_cert_store: Some(root_cert_store.clone()),
      blob_store,
      broadcast_channel,
      shared_array_buffer_store,
      compiled_wasm_module_store,
      maybe_resolver,
    })))
  }

  /// Return any imports that should be brought into the scope of the module
  /// graph.
  fn get_maybe_imports(&self) -> MaybeImportsResult {
    let mut imports = Vec::new();
    if let Some(config_file) = &self.maybe_config_file {
      if let Some(config_imports) = config_file.to_maybe_imports()? {
        imports.extend(config_imports);
      }
    }
    if self.flags.compat {
      imports.extend(compat::get_node_imports());
    }
    if imports.is_empty() {
      Ok(None)
    } else {
      Ok(Some(imports))
    }
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate `self.graph_data` in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  pub(crate) async fn prepare_module_load(
    &self,
    roots: Vec<(ModuleSpecifier, ModuleKind)>,
    is_dynamic: bool,
    lib: emit::TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    reload_on_watch: bool,
  ) -> Result<(), AnyError> {
    // TODO(bartlomieju): this is very make-shift, is there an existing API
    // that we could include it like with "maybe_imports"?
    let roots = if self.flags.compat {
      let mut r = vec![(compat::GLOBAL_URL.clone(), ModuleKind::Esm)];
      r.extend(roots);
      r
    } else {
      roots
    };
    if !reload_on_watch {
      let graph_data = self.graph_data.read();
      if self.flags.check == flags::CheckFlag::None
        || graph_data.is_type_checked(&roots, &lib)
      {
        if let Some(result) = graph_data.check(
          &roots,
          self.flags.check != flags::CheckFlag::None,
          false,
        ) {
          return result;
        }
      }
    }
    let mut cache = cache::FetchCacher::new(
      self.dir.gen_cache.clone(),
      self.file_fetcher.clone(),
      root_permissions.clone(),
      dynamic_permissions.clone(),
    );
    let maybe_locker = as_maybe_locker(self.lockfile.clone());
    let maybe_imports = self.get_maybe_imports()?;
    let maybe_resolver: Option<&dyn deno_graph::source::Resolver> =
      if let Some(resolver) = &self.maybe_resolver {
        Some(resolver.as_ref())
      } else {
        None
      };

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
    let graph = create_graph(
      roots.clone(),
      is_dynamic,
      maybe_imports,
      &mut loader,
      maybe_resolver,
      maybe_locker,
      None,
      None,
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
      graph_data.entries().into_keys().cloned().collect()
    };

    {
      let mut graph_data = self.graph_data.write();
      graph_data.add_graph(&graph, reload_on_watch);
      let check_js = self
        .maybe_config_file
        .as_ref()
        .map(|cf| cf.get_check_js())
        .unwrap_or(false);
      graph_data
        .check(&roots, self.flags.check != flags::CheckFlag::None, check_js)
        .unwrap()?;
    }

    let config_type = if self.flags.check == flags::CheckFlag::None {
      emit::ConfigType::Emit
    } else {
      emit::ConfigType::Check {
        tsc_emit: true,
        lib: lib.clone(),
      }
    };

    let (ts_config, maybe_ignored_options) =
      emit::get_ts_config(config_type, self.maybe_config_file.as_ref(), None)?;

    if let Some(ignored_options) = maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }

    if self.flags.check == flags::CheckFlag::None {
      let options = emit::EmitOptions {
        ts_config,
        reload: self.flags.reload,
        reload_exclusions,
      };
      let emit_result = emit::emit(&graph, &mut cache, options)?;
      log::debug!("{}", emit_result.stats);
    } else {
      let maybe_config_specifier = self
        .maybe_config_file
        .as_ref()
        .map(|cf| cf.specifier.clone());
      let options = emit::CheckOptions {
        check: self.flags.check.clone(),
        debug: self.flags.log_level == Some(log::Level::Debug),
        emit_with_diagnostics: false,
        maybe_config_specifier,
        ts_config,
        log_checks: true,
        reload: self.flags.reload,
        reload_exclusions,
      };
      let emit_result = emit::check_and_maybe_emit(
        &roots,
        self.graph_data.clone(),
        &mut cache,
        options,
      )?;
      if !emit_result.diagnostics.is_empty() {
        return Err(anyhow!(emit_result.diagnostics));
      }
      log::debug!("{}", emit_result.stats);
    }

    if self.flags.check != flags::CheckFlag::None {
      let mut graph_data = self.graph_data.write();
      graph_data.set_type_checked(&roots, &lib);
    }

    // any updates to the lockfile should be updated now
    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock();
      g.write()?;
    }

    Ok(())
  }

  pub(crate) fn resolve(
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
    let referrer = if referrer.is_empty() && self.flags.repl {
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

  pub fn load(
    &self,
    specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Result<ModuleSource, AnyError> {
    log::debug!(
      "specifier: {} maybe_referrer: {} is_dynamic: {}",
      specifier,
      maybe_referrer
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<none>".to_string()),
      is_dynamic
    );

    let graph_data = self.graph_data.read();
    let found = graph_data.follow_redirect(&specifier);
    match graph_data.get(&found) {
      Some(ModuleEntry::Module {
        code, media_type, ..
      }) => {
        let code = match media_type {
          MediaType::JavaScript
          | MediaType::Unknown
          | MediaType::Cjs
          | MediaType::Mjs
          | MediaType::Json => {
            if let Some(source) = graph_data.get_cjs_esm_translation(&specifier)
            {
              source.to_owned()
            } else {
              code.as_ref().clone()
            }
          }
          MediaType::Dts => "".to_string(),
          _ => {
            let emit_path = self
              .dir
              .gen_cache
              .get_cache_filename_with_extension(&found, "js")
              .unwrap_or_else(|| {
                unreachable!("Unable to get cache filename: {}", &found)
              });
            match self.dir.gen_cache.get(&emit_path) {
              Ok(b) => String::from_utf8(b).unwrap(),
              Err(_) => unreachable!("Unexpected missing emit: {}", found),
            }
          }
        };
        Ok(ModuleSource {
          code,
          module_url_specified: specifier.to_string(),
          module_url_found: found.to_string(),
          module_type: match media_type {
            MediaType::Json => ModuleType::Json,
            _ => ModuleType::JavaScript,
          },
        })
      }
      _ => Err(anyhow!(
        "Loading unprepared module: {}",
        specifier.to_string()
      )),
    }
  }

  // TODO(@kitsonk) this should be refactored to get it from the module graph
  fn get_emit(&self, url: &Url) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    let emit_path = self
      .dir
      .gen_cache
      .get_cache_filename_with_extension(url, "js")?;
    let emit_map_path = self
      .dir
      .gen_cache
      .get_cache_filename_with_extension(url, "js.map")?;
    if let Ok(code) = self.dir.gen_cache.get(&emit_path) {
      let maybe_map = if let Ok(map) = self.dir.gen_cache.get(&emit_map_path) {
        Some(map)
      } else {
        None
      };
      Some((code, maybe_map))
    } else {
      None
    }
  }
}

// TODO(@kitsonk) this is only temporary, but should be refactored to somewhere
// else, like a refactored file_fetcher.
impl SourceMapGetter for ProcState {
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    if let Ok(specifier) = resolve_url(file_name) {
      match specifier.scheme() {
        // we should only be looking for emits for schemes that denote external
        // modules, which the disk_cache supports
        "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
        _ => return None,
      }
      if let Some((code, maybe_map)) = self.get_emit(&specifier) {
        let code = String::from_utf8(code).unwrap();
        source_map_from_code(code).or(maybe_map)
      } else if let Ok(source) = self.load(specifier, None, false) {
        source_map_from_code(source.code)
      } else {
        None
      }
    } else {
      None
    }
  }

  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    if let Ok(specifier) = resolve_url(file_name) {
      self.file_fetcher.get_source(&specifier).map(|out| {
        // Do NOT use .lines(): it skips the terminating empty line.
        // (due to internally using_terminator() instead of .split())
        let lines: Vec<&str> = out.source.split('\n').collect();
        if line_number >= lines.len() {
          format!(
            "{} Couldn't format source line: Line {} is out of bounds (source may have changed at runtime)",
            crate::colors::yellow("Warning"), line_number + 1,
          )
        } else {
          lines[line_number].to_string()
        }
      })
    } else {
      None
    }
  }
}

pub fn import_map_from_text(
  specifier: &Url,
  json_text: &str,
) -> Result<ImportMap, AnyError> {
  let result = parse_from_json(specifier, json_text)?;
  if !result.diagnostics.is_empty() {
    warn!(
      "Import map diagnostics:\n{}",
      result
        .diagnostics
        .into_iter()
        .map(|d| format!("  - {}", d))
        .collect::<Vec<_>>()
        .join("\n")
    )
  }
  Ok(result.import_map)
}

fn source_map_from_code(code: String) -> Option<Vec<u8>> {
  let lines: Vec<&str> = code.split('\n').collect();
  if let Some(last_line) = lines.last() {
    if last_line
      .starts_with("//# sourceMappingURL=data:application/json;base64,")
    {
      let input = last_line.trim_start_matches(
        "//# sourceMappingURL=data:application/json;base64,",
      );
      let decoded_map = base64::decode(input)
        .expect("Unable to decode source map from emitted file.");
      Some(decoded_map)
    } else {
      None
    }
  } else {
    None
  }
}
