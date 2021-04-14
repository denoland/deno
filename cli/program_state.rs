// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
use deno_runtime::deno_file::BlobUrlStore;
use deno_runtime::inspector::InspectorServer;
use deno_runtime::permissions::Permissions;

use deno_core::error::anyhow;
use deno_core::error::get_custom_error_class;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::resolve_url;
use deno_core::url::Url;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use log::debug;
use log::warn;
use std::collections::HashMap;
use std::env;
use std::fs::read;
use std::sync::Arc;
use std::sync::Mutex;

pub fn exit_unstable(api_name: &str) {
  eprintln!(
    "Unstable API '{}'. The --unstable flag must be provided.",
    api_name
  );
  std::process::exit(70);
}

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
  pub maybe_import_map: Option<ImportMap>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  pub ca_data: Option<Vec<u8>>,
  pub blob_url_store: BlobUrlStore,
}

impl ProgramState {
  pub async fn build(flags: flags::Flags) -> Result<Arc<Self>, AnyError> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root)?;
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);
    let ca_file = flags.ca_file.clone().or_else(|| env::var("DENO_CERT").ok());
    let ca_data = match &ca_file {
      Some(ca_file) => Some(read(ca_file).context("Failed to open ca file")?),
      None => None,
    };

    let cache_usage = if flags.cached_only {
      CacheSetting::Only
    } else if !flags.cache_blocklist.is_empty() {
      CacheSetting::ReloadSome(flags.cache_blocklist.clone())
    } else if flags.reload {
      CacheSetting::ReloadAll
    } else {
      CacheSetting::Use
    };

    let blob_url_store = BlobUrlStore::default();

    let file_fetcher = FileFetcher::new(
      http_cache,
      cache_usage,
      !flags.no_remote,
      ca_data.clone(),
      blob_url_store.clone(),
    )?;

    let lockfile = if let Some(filename) = &flags.lock {
      let lockfile = Lockfile::new(filename.clone(), flags.lock_write)?;
      Some(Arc::new(Mutex::new(lockfile)))
    } else {
      None
    };

    let maybe_import_map: Option<ImportMap> =
      match flags.import_map_path.as_ref() {
        None => None,
        Some(import_map_url) => {
          let import_map_specifier =
            deno_core::resolve_url_or_path(&import_map_url).context(
              format!("Bad URL (\"{}\") for import map.", import_map_url),
            )?;
          let file = file_fetcher
            .fetch(&import_map_specifier, &mut Permissions::allow_all())
            .await?;
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
      maybe_import_map,
      maybe_inspector_server,
      ca_data,
      blob_url_store,
    };
    Ok(Arc::new(program_state))
  }

  /// Prepares a set of module specifiers for loading in one shot.
  ///
  pub async fn prepare_module_graph(
    self: &Arc<Self>,
    specifiers: Vec<ModuleSpecifier>,
    lib: TypeLib,
    runtime_permissions: Permissions,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let handler = Arc::new(Mutex::new(FetchHandler::new(
      self,
      runtime_permissions.clone(),
    )?));

    let mut builder =
      GraphBuilder::new(handler, maybe_import_map, self.lockfile.clone());

    for specifier in specifiers {
      builder.add(&specifier, false).await?;
    }

    let mut graph = builder.get_graph();
    let debug = self.flags.log_level == Some(log::Level::Debug);
    let maybe_config_path = self.flags.config_path.clone();

    let result_modules = if self.flags.no_check {
      let result_info = graph.transpile(TranspileOptions {
        debug,
        maybe_config_path,
        reload: self.flags.reload,
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
        maybe_config_path,
        reload: self.flags.reload,
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

    let mut loadable_modules = self.modules.lock().unwrap();
    loadable_modules.extend(result_modules);

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock().unwrap();
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
    mut runtime_permissions: Permissions,
    is_dynamic: bool,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let specifier = specifier.clone();
    // Workers are subject to the current runtime permissions.  We do the
    // permission check here early to avoid "wasting" time building a module
    // graph for a module that cannot be loaded.
    if lib == TypeLib::DenoWorker || lib == TypeLib::UnstableDenoWorker {
      runtime_permissions.check_specifier(&specifier)?;
    }
    let handler =
      Arc::new(Mutex::new(FetchHandler::new(self, runtime_permissions)?));
    let mut builder =
      GraphBuilder::new(handler, maybe_import_map, self.lockfile.clone());
    builder.add(&specifier, is_dynamic).await?;
    let mut graph = builder.get_graph();
    let debug = self.flags.log_level == Some(log::Level::Debug);
    let maybe_config_path = self.flags.config_path.clone();

    let result_modules = if self.flags.no_check {
      let result_info = graph.transpile(TranspileOptions {
        debug,
        maybe_config_path,
        reload: self.flags.reload,
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
        maybe_config_path,
        reload: self.flags.reload,
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

    let mut loadable_modules = self.modules.lock().unwrap();
    loadable_modules.extend(result_modules);

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock().unwrap();
      g.write()?;
    }

    Ok(())
  }

  pub fn load(
    &self,
    specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<ModuleSource, AnyError> {
    let modules = self.modules.lock().unwrap();
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
      .get_cache_filename_with_extension(&url, "js")?;
    let emit_map_path = self
      .dir
      .gen_cache
      .get_cache_filename_with_extension(&url, "js.map")?;
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
