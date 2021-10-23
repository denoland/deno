// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::colors;
use crate::compat;
use crate::compat::NodeEsmResolver;
use crate::config_file::ConfigFile;
use crate::deno_dir;
use crate::emit;
use crate::errors::get_module_graph_error_class;
use crate::file_fetcher::CacheSetting;
use crate::file_fetcher::FileFetcher;
use crate::flags;
use crate::http_cache;
use crate::lockfile::as_maybe_locker;
use crate::lockfile::Lockfile;
use crate::resolver::ImportMapResolver;
use crate::source_maps::SourceMapGetter;
use crate::version;

use deno_core::error::anyhow;
use deno_core::error::custom_error;
use deno_core::error::get_custom_error_class;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::url::Url;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::SharedArrayBufferStore;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::Permissions;
use deno_tls::rustls::RootCertStore;
use deno_tls::rustls_native_certs::load_native_certs;
use deno_tls::webpki_roots::TLS_SERVER_ROOTS;
use import_map::ImportMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[derive(Clone)]
pub struct ProcState(Arc<Inner>);

#[derive(Default)]
struct GraphData {
  modules: HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>>,
  // because the graph detects resolution issues early, but is build and dropped
  // during the `prepare_module_load` method, we need to extract out the module
  // resolution map so that those errors can be surfaced at the appropriate time
  resolution_map:
    HashMap<ModuleSpecifier, HashMap<String, deno_graph::Resolved>>,
  // in some cases we want to provide the range where the resolution error
  // occurred but need to surface it on load, but on load we don't know who the
  // referrer and span was, so we need to cache those
  resolved_map: HashMap<ModuleSpecifier, deno_graph::Range>,
  // deno_graph detects all sorts of issues at build time (prepare_module_load)
  // but if they are errors at that stage, the don't cause the correct behaviors
  // so we cache the error and then surface it when appropriate (e.g. load)
  maybe_graph_error: Option<deno_graph::ModuleGraphError>,
}

pub struct Inner {
  /// Flags parsed from `argv` contents.
  pub flags: flags::Flags,
  pub dir: deno_dir::DenoDir,
  pub coverage_dir: Option<String>,
  pub file_fetcher: FileFetcher,
  graph_data: Arc<Mutex<GraphData>>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub maybe_config_file: Option<ConfigFile>,
  pub maybe_import_map: Option<ImportMap>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub root_cert_store: Option<RootCertStore>,
  pub blob_store: BlobStore,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub shared_array_buffer_store: SharedArrayBufferStore,
  pub compiled_wasm_module_store: CompiledWasmModuleStore,
}

