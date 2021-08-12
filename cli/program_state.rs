// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::config_file::ConfigFile;
use crate::deno_dir;
use crate::file_fetcher::CacheSetting;
use crate::file_fetcher::FileFetcher;
use crate::flags;
use crate::http_cache;
use crate::import_map::ImportMap;
use crate::lockfile::Lockfile;
use crate::module_graph::CheckOptions;
use crate::module_graph::GraphBuilder;
use crate::module_graph::TranspileOptions;
use crate::module_graph::TypeLib;
use crate::source_maps::SourceMapGetter;
use crate::specifier_handler::FetchHandler;
use crate::version;
use deno_core::SharedArrayBufferStore;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::permissions::Permissions;

use deno_core::error::anyhow;
use deno_core::error::get_custom_error_class;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::url::Url;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_tls::rustls::RootCertStore;
use deno_tls::rustls_native_certs::load_native_certs;
use deno_tls::webpki_roots::TLS_SERVER_ROOTS;
use log::debug;
use log::warn;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

/// This structure represents state of single "deno" program.
///
/// It is shared by all created workers (thus V8 isolates).
pub struct ProgramState {
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
}

impl ProgramState {
  pub async fn build(flags: flags::Flags) -> Result<Arc<Self>, AnyError> {
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

    let program_state = ProgramState {
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
    };
    Ok(Arc::new(program_state))
  }

  /// Prepares a set of module specifiers for loading in one shot.
  ///
  pub async fn prepare_module_graph(
    self: &Arc<Self>,
    specifiers: Vec<ModuleSpecifier>,
    lib: TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let handler = Arc::new(Mutex::new(FetchHandler::new(
      self,
      root_permissions,
      dynamic_permissions,
    )?));

    let mut builder =
      GraphBuilder::new(handler, maybe_import_map, self.lockfile.clone());

    for specifier in specifiers {
      builder.add(&specifier, false).await?;
    }
    builder.analyze_config_file(&self.maybe_config_file).await?;

    let mut graph = builder.get_graph();
    let debug = self.flags.log_level == Some(log::Level::Debug);
    let maybe_config_file = self.maybe_config_file.clone();
    let reload_exclusions = {
      let modules = self.modules.lock();
      modules.keys().cloned().collect::<HashSet<_>>()
    };

    let result_modules = if self.flags.no_check {
      let result_info = graph.transpile(TranspileOptions {
        debug,
        maybe_config_file,
        reload: self.flags.reload,
        reload_exclusions,
      })?;
      debug!("{}", result_info.stats);
      if let Some(ignored_options) = result_info.maybe_ignored_options {
        warn!("{}", ignored_options);
      }
      result_info.loadable_modules
    } else {
      let result_info = graph.check(CheckOptions {
        debug,
        emit: true,
        lib,
        maybe_config_file,
        reload: self.flags.reload,
        reload_exclusions,
      })?;

      debug!("{}", result_info.stats);
      if let Some(ignored_options) = result_info.maybe_ignored_options {
        eprintln!("{}", ignored_options);
      }
      if !result_info.diagnostics.is_empty() {
        return Err(anyhow!(result_info.diagnostics));
      }
      result_info.loadable_modules
    };

    let mut loadable_modules = self.modules.lock();
    loadable_modules.extend(result_modules);

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock();
      g.write()?;
    }

    Ok(())
  }

  /// This function is called when new module load is
  /// initialized by the JsRuntime. Its resposibility is to collect
  /// all dependencies and if it is required then also perform TS typecheck
  /// and traspilation.
  pub async fn prepare_module_load(
    self: &Arc<Self>,
    specifier: ModuleSpecifier,
    lib: TypeLib,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
    is_dynamic: bool,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let specifier = specifier.clone();
    let handler = Arc::new(Mutex::new(FetchHandler::new(
      self,
      root_permissions,
      dynamic_permissions,
    )?));
    let mut builder =
      GraphBuilder::new(handler, maybe_import_map, self.lockfile.clone());
    builder.add(&specifier, is_dynamic).await?;
    builder.analyze_config_file(&self.maybe_config_file).await?;
    let mut graph = builder.get_graph();
    let debug = self.flags.log_level == Some(log::Level::Debug);
    let maybe_config_file = self.maybe_config_file.clone();
    let reload_exclusions = {
      let modules = self.modules.lock();
      modules.keys().cloned().collect::<HashSet<_>>()
    };

    let result_modules = if self.flags.no_check {
      let result_info = graph.transpile(TranspileOptions {
        debug,
        maybe_config_file,
        reload: self.flags.reload,
        reload_exclusions,
      })?;
      debug!("{}", result_info.stats);
      if let Some(ignored_options) = result_info.maybe_ignored_options {
        warn!("{}", ignored_options);
      }
      result_info.loadable_modules
    } else {
      let result_info = graph.check(CheckOptions {
        debug,
        emit: true,
        lib,
        maybe_config_file,
        reload: self.flags.reload,
        reload_exclusions,
      })?;

      debug!("{}", result_info.stats);
      if let Some(ignored_options) = result_info.maybe_ignored_options {
        eprintln!("{}", ignored_options);
      }
      if !result_info.diagnostics.is_empty() {
        return Err(anyhow!(result_info.diagnostics));
      }
      result_info.loadable_modules
    };

    let mut loadable_modules = self.modules.lock();
    loadable_modules.extend(result_modules);

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock();
      g.write()?;
    }

    Ok(())
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
impl SourceMapGetter for ProgramState {
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
