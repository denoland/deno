// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::ParsedSource;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_graph::ModuleEntryRef;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionResolved;
use deno_graph::WalkOptions;
use lsp_types::Url;

use crate::cache::LazyGraphSourceParser;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;

pub fn collect_invalid_external_imports(
  graph: &ModuleGraph,
  diagnostics_collector: &PublishDiagnosticsCollector,
) {
  let mut visited = HashSet::new();
  let mut skip_specifiers: HashSet<Url> = HashSet::new();

  let mut collect_if_invalid =
    |skip_specifiers: &mut HashSet<Url>,
     text: &Arc<str>,
     resolution: &ResolutionResolved| {
      if visited.insert(resolution.specifier.clone()) {
        match resolution.specifier.scheme() {
          "file" | "data" | "node" => {}
          "jsr" | "npm" => {
            skip_specifiers.insert(resolution.specifier.clone());
          }
          "http" | "https" => {
            skip_specifiers.insert(resolution.specifier.clone());
            diagnostics_collector.push(
              PublishDiagnostic::InvalidExternalImport {
                kind: format!("non-JSR '{}'", resolution.specifier.scheme()),
                text_info: SourceTextInfo::new(text.clone()),
                imported: resolution.specifier.clone(),
                referrer: resolution.range.clone(),
              },
            );
          }
          _ => {
            skip_specifiers.insert(resolution.specifier.clone());
            diagnostics_collector.push(
              PublishDiagnostic::InvalidExternalImport {
                kind: format!("'{}'", resolution.specifier.scheme()),
                text_info: SourceTextInfo::new(text.clone()),
                imported: resolution.specifier.clone(),
                referrer: resolution.range.clone(),
              },
            );
          }
        }
      }
    };

  let options = WalkOptions {
    check_js: true,
    follow_dynamic: true,
    follow_type_only: true,
  };
  let mut iter = graph.walk(&graph.roots, options);
  while let Some((specifier, entry)) = iter.next() {
    if skip_specifiers.contains(specifier) {
      iter.skip_previous_dependencies();
      continue;
    }

    let ModuleEntryRef::Module(module) = entry else {
      continue;
    };
    let Some(module) = module.js() else {
      continue;
    };

    for (_, dep) in &module.dependencies {
      if let Some(resolved) = dep.maybe_code.ok() {
        collect_if_invalid(&mut skip_specifiers, &module.source, resolved);
      }
      if let Some(resolved) = dep.maybe_type.ok() {
        collect_if_invalid(&mut skip_specifiers, &module.source, resolved);
      }
    }
  }
}

fn check_for_banned_syntax_single_module(
  parsed_source: &ParsedSource,
  diagnostics_collector: &PublishDiagnosticsCollector,
) {
  use deno_ast::swc::ast;

  for i in parsed_source.module().body.iter() {
    match i {
      ast::ModuleItem::ModuleDecl(n) => match n {
        ast::ModuleDecl::TsNamespaceExport(n) => {
          diagnostics_collector.push(
            PublishDiagnostic::GlobalTypeAugmentation {
              specifier: parsed_source.specifier().to_owned(),
              range: n.range(),
              text_info: parsed_source.text_info().clone(),
            },
          );
        }
        ast::ModuleDecl::TsExportAssignment(n) => {
          diagnostics_collector.push(
            PublishDiagnostic::GlobalTypeAugmentation {
              specifier: parsed_source.specifier().to_owned(),
              range: n.range(),
              text_info: parsed_source.text_info().clone(),
            },
          );
        }
        ast::ModuleDecl::TsImportEquals(n) => match n.module_ref {
          ast::TsModuleRef::TsExternalModuleRef(_) => {
            diagnostics_collector.push(PublishDiagnostic::CommonJs {
              specifier: parsed_source.specifier().to_owned(),
              range: n.range(),
              text_info: parsed_source.text_info().clone(),
            });
          }
          _ => {
            continue;
          }
        },
        _ => continue,
      },
      ast::ModuleItem::Stmt(n) => match n {
        ast::Stmt::Decl(ast::Decl::TsModule(n)) => {
          if n.global {
            diagnostics_collector.push(
              PublishDiagnostic::GlobalTypeAugmentation {
                specifier: parsed_source.specifier().to_owned(),
                range: n.range(),
                text_info: parsed_source.text_info().clone(),
              },
            );
          }
          match &n.id {
            ast::TsModuleName::Str(n) => {
              diagnostics_collector.push(
                PublishDiagnostic::GlobalTypeAugmentation {
                  specifier: parsed_source.specifier().to_owned(),
                  range: n.range(),
                  text_info: parsed_source.text_info().clone(),
                },
              );
            }
            _ => continue,
          }
        }
        _ => continue,
      },
    }
  }
}

pub fn check_for_banned_syntax(
  graph: &ModuleGraph,
  lazy_graph_source_parser: &LazyGraphSourceParser,
  diagnostics_collector: &PublishDiagnosticsCollector,
) -> Result<(), AnyError> {
  for module in graph.modules() {
    if let Some(parsed_source) =
      lazy_graph_source_parser.get_or_parse_source(module.specifier())?
    {
      check_for_banned_syntax_single_module(
        &parsed_source,
        diagnostics_collector,
      );
    }
  }
  Ok(())
}
