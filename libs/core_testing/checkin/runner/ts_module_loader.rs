// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::ModuleCodeBytes;
use deno_core::ModuleCodeString;
use deno_core::ModuleLoadOptions;
use deno_core::ModuleLoadReferrer;
use deno_core::ModuleLoadResponse;
use deno_core::ModuleLoader;
use deno_core::ModuleName;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceMapData;
use deno_core::error::ModuleLoaderError;
use deno_core::resolve_import;
use deno_core::url::Url;
use deno_error::JsErrorBox;

// TODO(bartlomieju): this is duplicated in `core/examples/ts_modules_loader.rs`.
type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

// TODO(bartlomieju): this is duplicated in `core/examples/ts_modules_loader.rs`.
#[derive(Default)]
pub struct TypescriptModuleLoader {
  source_maps: SourceMapStore,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Trying to load {path:?} for {module_specifier}")]
struct AttemptedLoadError {
  path: PathBuf,
  module_specifier: ModuleSpecifier,
  #[source]
  #[inherit]
  source: std::io::Error,
}

// TODO(bartlomieju): this is duplicated in `core/examples/ts_modules_loader.rs`.
impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let source_maps = self.source_maps.clone();
    fn load(
      source_maps: SourceMapStore,
      module_specifier: &ModuleSpecifier,
      requested_module_type: RequestedModuleType,
    ) -> Result<ModuleSource, ModuleLoaderError> {
      let root = Path::new(env!("CARGO_MANIFEST_DIR"));
      let start = if module_specifier.scheme() == "test" {
        1
      } else {
        0
      };
      let path = if module_specifier.scheme() == "file" {
        module_specifier.to_file_path().unwrap()
      } else {
        root.join(Path::new(&module_specifier.path()[start..]))
      };
      if matches!(
        requested_module_type,
        RequestedModuleType::Bytes
          | RequestedModuleType::Text
          | RequestedModuleType::Other(_)
      ) {
        let bytes = fs::read(path).map_err(JsErrorBox::from_err)?;
        return Ok(ModuleSource::new(
          match requested_module_type {
            RequestedModuleType::Bytes => ModuleType::Bytes,
            RequestedModuleType::Text => ModuleType::Text,
            RequestedModuleType::Other(ty) => ModuleType::Other(ty),
            _ => unreachable!(),
          },
          ModuleSourceCode::Bytes(ModuleCodeBytes::Boxed(bytes.into())),
          module_specifier,
          None,
        ));
      }

      let media_type = MediaType::from_path(&path);
      let (module_type, should_transpile) = match MediaType::from_path(&path) {
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
          (ModuleType::JavaScript, false)
        }
        MediaType::Jsx => (ModuleType::JavaScript, true),
        MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => (ModuleType::JavaScript, true),
        MediaType::Json => (ModuleType::Json, false),
        MediaType::Wasm => (ModuleType::Wasm, false),
        _ => {
          if path.extension().unwrap_or_default() == "nocompile" {
            (ModuleType::JavaScript, false)
          } else {
            return Err(JsErrorBox::generic(format!(
              "Unknown extension {:?}",
              path.extension()
            )));
          }
        }
      };
      let code = if should_transpile {
        let code = std::fs::read_to_string(&path).map_err(|source| {
          JsErrorBox::from_err(AttemptedLoadError {
            path,
            module_specifier: module_specifier.clone(),
            source,
          })
        })?;
        let parsed = deno_ast::parse_module(ParseParams {
          specifier: module_specifier.clone(),
          text: code.into(),
          media_type,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })
        .map_err(JsErrorBox::from_err)?;
        let res = parsed
          .transpile(
            &deno_ast::TranspileOptions {
              imports_not_used_as_values:
                deno_ast::ImportsNotUsedAsValues::Remove,
              decorators: deno_ast::DecoratorsTranspileOption::Ecma,
              ..Default::default()
            },
            &deno_ast::TranspileModuleOptions { module_kind: None },
            &deno_ast::EmitOptions {
              source_map: SourceMapOption::Separate,
              inline_sources: false,
              ..Default::default()
            },
          )
          .map_err(JsErrorBox::from_err)?;
        let res = res.into_source();
        let source_map = res.source_map.unwrap().into_bytes();
        source_maps
          .borrow_mut()
          .insert(module_specifier.to_string(), source_map);
        ModuleSourceCode::String(res.text.into())
      } else {
        let code = std::fs::read(&path).map_err(|source| {
          JsErrorBox::from_err(AttemptedLoadError {
            path,
            module_specifier: module_specifier.clone(),
            source,
          })
        })?;
        ModuleSourceCode::Bytes(code.into_boxed_slice().into())
      };
      Ok(ModuleSource::new(module_type, code, module_specifier, None))
    }

    ModuleLoadResponse::Sync(load(
      source_maps,
      module_specifier,
      options.requested_module_type,
    ))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| v.clone().into())
  }
}

pub fn maybe_transpile_source(
  specifier: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  // Always transpile `checkin:` built-in modules, since they might be TypeScript.
  let media_type = if specifier.starts_with("checkin:") {
    MediaType::TypeScript
  } else {
    MediaType::from_path(Path::new(&specifier))
  };

  match media_type {
    MediaType::TypeScript => {}
    MediaType::JavaScript => return Ok((source, None)),
    MediaType::Mjs => return Ok((source, None)),
    _ => panic!(
      "Unsupported media type for snapshotting {media_type:?} for file {}",
      specifier
    ),
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: Url::parse(&specifier).unwrap(),
    text: source.as_str().into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(JsErrorBox::from_err)?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        decorators: deno_ast::DecoratorsTranspileOption::LegacyTypeScript {
          emit_metadata: true,
        },
        ..Default::default()
      },
      &deno_ast::TranspileModuleOptions { module_kind: None },
      &deno_ast::EmitOptions {
        source_map: SourceMapOption::Separate,
        inline_sources: false,
        ..Default::default()
      },
    )
    .map_err(JsErrorBox::from_err)?
    .into_source();

  Ok((
    transpiled_source.text.into(),
    transpiled_source.source_map.map(|s| s.into_bytes().into()),
  ))
}
