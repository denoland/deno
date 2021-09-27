// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::colors;
use crate::config_file::ConfigFile;
use crate::deno_dir;
use crate::emit;
use crate::file_fetcher::CacheSetting;
use crate::file_fetcher::FileFetcher;
use crate::flags;
use crate::http_cache;
use crate::lockfile::Locker;
use crate::lockfile::Lockfile;
use crate::source_maps::SourceMapGetter;
use crate::version;

use deno_core::error::anyhow;
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
use log::debug;
use log::info;
use log::warn;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
#[derive(Clone)]
pub struct ProcState(Arc<Inner>);

pub struct Inner {
  /// Flags parsed from `argv` contents.
  pub flags: flags::Flags,
  pub dir: deno_dir::DenoDir,
  pub coverage_dir: Option<String>,
  pub file_fetcher: FileFetcher,
  pub modules:
    Arc<Mutex<HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>>>>,
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

    let mut maybe_import_map: Option<ImportMap> =
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

    if flags.compat {
      let mut import_map = match maybe_import_map {
        Some(import_map) => import_map,
        None => {
          // INFO: we're creating an empty import map, with its specifier pointing
          // to `CWD/node_import_map.json` to make sure the map still works as expected.
          let import_map_specifier =
            std::env::current_dir()?.join("node_import_map.json");
          ImportMap::from_json(import_map_specifier.to_str().unwrap(), "{}")
            .unwrap()
        }
      };
      let node_builtins = crate::compat::get_mapped_node_builtins();
      let diagnostics = import_map.update_imports(node_builtins)?;

      if !diagnostics.is_empty() {
        info!("Some Node built-ins were not added to the import map:");
        for diagnostic in diagnostics {
          info!("  - {}", diagnostic);
        }
        info!("If you want to use Node built-ins provided by Deno remove listed specifiers from \"imports\" mapping in the import map file.");
      }

      maybe_import_map = Some(import_map);
    }

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
      modules: Default::default(),
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

  /// A method the builds a module graph, optionally types check it, check the
  /// integrity of the modules, and loads all modules into the proc state to be
  /// available for loading.
  pub(crate) async fn build_and_emit_graph(
    &self,
    roots: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    lib: emit::TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let mut cache = cache::FetchCacher::new(
      self.dir.gen_cache.clone(),
      self.file_fetcher.clone(),
      root_permissions,
      dynamic_permissions,
    );
    let maybe_locker = self.lockfile.as_ref().map(|lf| {
      Rc::new(RefCell::new(
        Box::new(Locker(lf.clone())) as Box<dyn deno_graph::source::Locker>
      ))
    });
    let graph = deno_graph::create_graph(
      roots,
      is_dynamic,
      &mut cache,
      maybe_import_map
        .as_ref()
        .map(|r| r as &dyn deno_graph::source::Resolver),
      maybe_locker,
      None,
    )
    .await;
    // Ensure that all non-dynamic imports are properly loaded and if not, error
    // with the first issue encountered.
    graph.valid()?;
    // If there was a locker, validate the integrity of all the modules in the
    // locker.
    graph.lock()?;

    let reload_exclusions: HashSet<ModuleSpecifier> = {
      let modules = self.modules.lock();
      modules.keys().cloned().collect()
    };

    let config_type = if self.flags.no_check {
      emit::ConfigType::Emit
    } else {
      emit::ConfigType::Check { emit: true, lib }
    };
    let (ts_config, maybe_ignored_options) =
      emit::get_ts_config(config_type, &self.maybe_config_file)?;
    let graph = Arc::new(graph);
    if emit::valid_emit(
      graph.clone(),
      &cache,
      &ts_config,
      self.flags.reload,
      &reload_exclusions,
    ) {
      debug!("specifier \"{}\" and dependencies have valid emit, skipping checking and emitting", graph.roots[0]);
    } else {
      if let Some(ignored_options) = maybe_ignored_options {
        eprintln!("{}", ignored_options);
      }
      let emit_result = if self.flags.no_check {
        let options = emit::EmitOptions {
          ts_config,
          reload_exclusions,
          reload: self.flags.reload,
        };
        emit::emit(graph.clone(), &mut cache, options)?
      } else {
        let maybe_config_specifier = self
          .maybe_config_file
          .clone()
          .map(|ref cf| ModuleSpecifier::from_file_path(&cf.path).unwrap());
        let options = emit::CheckOptions {
          debug: self.flags.log_level == Some(log::Level::Debug),
          maybe_config_specifier,
          ts_config,
        };
        for root in &graph.roots {
          log::info!("{} {}", colors::green("Check"), root);
        }
        emit::check_and_maybe_emit(graph.clone(), &mut cache, options)?
      };
      debug!("{}", emit_result.stats);
      if !emit_result.diagnostics.is_empty() {
        return Err(anyhow!(emit_result.diagnostics));
      }
    }

    let mut loadable_modules = self.modules.lock();
    loadable_modules.extend(emit::to_module_sources(graph, &cache));

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock();
      g.write()?;
    }

    Ok(())
  }

  /// This method is called when a module requested by the `JsRuntime` is not
  /// available. The method will collect all the dependencies of the provided
  /// specifier, optionally checks their integrity, optionally type checks them,
  /// and ensures that any modules that needs to be transpiled is transpiled.
  ///
  /// It then populates the `loadable_modules` with what can be loaded into v8.
  pub(crate) async fn prepare_module_load(
    &self,
    root_specifier: ModuleSpecifier,
    lib: emit::TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    is_dynamic: bool,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    self
      .build_and_emit_graph(
        vec![root_specifier],
        is_dynamic,
        lib,
        root_permissions,
        dynamic_permissions,
        maybe_import_map,
      )
      .await
  }

  pub fn load(
    &self,
    specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<ModuleSource, AnyError> {
    let modules = self.modules.lock();
    modules
      .get(&specifier)
      .map(|r| match r {
        Ok(module_source) => Ok(module_source.clone()),
        Err(err) => {
          // TODO(@kitsonk) this feels a bit hacky but it works, without
          // introducing another enum to have to try to deal with.
          if get_custom_error_class(err) == Some("NotFound") {
            let message = if let Some(referrer) = &maybe_referrer {
              format!("{}\n  From: {}\n    If the source module contains only types, use `import type` and `export type` to import it instead.", err, referrer)
            } else {
              format!("{}\n  If the source module contains only types, use `import type` and `export type` to import it instead.", err)
            };
            warn!("{}: {}", crate::colors::yellow("warning"), message);
            Ok(ModuleSource {
              code: "".to_string(),
              module_url_found: specifier.to_string(),
              module_url_specified: specifier.to_string(),
            })
          } else {
            // anyhow errors don't support cloning, so we have to manage this
            // ourselves
            Err(anyhow!(err.to_string()))
          }
        },
      })
      .unwrap_or_else(|| {
        if let Some(referrer) = maybe_referrer {
          Err(anyhow!(
            "Module \"{}\" is missing from the graph.\n  From: {}",
            specifier,
            referrer
          ))
        } else {
          Err(anyhow!(
            "Module \"{}\" is missing from the graph.",
            specifier
          ))
        }
      })
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
      } else if let Ok(source) = self.load(specifier, None) {
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
        assert!(lines.len() > line_number);
        lines[line_number].to_string()
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
