// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::import_map::ImportMap;
use crate::media_type::MediaType;
use crate::module_graph::parse_deno_types;
use crate::module_graph::parse_ts_reference;
use crate::module_graph::TypeScriptReference;
use crate::tools::lint::create_linter;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_lint::rules;
use lsp_types::Position;
use lsp_types::Range;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Category of self-generated diagnostic messages (those not coming from)
/// TypeScript.
pub enum Category {
  /// A lint diagnostic, where the first element is the message, the second
  /// element is the lint code (rule) and the third is an optional hint.
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dependency {
  pub is_dynamic: bool,
  pub maybe_code: Option<ResolvedImport>,
  pub maybe_type: Option<ResolvedImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedImport {
  Resolved(ModuleSpecifier),
  Err(String),
}

pub fn resolve_import(
  specifier: &str,
  referrer: &ModuleSpecifier,
  maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
) -> ResolvedImport {
  let maybe_mapped = if let Some(import_map) = maybe_import_map {
    if let Ok(maybe_specifier) =
      import_map.borrow().resolve(specifier, referrer.as_str())
    {
      maybe_specifier
    } else {
      None
    }
  } else {
    None
  };
  let remapped = maybe_mapped.is_some();
  let specifier = if let Some(remapped) = maybe_mapped {
    remapped
  } else {
    match ModuleSpecifier::resolve_import(specifier, referrer.as_str()) {
      Ok(resolved) => resolved,
      Err(err) => return ResolvedImport::Err(err.to_string()),
    }
  };
  let referrer_scheme = referrer.as_url().scheme();
  let specifier_scheme = specifier.as_url().scheme();
  if referrer_scheme == "https" && specifier_scheme == "http" {
    return ResolvedImport::Err(
      "Modules imported via https are not allowed to import http modules."
        .to_string(),
    );
  }
  if (referrer_scheme == "https" || referrer_scheme == "http")
    && !(specifier_scheme == "https" || specifier_scheme == "http")
    && !remapped
  {
    return ResolvedImport::Err("Remote modules are not allowed to import local modules.  Consider using a dynamic import instead.".to_string());
  }

  ResolvedImport::Resolved(specifier)
}

// TODO(@kitsonk) a lot of this logic is duplicated in module_graph.rs in
// Module::parse() and should be refactored out to a common function.
pub fn analyze_dependencies(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
  maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
) -> Option<(HashMap<String, Dependency>, Option<ResolvedImport>)> {
  let specifier_str = specifier.to_string();
  let source_map = Rc::new(swc_common::SourceMap::default());
  let mut maybe_type = None;
  if let Ok(parsed_module) =
    ast::parse_with_source_map(&specifier_str, source, &media_type, source_map)
  {
    let mut dependencies = HashMap::<String, Dependency>::new();

    // Parse leading comments for supported triple slash references.
    for comment in parsed_module.get_leading_comments().iter() {
      if let Some(ts_reference) = parse_ts_reference(&comment.text) {
        match ts_reference {
          TypeScriptReference::Path(import) => {
            let dep = dependencies.entry(import.clone()).or_default();
            let resolved_import =
              resolve_import(&import, specifier, maybe_import_map.clone());
            dep.maybe_code = Some(resolved_import);
          }
          TypeScriptReference::Types(import) => {
            let resolved_import =
              resolve_import(&import, specifier, maybe_import_map.clone());
            if media_type == &MediaType::JavaScript
              || media_type == &MediaType::JSX
            {
              maybe_type = Some(resolved_import)
            } else {
              let dep = dependencies.entry(import).or_default();
              dep.maybe_type = Some(resolved_import);
            }
          }
        }
      }
    }

    // Parse ES and type only imports
    let descriptors = parsed_module.analyze_dependencies();
    for desc in descriptors.into_iter().filter(|desc| {
      desc.kind != swc_ecmascript::dep_graph::DependencyKind::Require
    }) {
      let resolved_import =
        resolve_import(&desc.specifier, specifier, maybe_import_map.clone());

      // Check for `@deno-types` pragmas that effect the import
      let maybe_resolved_type_import =
        if let Some(comment) = desc.leading_comments.last() {
          if let Some(deno_types) = parse_deno_types(&comment.text).as_ref() {
            Some(resolve_import(
              deno_types,
              specifier,
              maybe_import_map.clone(),
            ))
          } else {
            None
          }
        } else {
          None
        };

      let dep = dependencies.entry(desc.specifier.to_string()).or_default();
      dep.is_dynamic = desc.is_dynamic;
      match desc.kind {
        swc_ecmascript::dep_graph::DependencyKind::ExportType
        | swc_ecmascript::dep_graph::DependencyKind::ImportType => {
          dep.maybe_type = Some(resolved_import)
        }
        _ => dep.maybe_code = Some(resolved_import),
      }
      if maybe_resolved_type_import.is_some() && dep.maybe_type.is_none() {
        dep.maybe_type = maybe_resolved_type_import;
      }
    }

    Some((dependencies, maybe_type))
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_as_lsp_range() {
    let fixture = deno_lint::diagnostic::Range {
      start: deno_lint::diagnostic::Position {
        line: 1,
        col: 2,
        byte_pos: 23,
      },
      end: deno_lint::diagnostic::Position {
        line: 2,
        col: 0,
        byte_pos: 33,
      },
    };
    let actual = as_lsp_range(&fixture);
    assert_eq!(
      actual,
      lsp_types::Range {
        start: lsp_types::Position {
          line: 0,
          character: 2,
        },
        end: lsp_types::Position {
          line: 1,
          character: 0,
        },
      }
    );
  }

  #[test]
  fn test_analyze_dependencies() {
    let specifier =
      ModuleSpecifier::resolve_url("file:///a.ts").expect("bad specifier");
    let source = r#"import {
      Application,
      Context,
      Router,
      Status,
    } from "https://deno.land/x/oak@v6.3.2/mod.ts";
    
    // @deno-types="https://deno.land/x/types/react/index.d.ts";
    import * as React from "https://cdn.skypack.dev/react";
    "#;
    let actual =
      analyze_dependencies(&specifier, source, &MediaType::TypeScript, None);
    assert!(actual.is_some());
    let (actual, maybe_type) = actual.unwrap();
    assert!(maybe_type.is_none());
    assert_eq!(actual.len(), 2);
    assert_eq!(
      actual.get("https://cdn.skypack.dev/react").cloned(),
      Some(Dependency {
        is_dynamic: false,
        maybe_code: Some(ResolvedImport::Resolved(
          ModuleSpecifier::resolve_url("https://cdn.skypack.dev/react")
            .unwrap()
        )),
        maybe_type: Some(ResolvedImport::Resolved(
          ModuleSpecifier::resolve_url(
            "https://deno.land/x/types/react/index.d.ts"
          )
          .unwrap()
        )),
      })
    );
    assert_eq!(
      actual.get("https://deno.land/x/oak@v6.3.2/mod.ts").cloned(),
      Some(Dependency {
        is_dynamic: false,
        maybe_code: Some(ResolvedImport::Resolved(
          ModuleSpecifier::resolve_url("https://deno.land/x/oak@v6.3.2/mod.ts")
            .unwrap()
        )),
        maybe_type: None,
      })
    );
  }
}
