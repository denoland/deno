// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::deno_dir;
use crate::file_fetcher::SourceFileFetcher;
use crate::flags;
use crate::http_cache;
use crate::import_map::ImportMap;
use crate::inspector::InspectorServer;
use crate::lockfile::Lockfile;
use crate::media_type::MediaType;
use crate::module_graph2::CheckOptions;
use crate::module_graph2::GraphBuilder2;
use crate::module_graph2::TranspileOptions;
use crate::module_graph2::TypeLib;
use crate::permissions::Permissions;
use crate::source_maps::SourceMapGetter;
use crate::specifier_handler::FetchHandler;
use crate::tsc::CompiledModule;
use crate::tsc::TargetLib;
use crate::tsc::TsCompiler;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use std::cell::RefCell;
use std::env;
use std::rc::Rc;
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
  /// Permissions parsed from `flags`.
  pub permissions: Permissions,
  pub dir: deno_dir::DenoDir,
  pub file_fetcher: SourceFileFetcher,
  pub ts_compiler: TsCompiler,
  pub lockfile: Option<Mutex<Lockfile>>,
  pub maybe_import_map: Option<ImportMap>,
  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
}

impl ProgramState {
  pub fn new(flags: flags::Flags) -> Result<Arc<Self>, AnyError> {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();
    let dir = deno_dir::DenoDir::new(custom_root)?;
    let deps_cache_location = dir.root.join("deps");
    let http_cache = http_cache::HttpCache::new(&deps_cache_location);
    let ca_file = flags.ca_file.clone().or_else(|| env::var("DENO_CERT").ok());

    let file_fetcher = SourceFileFetcher::new(
      http_cache,
      !flags.reload,
      flags.cache_blocklist.clone(),
      flags.no_remote,
      flags.cached_only,
      ca_file.as_deref(),
    )?;

    let ts_compiler = TsCompiler::new(
      file_fetcher.clone(),
      flags.clone(),
      dir.gen_cache.clone(),
    )?;

    let lockfile = if let Some(filename) = &flags.lock {
      let lockfile = Lockfile::new(filename.clone(), flags.lock_write)?;
      Some(Mutex::new(lockfile))
    } else {
      None
    };

    let maybe_import_map: Option<ImportMap> =
      match flags.import_map_path.as_ref() {
        None => None,
        Some(file_path) => {
          if !flags.unstable {
            exit_unstable("--import-map")
          }
          Some(ImportMap::load(file_path)?)
        }
      };

    let maybe_inspect_host = flags.inspect.or(flags.inspect_brk);
    let maybe_inspector_server = match maybe_inspect_host {
      Some(host) => Some(Arc::new(InspectorServer::new(host))),
      None => None,
    };

    let program_state = ProgramState {
      dir,
      permissions: Permissions::from_flags(&flags),
      flags,
      file_fetcher,
      ts_compiler,
      lockfile,
      maybe_import_map,
      maybe_inspector_server,
    };
    Ok(Arc::new(program_state))
  }

  /// This function is called when new module load is
  /// initialized by the JsRuntime. Its resposibility is to collect
  /// all dependencies and if it is required then also perform TS typecheck
  /// and traspilation.
  pub async fn prepare_module_load(
    self: &Arc<Self>,
    specifier: ModuleSpecifier,
    target_lib: TargetLib,
    dynamic_permissions: Permissions,
    is_dynamic: bool,
    maybe_import_map: Option<ImportMap>,
  ) -> Result<(), AnyError> {
    let specifier = specifier.clone();
    let handler =
      Rc::new(RefCell::new(FetchHandler::new(self, dynamic_permissions)?));
    let mut builder = GraphBuilder2::new(handler, maybe_import_map);
    builder.add(&specifier, is_dynamic).await?;
    let mut graph = builder.get_graph(&self.lockfile);
    let debug = self.flags.log_level == Some(log::Level::Debug);
    let maybe_config_path = self.flags.config_path.clone();

    if self.flags.no_check {
      let (stats, maybe_ignored_options) =
        graph.transpile(TranspileOptions {
          debug,
          maybe_config_path,
          reload: self.flags.reload,
        })?;
      debug!("{}", stats);
      if let Some(ignored_options) = maybe_ignored_options {
        eprintln!("{}", ignored_options);
      }
    } else {
      let lib = match target_lib {
        TargetLib::Main => {
          if self.flags.unstable {
            TypeLib::UnstableDenoWindow
          } else {
            TypeLib::DenoWindow
          }
        }
        TargetLib::Worker => {
          if self.flags.unstable {
            TypeLib::UnstableDenoWorker
          } else {
            TypeLib::DenoWorker
          }
        }
      };
      let (stats, diagnostics, maybe_ignored_options) =
        graph.check(CheckOptions {
          debug,
          emit: true,
          lib,
          maybe_config_path,
          reload: self.flags.reload,
        })?;

      debug!("{}", stats);
      if let Some(ignored_options) = maybe_ignored_options {
        eprintln!("{}", ignored_options);
      }
      if !diagnostics.0.is_empty() {
        return Err(generic_error(diagnostics.to_string()));
      }
    };

    if let Some(ref lockfile) = self.lockfile {
      let g = lockfile.lock().unwrap();
      g.write()?;
    }

    Ok(())
  }

