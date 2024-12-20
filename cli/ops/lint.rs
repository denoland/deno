// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;

use crate::tools::lint;

deno_core::extension!(deno_lint, ops = [op_lint_create_serialized_ast,],);

#[op2]
#[buffer]
fn op_lint_create_serialized_ast(
  #[string] file_name: &str,
  #[string] source: String,
) -> Result<Vec<u8>, AnyError> {
  let file_text = deno_ast::strip_bom(source);
  let path = std::env::current_dir()?.join(file_name);
  let specifier = ModuleSpecifier::from_file_path(&path).map_err(|_| {
    generic_error(format!("Failed to parse path as URL: {}", path.display()))
  })?;
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
