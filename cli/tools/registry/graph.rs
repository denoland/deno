// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::SourceTextInfo;
use deno_graph::ModuleEntryRef;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionResolved;
use deno_graph::WalkOptions;
use lsp_types::Url;

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
