// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_core::ModuleCodeString;
use deno_core::ModuleName;
use deno_core::SourceMapData;
use deno_error::JsErrorBox;

deno_error::js_error_wrapper!(
  deno_ast::ParseDiagnostic,
  JsParseDiagnostic,
  "Error"
);
deno_error::js_error_wrapper!(
  deno_ast::TranspileError,
  JsTranspileError,
  "Error"
);

pub fn maybe_transpile_source(
  name: ModuleName,
  source: ModuleCodeString,
) -> Result<(ModuleCodeString, Option<SourceMapData>), JsErrorBox> {
  // Always transpile `node:` built-in modules, since they might be TypeScript.
  let media_type = if name.starts_with("node:") {
    MediaType::TypeScript
  } else {
    MediaType::from_path(Path::new(&name))
  };

  match media_type {
    MediaType::TypeScript => {}
    MediaType::JavaScript => return Ok((source, None)),
    MediaType::Mjs => return Ok((source, None)),
    _ => panic!(
      "Unsupported media type for snapshotting {media_type:?} for file {}",
      name
    ),
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier: deno_core::url::Url::parse(&name).unwrap(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(|e| JsErrorBox::from_err(JsParseDiagnostic(e)))?;
  let transpiled_source = parsed
    .transpile(
      &deno_ast::TranspileOptions {
        imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &deno_ast::TranspileModuleOptions::default(),
      &deno_ast::EmitOptions {
        source_map: if cfg!(debug_assertions) {
          SourceMapOption::Separate
        } else {
          SourceMapOption::None
        },
        ..Default::default()
      },
    )
    .map_err(|e| JsErrorBox::from_err(JsTranspileError(e)))?
    .into_source();

  let maybe_source_map: Option<SourceMapData> = transpiled_source
    .source_map
    .map(|sm| sm.into_bytes().into());
  let source_text = transpiled_source.text;
  Ok((source_text.into(), maybe_source_map))
}
