// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::media_type::MediaType;
use crate::tools::lint::create_linter;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_lint::rules;
use lsp_types::Position;
use lsp_types::Range;

pub enum Category {
  Lint(String, String, Option<String>),
}

/// A structure to hold a reference to a diagnostic message.
pub struct Reference {
  category: Category,
  range: Range,
}

fn as_lsp_range(range: &deno_lint::diagnostic::Range) -> Range {
  Range {
    start: Position {
      line: (range.start.line - 1) as u32,
      character: range.start.col as u32,
    },
    end: Position {
      line: (range.end.line - 1) as u32,
      character: range.end.col as u32,
    },
  }
}

pub fn get_lint_references(
  specifier: &ModuleSpecifier,
  media_type: &MediaType,
  source_code: &str,
) -> Result<Vec<Reference>, AnyError> {
  let syntax = ast::get_syntax(media_type);
  let lint_rules = rules::get_recommended_rules();
  let mut linter = create_linter(syntax, lint_rules);
  // TODO(@kitsonk) we should consider caching the swc source file versions for
  // reuse by other processes
  let (_, lint_diagnostics) =
    linter.lint(specifier.to_string(), source_code.to_string())?;

  Ok(
    lint_diagnostics
      .into_iter()
      .map(|d| Reference {
        category: Category::Lint(d.message, d.code, d.hint),
        range: as_lsp_range(&d.range),
      })
      .collect(),
  )
}

pub fn references_to_diagnostics(
  references: Vec<Reference>,
) -> Vec<lsp_types::Diagnostic> {
  references
    .into_iter()
    .map(|r| match r.category {
      Category::Lint(message, code, _) => lsp_types::Diagnostic {
        range: r.range,
        severity: Some(lsp_types::DiagnosticSeverity::Warning),
        code: Some(lsp_types::NumberOrString::String(code)),
        code_description: None,
        // TODO(@kitsonk) this won't make sense for every diagnostic
        source: Some("deno-lint".to_string()),
        message,
        related_information: None,
        tags: None, // we should tag unused code
        data: None,
      },
    })
    .collect()
}
