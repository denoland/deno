// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::SourceTextInfo;
use deno_graph::ModuleEntryRef;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionResolved;
use deno_graph::WalkOptions;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
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
     source_text: &Arc<str>,
     specifier_text: &str,
     resolution: &ResolutionResolved| {
      if visited.insert(resolution.specifier.clone()) {
        match resolution.specifier.scheme() {
          "file" | "data" | "node" => {}
          "jsr" => {
            skip_specifiers.insert(resolution.specifier.clone());

            // check for a missing version constraint
            if let Ok(jsr_req_ref) =
              JsrPackageReqReference::from_specifier(&resolution.specifier)
            {
              if jsr_req_ref.req().version_req.version_text() == "*" {
                let maybe_version = graph
                  .packages
                  .mappings()
                  .find(|(req, _)| *req == jsr_req_ref.req())
                  .map(|(_, nv)| nv.version.clone());
                diagnostics_collector.push(
                  PublishDiagnostic::MissingConstraint {
                    specifier: resolution.specifier.clone(),
                    specifier_text: specifier_text.to_string(),
                    resolved_version: maybe_version,
                    text_info: SourceTextInfo::new(source_text.clone()),
                    referrer: resolution.range.clone(),
                  },
                );
              }
            }
          }
          "npm" => {
            skip_specifiers.insert(resolution.specifier.clone());

            // check for a missing version constraint
            if let Ok(jsr_req_ref) =
              NpmPackageReqReference::from_specifier(&resolution.specifier)
            {
              if jsr_req_ref.req().version_req.version_text() == "*" {
                let maybe_version = graph
                  .get(&resolution.specifier)
                  .and_then(|m| m.npm())
                  .map(|n| n.nv_reference.nv().version.clone());
                diagnostics_collector.push(
                  PublishDiagnostic::MissingConstraint {
                    specifier: resolution.specifier.clone(),
                    specifier_text: specifier_text.to_string(),
                    resolved_version: maybe_version,
                    text_info: SourceTextInfo::new(source_text.clone()),
                    referrer: resolution.range.clone(),
                  },
                );
              }
            }
          }
          "http" | "https" => {
            skip_specifiers.insert(resolution.specifier.clone());
            diagnostics_collector.push(
              PublishDiagnostic::InvalidExternalImport {
                kind: format!("non-JSR '{}'", resolution.specifier.scheme()),
                text_info: SourceTextInfo::new(source_text.clone()),
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
                text_info: SourceTextInfo::new(source_text.clone()),
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
    // this being disabled will cause it to follow everything in the graph
    prefer_fast_check_graph: false,
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

    for (specifier_text, dep) in &module.dependencies {
      if let Some(resolved) = dep.maybe_code.ok() {
        collect_if_invalid(
          &mut skip_specifiers,
          &module.source,
          specifier_text,
          resolved,
        );
      }
      if let Some(resolved) = dep.maybe_type.ok() {
        collect_if_invalid(
          &mut skip_specifiers,
          &module.source,
          specifier_text,
          resolved,
        );
      }
    }
  }
}
