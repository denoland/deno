// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::TsTypeLib;
use crate::emit::emit_parsed_source;
use crate::graph_util::ModuleEntry;
use crate::node;
use crate::proc_state::ProcState;
use crate::util::text_encoding::code_without_source_map;
use crate::util::text_encoding::source_map_from_code;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::resolve_url;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::OpState;
use deno_core::SourceMapGetter;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

struct ModuleCodeSource {
  pub code: String,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

pub struct CliModuleLoader {
  pub lib: TsTypeLib,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. They are decoupled from the worker (dynamic) permissions since
  /// read access errors must be raised based on the parent thread permissions.
  pub root_permissions: Permissions,
  pub ps: ProcState,
}

impl CliModuleLoader {
  pub fn new(ps: ProcState) -> Rc<Self> {
    Rc::new(CliModuleLoader {
      lib: ps.options.ts_type_lib_window(),
      root_permissions: Permissions::allow_all(),
      ps,
    })
  }

  pub fn new_for_worker(ps: ProcState, permissions: Permissions) -> Rc<Self> {
    Rc::new(CliModuleLoader {
      lib: ps.options.ts_type_lib_worker(),
      root_permissions: permissions,
      ps,
    })
  }

  fn load_prepared_module(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<ModuleCodeSource, AnyError> {
    if specifier.as_str() == "node:module" {
      return Ok(ModuleCodeSource {
        code: deno_runtime::deno_node::MODULE_ES_SHIM.to_string(),
        found_url: specifier.to_owned(),
        media_type: MediaType::JavaScript,
      });
    }
    let graph_data = self.ps.graph_data.read();
    let found_url = graph_data.follow_redirect(specifier);
    match graph_data.get(&found_url) {
      Some(ModuleEntry::Module {
        code, media_type, ..
      }) => {
        let code = match media_type {
          MediaType::JavaScript
          | MediaType::Unknown
          | MediaType::Cjs
          | MediaType::Mjs
          | MediaType::Json => {
            if let Some(source) = graph_data.get_cjs_esm_translation(specifier)
            {
              source.to_owned()
            } else {
              code.to_string()
            }
          }
          MediaType::Dts | MediaType::Dcts | MediaType::Dmts => "".to_string(),
          MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Jsx
          | MediaType::Tsx => {
            // get emit text
            emit_parsed_source(
              &self.ps.emit_cache,
              &self.ps.parsed_source_cache,
              &found_url,
              *media_type,
              code,
              &self.ps.emit_options,
              self.ps.emit_options_hash,
            )?
          }
          MediaType::TsBuildInfo | MediaType::Wasm | MediaType::SourceMap => {
            panic!("Unexpected media type {} for {}", media_type, found_url)
          }
        };

        // at this point, we no longer need the parsed source in memory, so free it
        self.ps.parsed_source_cache.free(specifier);

        Ok(ModuleCodeSource {
          code,
          found_url,
          media_type: *media_type,
        })
      }
      _ => {
        let mut msg = format!("Loading unprepared module: {}", specifier);
        if let Some(referrer) = maybe_referrer {
          msg = format!("{}, imported from: {}", msg, referrer.as_str());
        }
        Err(anyhow!(msg))
      }
    }
  }

  fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<ModuleSource, AnyError> {
    let code_source = if self.ps.npm_resolver.in_npm_package(specifier) {
      let file_path = specifier.to_file_path().unwrap();
      let code = std::fs::read_to_string(&file_path).with_context(|| {
        let mut msg = "Unable to load ".to_string();
        msg.push_str(&file_path.to_string_lossy());
        if let Some(referrer) = &maybe_referrer {
          msg.push_str(" imported from ");
          msg.push_str(referrer.as_str());
        }
        msg
      })?;

      let code = if self.ps.cjs_resolutions.lock().contains(specifier) {
        // translate cjs to esm if it's cjs and inject node globals
        node::translate_cjs_to_esm(
          &self.ps.file_fetcher,
          specifier,
          code,
          MediaType::Cjs,
          &self.ps.npm_resolver,
          &self.ps.node_analysis_cache,
        )?
      } else {
        // only inject node globals for esm
        node::esm_code_with_node_globals(
          &self.ps.node_analysis_cache,
          specifier,
          code,
        )?
      };
      ModuleCodeSource {
        code,
        found_url: specifier.clone(),
        media_type: MediaType::from(specifier),
      }
    } else {
      self.load_prepared_module(specifier, maybe_referrer)?
    };
    let code = if self.ps.options.is_inspecting() {
      // we need the code with the source map in order for
      // it to work with --inspect or --inspect-brk
      code_source.code
    } else {
      // reduce memory and throw away the source map
      // because we don't need it
      code_without_source_map(code_source.code)
    };
    Ok(ModuleSource {
      code: code.into_bytes().into_boxed_slice(),
      module_url_specified: specifier.to_string(),
      module_url_found: code_source.found_url.to_string(),
      module_type: match code_source.media_type {
        MediaType::Json => ModuleType::Json,
        _ => ModuleType::JavaScript,
      },
    })
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.ps.resolve(specifier, referrer)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    // NOTE: this block is async only because of `deno_core` interface
    // requirements; module was already loaded when constructing module graph
    // during call to `prepare_load` so we can load it synchronously.
    Box::pin(deno_core::futures::future::ready(
      self.load_sync(specifier, maybe_referrer),
    ))
  }

  fn prepare_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    if self.ps.npm_resolver.in_npm_package(specifier) {
      // nothing to prepare
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    let specifier = specifier.clone();
    let ps = self.ps.clone();
    let state = op_state.borrow();

    let dynamic_permissions = state.borrow::<Permissions>().clone();
    let root_permissions = if is_dynamic {
      dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let lib = self.lib;

    drop(state);

    async move {
      ps.prepare_module_load(
        vec![specifier],
        is_dynamic,
        lib,
        root_permissions,
        dynamic_permissions,
        false,
      )
      .await
    }
    .boxed_local()
  }
}

impl SourceMapGetter for CliModuleLoader {
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    let specifier = resolve_url(file_name).ok()?;
    match specifier.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => return None,
    }
    let source = self.load_prepared_module(&specifier, None).ok()?;
    source_map_from_code(&source.code)
  }

  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let graph_data = self.ps.graph_data.read();
    let specifier = graph_data.follow_redirect(&resolve_url(file_name).ok()?);
    let code = match graph_data.get(&specifier) {
      Some(ModuleEntry::Module { code, .. }) => code,
      _ => return None,
    };
    // Do NOT use .lines(): it skips the terminating empty line.
    // (due to internally using_terminator() instead of .split())
    let lines: Vec<&str> = code.split('\n').collect();
    if line_number >= lines.len() {
      Some(format!(
        "{} Couldn't format source line: Line {} is out of bounds (source may have changed at runtime)",
        crate::colors::yellow("Warning"), line_number + 1,
      ))
    } else {
      Some(lines[line_number].to_string())
    }
  }
}
