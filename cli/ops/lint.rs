// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnostic;
use deno_core::op2;

use crate::tools::lint;

deno_core::extension!(deno_lint, ops = [op_lint_create_serialized_ast,],);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LintError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  ParseDiagnostic(#[from] ParseDiagnostic),
  #[class(type)]
  #[error("Failed to parse path as URL: {0}")]
  PathParse(std::path::PathBuf),
}

#[op2]
#[buffer]
fn op_lint_create_serialized_ast(
  #[string] file_name: &str,
  #[string] source: String,
) -> Result<Vec<u8>, LintError> {
  let file_text = deno_ast::strip_bom(source);
  let path = std::env::current_dir()?.join(file_name);
  let specifier = ModuleSpecifier::from_file_path(&path)
    .map_err(|_| LintError::PathParse(path))?;
  let media_type = MediaType::from_specifier(&specifier);
  let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
    specifier,
    text: file_text.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  Ok(lint::serialize_ast_to_buffer(&parsed_source))
}
