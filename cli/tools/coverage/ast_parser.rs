// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::get_syntax;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseDiagnostic;
use deno_ast::ParsedSource;

pub(crate) fn parse_program(
  specifier: ModuleSpecifier,
  media_type: MediaType,
  source_code: &str,
) -> Result<ParsedSource, ParseDiagnostic> {
  let syntax = get_syntax(media_type);
  deno_ast::parse_program(deno_ast::ParseParams {
    specifier,
    media_type,
    text: source_code.into(),
    capture_tokens: true,
    maybe_syntax: Some(syntax),
    scope_analysis: true,
  })
}