impl Deref for ProcState {
  type Target = Arc<Inner>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ProcState {
  pub async fn build(flags: flags::Flags) -> Result<Self, AnyError> {
    let maybe_custom_root = flags
      .cache_path
      .clone()
      .or_else(|| env::var("DENO_DIR").map(String::into).ok());
    let dir = deno_dir::DenoDir::new(maybe_custom_root)?;
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);

    let mut root_cert_store = RootCertStore::empty();
    let ca_stores: Vec<String> = flags
      .ca_stores
      .clone()
      .or_else(|| {
        let env_ca_store = env::var("DENO_TLS_CA_STORE").ok()?;
        Some(
          env_ca_store
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        )
      })
      .unwrap_or_else(|| vec!["mozilla".to_string()]);

    for store in ca_stores.iter() {
      match store.as_str() {
        "mozilla" => {
          root_cert_store.add_server_trust_anchors(&TLS_SERVER_ROOTS);
        }
        "system" => {
          let roots = load_native_certs()
            .expect("could not load platform certs")
            .roots;
          root_cert_store.roots.extend(roots);
        }
        _ => {
          return Err(anyhow!("Unknown certificate store \"{}\" specified (allowed: \"system,mozilla\")", store));
        }
      }
    }

    let ca_file = flags.ca_file.clone().or_else(|| env::var("DENO_CERT").ok());
    if let Some(ca_file) = ca_file {
      let certfile = File::open(&ca_file)?;
      let mut reader = BufReader::new(certfile);

      // This function does not return specific errors, if it fails give a generic message.
      if let Err(_err) = root_cert_store.add_pem_file(&mut reader) {
        return Err(anyhow!("Unable to add pem file to certificate store"));
      }
    }

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

    let maybe_config_file =
      if let Some(config_path) = flags.config_path.as_ref() {
        Some(ConfigFile::read(config_path)?)
      } else {
        None
      };

    let maybe_import_map: Option<ImportMap> =
      match flags.import_map_path.as_ref() {
        None => None,
        Some(import_map_url) => {
          let import_map_specifier =
            deno_core::resolve_url_or_path(import_map_url).context(format!(
              "Bad URL (\"{}\") for import map.",
              import_map_url
            ))?;
          let file = file_fetcher
            .fetch(&import_map_specifier, &mut Permissions::allow_all())
            .await
            .context(format!(
              "Unable to load '{}' import map",
              import_map_specifier
            ))?;
          let import_map =
            ImportMap::from_json(import_map_specifier.as_str(), &file.source)?;
          Some(import_map)
        }
      };

    let maybe_inspect_host = flags.inspect.or(flags.inspect_brk);
    let maybe_inspector_server = maybe_inspect_host.map(|host| {
      Arc::new(InspectorServer::new(host, version::get_user_agent()))
    });

    let coverage_dir = flags
      .coverage_dir
      .clone()
      .or_else(|| env::var("DENO_UNSTABLE_COVERAGE_DIR").ok());

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
    })))
  }

  pub(crate) fn take_graph_error(
    &self,
  ) -> Option<deno_graph::ModuleGraphError> {
    self.graph_data.lock().maybe_graph_error.take()
  }

  /// Return any imports that should be brought into the scope of the module
  /// graph.
  fn get_maybe_imports(&self) -> Option<Vec<(ModuleSpecifier, Vec<String>)>> {
    let mut imports = Vec::new();
    if let Some(config_file) = &self.maybe_config_file {
      if let Some(config_imports) = config_file.to_maybe_imports() {
        imports.extend(config_imports);
      }
    }
    if self.flags.compat {
      imports.extend(compat::get_node_imports());
    }
    if imports.is_empty() {
      None
    } else {
      Some(imports)
    }
  }

  /// This method is called when a module requested by the `JsRuntime` is not
  /// available, or in other sub-commands that need to "load" a module graph.
  /// The method will collect all the dependencies of the provided specifier,
  /// optionally checks their integrity, optionally type checks them, and
  /// ensures that any modules that needs to be transpiled is transpiled.
  ///
  /// It then populates the `loadable_modules` with what can be loaded into v8.
  pub(crate) async fn prepare_module_load(
    &self,
    roots: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    lib: emit::TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
  ) -> Result<(), AnyError> {
    let mut cache = cache::FetchCacher::new(
      self.dir.gen_cache.clone(),
      self.file_fetcher.clone(),
      root_permissions.clone(),
      dynamic_permissions.clone(),
    );
    let maybe_locker = as_maybe_locker(self.lockfile.clone());
    let maybe_imports = self.get_maybe_imports();
    let node_resolver = NodeEsmResolver;
    let import_map_resolver =
      self.maybe_import_map.as_ref().map(ImportMapResolver::new);
    let maybe_resolver = if self.flags.compat {
      Some(node_resolver.as_resolver())
    } else {
      import_map_resolver.as_ref().map(|im| im.as_resolver())
    };
    // TODO(bartlomieju): this is very make-shift, is there an existing API
    // that we could include it like with "maybe_imports"?
    let roots = if self.flags.compat {
      let mut r = vec![compat::GLOBAL_URL.clone()];
      r.extend(roots);
      r
    } else {
      roots
    };
    let graph = deno_graph::create_graph(
      roots,
      is_dynamic,
      maybe_imports,
      &mut cache,
      maybe_resolver,
      maybe_locker,
      None,
    )
    .await;
    // If there was a locker, validate the integrity of all the modules in the
    // locker.
    emit::lock(&graph);

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> = {
      let graph_data = self.graph_data.lock();
      graph_data.modules.keys().cloned().collect()
    };

    let config_type = if self.flags.no_check {
      emit::ConfigType::Emit
    } else {
      emit::ConfigType::Check {
        tsc_emit: true,
        lib,
      }
    };

    let (ts_config, maybe_ignored_options) =
      emit::get_ts_config(config_type, self.maybe_config_file.as_ref(), None)?;
    let graph = Arc::new(graph);

    // we will store this in proc state later, as if we were to return it from
    // prepare_load, some dynamic errors would not be catchable
    let maybe_graph_error = graph.valid().err();

    if emit::valid_emit(
      graph.as_ref(),
      &cache,
      &ts_config,
      self.flags.reload,
      &reload_exclusions,
    ) {
      if let Some(root) = graph.roots.get(0) {
        log::debug!("specifier \"{}\" and dependencies have valid emit, skipping checking and emitting", root);
      } else {
        log::debug!("rootless graph, skipping checking and emitting");
      }
    } else {
      if let Some(ignored_options) = maybe_ignored_options {
        log::warn!("{}", ignored_options);
      }
      let emit_result = if self.flags.no_check {
        let options = emit::EmitOptions {
          ts_config,
          reload_exclusions,
          reload: self.flags.reload,
        };
        emit::emit(graph.as_ref(), &mut cache, options)?
      } else {
        // here, we are type checking, so we want to error here if any of the
        // type only dependencies are missing or we have other errors with them
        // where as if we are not type checking, we shouldn't care about these
        // errors, and they don't get returned in `graph.valid()` above.
        graph.valid_types_only()?;

        let maybe_config_specifier = self
          .maybe_config_file
          .as_ref()
          .map(|cf| ModuleSpecifier::from_file_path(&cf.path).unwrap());
        let options = emit::CheckOptions {
          debug: self.flags.log_level == Some(log::Level::Debug),
          emit_with_diagnostics: true,
          maybe_config_specifier,
          ts_config,
        };
        for root in &graph.roots {
          let root_str = root.to_string();
          // `$deno$` specifiers are internal specifiers, printing out that
          // they are being checked is confusing to a user, since they don't
          // actually exist, so we will simply indicate that a generated module
          // is being checked instead of the cryptic internal module
          if !root_str.contains("$deno$") {
            log::info!("{} {}", colors::green("Check"), root);
          } else {
            log::info!("{} a generated module", colors::green("Check"))
          }
        }
        emit::check_and_maybe_emit(graph.clone(), &mut cache, options)?
      };
      log::debug!("{}", emit_result.stats);
      // if the graph is not valid then the diagnostics returned are bogus and
      // should just be ignored so that module loading can proceed to allow the
      // "real" error to be surfaced
      if !emit_result.diagnostics.is_empty() && maybe_graph_error.is_none() {
        return Err(anyhow!(emit_result.diagnostics));
      }
    }

    {
      let mut graph_data = self.graph_data.lock();
      // we iterate over the graph, looking for any modules that were emitted, or
      // should be loaded as their un-emitted source and add them to the in memory
      // cache of modules for loading by deno_core.
      graph_data
        .modules
        .extend(emit::to_module_sources(graph.as_ref(), &cache));

      // since we can't store the graph in proc state, because proc state needs to
      // be thread safe because of the need to provide source map resolution and
      // the graph needs to not be thread safe (due to wasmbind_gen constraints),
      // we have no choice but to extract out other meta data from the graph to
      // provide the correct loading behaviors for CLI
      graph_data.resolution_map.extend(graph.resolution_map());

      graph_data.maybe_graph_error = maybe_graph_error;
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
    if let Ok(s) = deno_core::resolve_url_or_path(referrer) {
      let maybe_resolved = {
        let graph_data = self.graph_data.lock();
        let resolved_specifier = graph_data
          .resolution_map
          .get(&s)
          .and_then(|map| map.get(specifier));
        resolved_specifier.cloned()
      };

      if let Some(resolved) = maybe_resolved {
        match resolved {
          Some(Ok((specifier, span))) => {
            let mut graph_data = self.graph_data.lock();
            graph_data.resolved_map.insert(specifier.clone(), span);
            return Ok(specifier);
          }
          Some(Err(err)) => {
            return Err(custom_error(
              "TypeError",
              format!("{}\n", err.to_string_with_range()),
            ))
          }
          _ => (),
        }
      }
    }

    // FIXME(bartlomieju): hacky way to provide compatibility with repl
    let referrer = if referrer.is_empty() && self.flags.repl {
      deno_core::DUMMY_SPECIFIER
    } else {
      referrer
    };
    if let Some(import_map) = &self.maybe_import_map {
      import_map
        .resolve(specifier, referrer)
        .map_err(|err| err.into())
    } else {
      deno_core::resolve_import(specifier, referrer).map_err(|err| err.into())
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

    {
      let graph_data = self.graph_data.lock();
      if let Some(module_result) = graph_data.modules.get(&specifier) {
        if let Ok(module_source) = module_result {
          return Ok(module_source.clone());
        }
      } else {
        if maybe_referrer.is_some() && !is_dynamic {
          if let Some(span) = graph_data.resolved_map.get(&specifier) {
            return Err(custom_error(
              "NotFound",
              format!("Cannot load module \"{}\".\n    at {}", specifier, span),
            ));
          }
        }
        return Err(custom_error(
          "NotFound",
          format!("Cannot load module \"{}\".", specifier),
        ));
      }
    }

    // If we're this far it means that there was an error for this module load.
    let mut graph_data = self.graph_data.lock();
    let err = graph_data
      .modules
      .get(&specifier)
      .unwrap()
      .as_ref()
      .unwrap_err();
    // this is the "pending" error we will return
    let err = if let Some(error_class) = get_custom_error_class(err) {
      if error_class == "NotFound" && maybe_referrer.is_some() && !is_dynamic {
        // in situations where we were to try to load a module that wasn't
        // emitted and we can't run the original source code (it isn't)
        // JavaScript, we will load a blank module instead.  This is
        // usually caused by people exporting type only exports and not
        // type checking.
        if let Some(span) = graph_data.resolved_map.get(&specifier) {
          log::warn!("{}: Cannot load module \"{}\".\n    at {}\n  If the source module contains only types, use `import type` and `export type` to import it instead.", colors::yellow("warning"), specifier, span);
          return Ok(ModuleSource {
            code: "".to_string(),
            module_url_found: specifier.to_string(),
            module_url_specified: specifier.to_string(),
          });
        }
      }
      custom_error(error_class, err.to_string())
    } else {
      anyhow!(err.to_string())
    };
    // if there is a pending graph error though we haven't returned, we
    // will return that one
    if let Some(graph_error) = graph_data.maybe_graph_error.take() {
      log::debug!("returning cached graph error");
      if let Some(span) = graph_data.resolved_map.get(&specifier) {
        if !span.specifier.as_str().contains("$deno") {
          return Err(custom_error(
            get_module_graph_error_class(&graph_error),
            format!("{}\n    at {}", graph_error, span),
          ));
        }
      }
      Err(graph_error.into())
    } else {
      Err(err)
    }
  }

  // TODO(@kitsonk) this should be refactored to get it from the module graph
  fn get_emit(&self, url: &Url) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    match url.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => {
        return None;
      }
    }
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
        // (due to internally using .split_terminator() instead of .split())
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
