// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::TsTypeLib;
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
use deno_core::ModuleCode;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::OpState;
use deno_core::ResolutionKind;
use deno_core::SourceMapGetter;
use deno_graph::EsmModule;
use deno_graph::JsonModule;
use deno_runtime::permissions::PermissionsContainer;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

struct ModuleCodeSource {
  pub code: ModuleCode,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

pub struct CliModuleLoader {
  pub lib: TsTypeLib,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. These are "allow all" for main worker, and parent thread
  /// permissions for Web Worker.
  pub root_permissions: PermissionsContainer,
  /// Permissions used to resolve dynamic imports, these get passed as
  /// "root permissions" for Web Worker.
  dynamic_permissions: PermissionsContainer,
  pub ps: ProcState,
}

impl CliModuleLoader {
  pub fn new(
    ps: ProcState,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<Self> {
    Rc::new(CliModuleLoader {
      lib: ps.options.ts_type_lib_window(),
      root_permissions,
      dynamic_permissions,
      ps,
    })
  }

  pub fn new_for_worker(
    ps: ProcState,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<Self> {
    Rc::new(CliModuleLoader {
      lib: ps.options.ts_type_lib_worker(),
      root_permissions,
      dynamic_permissions,
      ps,
    })
  }

  fn load_prepared_module(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
  ) -> Result<ModuleCodeSource, AnyError> {
    if specifier.scheme() == "node" {
      unreachable!(); // Node built-in modules should be handled internally.
    }

    let graph = self.ps.graph();
    match graph.get(specifier) {
      Some(deno_graph::Module::Json(JsonModule {
        source,
        media_type,
        specifier,
        ..
      })) => Ok(ModuleCodeSource {
        code: source.clone().into(),
        found_url: specifier.clone(),
        media_type: *media_type,
      }),
      Some(deno_graph::Module::Esm(EsmModule {
        source,
        media_type,
        specifier,
        ..
      })) => {
        let code: ModuleCode = match media_type {
          MediaType::JavaScript
          | MediaType::Unknown
          | MediaType::Cjs
          | MediaType::Mjs
          | MediaType::Json => source.clone().into(),
          MediaType::Dts | MediaType::Dcts | MediaType::Dmts => {
            Default::default()
          }
          MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Jsx
          | MediaType::Tsx => {
            // get emit text
            self.ps.emitter.emit_parsed_source(
              specifier,
              *media_type,
              source,
            )?
          }
          MediaType::TsBuildInfo | MediaType::Wasm | MediaType::SourceMap => {
            panic!("Unexpected media type {media_type} for {specifier}")
          }
        };

        // at this point, we no longer need the parsed source in memory, so free it
        self.ps.parsed_source_cache.free(specifier);

        Ok(ModuleCodeSource {
          code,
          found_url: specifier.clone(),
          media_type: *media_type,
        })
      }
      _ => {
        let mut msg = format!("Loading unprepared module: {specifier}");
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
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
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
        let mut permissions = if is_dynamic {
          self.dynamic_permissions.clone()
        } else {
          self.root_permissions.clone()
        };
        // translate cjs to esm if it's cjs and inject node globals
        node::translate_cjs_to_esm(
          &self.ps.file_fetcher,
          specifier,
          code,
          MediaType::Cjs,
          &self.ps.npm_resolver,
          &self.ps.node_analysis_cache,
          &mut permissions,
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
        code: code.into(),
        found_url: specifier.clone(),
        media_type: MediaType::from_specifier(specifier),
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
    Ok(ModuleSource::new_with_redirect(
      match code_source.media_type {
        MediaType::Json => ModuleType::Json,
        _ => ModuleType::JavaScript,
      },
      code,
      specifier,
      &code_source.found_url,
    ))
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    let mut permissions = if matches!(kind, ResolutionKind::DynamicImport) {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    self.ps.resolve(specifier, referrer, &mut permissions)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    // NOTE: this block is async only because of `deno_core` interface
    // requirements; module was already loaded when constructing module graph
    // during call to `prepare_load` so we can load it synchronously.
    Box::pin(deno_core::futures::future::ready(self.load_sync(
      specifier,
      maybe_referrer,
      is_dynamic,
    )))
  }

  fn prepare_load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
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

    let dynamic_permissions = self.dynamic_permissions.clone();
    let root_permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let lib = self.lib;

    async move {
      ps.prepare_module_load(
        vec![specifier],
        is_dynamic,
        lib,
        root_permissions,
        dynamic_permissions,
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
    let graph = self.ps.graph();
    let code = match graph.get(&resolve_url(file_name).ok()?) {
      Some(deno_graph::Module::Esm(module)) => &module.source,
      Some(deno_graph::Module::Json(module)) => &module.source,
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
