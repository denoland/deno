// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::EmitOptions;
use deno_ast::ImportsNotUsedAsValues;
use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceMapOption;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_ast::TypeStripOptions;
use deno_core::op2;
use deno_core::url::Url;
use deno_error::JsErrorBox;

deno_error::js_error_wrapper!(
  deno_ast::ParseDiagnostic,
  JsParseDiagnostic,
  "SyntaxError"
);
deno_error::js_error_wrapper!(
  deno_ast::TranspileError,
  JsTranspileError,
  "Error"
);
deno_error::js_error_wrapper!(
  deno_ast::TypeStripError,
  JsTypeStripError,
  "SyntaxError"
);

#[op2]
#[string]
pub fn op_node_strip_typescript_types(
  #[string] code: String,
  #[string] mode: &str,
  source_map: bool,
) -> Result<String, JsErrorBox> {
  let specifier = Url::parse("file:///stripTypeScriptTypes.ts").unwrap();
  if mode == "strip" {
    return deno_ast::type_strip(
      &specifier,
      code,
      TypeStripOptions { module: None },
    )
    .map_err(|e| JsErrorBox::from_err(JsTypeStripError(e)));
  }

  let parsed = deno_ast::parse_module(ParseParams {
    specifier,
    text: code.into(),
    media_type: MediaType::TypeScript,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })
  .map_err(|e| JsErrorBox::from_err(JsParseDiagnostic(e)))?;

  let source = parsed
    .transpile(
      &TranspileOptions {
        imports_not_used_as_values: ImportsNotUsedAsValues::Remove,
        ..Default::default()
      },
      &TranspileModuleOptions { module_kind: None },
      &EmitOptions {
        source_map: if source_map {
          SourceMapOption::Inline
        } else {
          SourceMapOption::None
        },
        ..Default::default()
      },
    )
    .map_err(|e| JsErrorBox::from_err(JsTranspileError(e)))?
    .into_source();

  Ok(source.text)
}