  pub fn fetch_compiled_module(
    &self,
    module_specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<CompiledModule, AnyError> {
    let out = self
      .file_fetcher
      .fetch_cached_source_file(&module_specifier, Permissions::allow_all())
      .expect("Cached source file doesn't exist");

    let url = out.url.clone();
    let compiled_module = if let Some((code, _)) = self.get_emit(&url) {
      CompiledModule {
        code: String::from_utf8(code).unwrap(),
        name: out.url.to_string(),
      }
    // We expect a compiled source for any non-JavaScript files, except for
    // local files that have an unknown media type and no referrer (root modules
    // that do not have an extension.)
    } else if out.media_type != MediaType::JavaScript
      && !(out.media_type == MediaType::Unknown
        && maybe_referrer.is_none()
        && url.scheme() == "file")
    {
      let message = if let Some(referrer) = maybe_referrer {
        format!("Compiled module not found \"{}\"\n  From: {}\n    If the source module contains only types, use `import type` and `export type` to import it instead.", module_specifier, referrer)
      } else {
        format!("Compiled module not found \"{}\"\n  If the source module contains only types, use `import type` and `export type` to import it instead.", module_specifier)
      };
      info!("{}: {}", crate::colors::yellow("warning"), message);
      CompiledModule {
        code: "".to_string(),
        name: out.url.to_string(),
      }
    } else {
      CompiledModule {
        code: out.source_code,
        name: out.url.to_string(),
      }
    };

    Ok(compiled_module)
  }

  // TODO(@kitsonk) this should be a straight forward API on file_fetcher or
  // whatever future refactors do...
  fn get_emit(&self, url: &Url) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    match url.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" => (),
      _ => {
        return None;
      }
    }
    let emit_path = self
      .dir
      .gen_cache
      .get_cache_filename_with_extension(&url, "js");
    let emit_map_path = self
      .dir
      .gen_cache
      .get_cache_filename_with_extension(&url, "js.map");
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

  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  pub fn check_unstable(&self, api_name: &str) {
    if !self.flags.unstable {
      exit_unstable(api_name);
    }
  }

  #[cfg(test)]
  pub fn mock(
    argv: Vec<String>,
    maybe_flags: Option<flags::Flags>,
  ) -> Arc<ProgramState> {
    ProgramState::new(flags::Flags {
      argv,
      ..maybe_flags.unwrap_or_default()
    })
    .unwrap()
  }
}

// TODO(@kitsonk) this is only temporary, but should be refactored to somewhere
// else, like a refactored file_fetcher.
impl SourceMapGetter for ProgramState {
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    if let Ok(specifier) = ModuleSpecifier::resolve_url(file_name) {
      if let Some((code, maybe_map)) = self.get_emit(&specifier.as_url()) {
        if maybe_map.is_some() {
          maybe_map
        } else {
          let code = String::from_utf8(code).unwrap();
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
    if let Ok(specifier) = ModuleSpecifier::resolve_url(file_name) {
      self
        .file_fetcher
        .fetch_cached_source_file(&specifier, Permissions::allow_all())
        .map(|out| {
          // Do NOT use .lines(): it skips the terminating empty line.
          // (due to internally using .split_terminator() instead of .split())
          let lines: Vec<&str> = out.source_code.split('\n').collect();
          assert!(lines.len() > line_number);
          lines[line_number].to_string()
        })
    } else {
      None
    }
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(ProgramState::mock(vec![], None));
}
