// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::swc::common::comments::CommentKind;
use deno_ast::ParsedSource;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleEntryRef;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionResolved;
use deno_graph::WalkOptions;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::cache::ParsedSourceCache;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;

pub struct GraphDiagnosticsCollector {
  parsed_source_cache: Arc<ParsedSourceCache>,
}

impl GraphDiagnosticsCollector {
  pub fn new(parsed_source_cache: Arc<ParsedSourceCache>) -> Self {
    Self {
      parsed_source_cache,
    }
  }

  pub fn collect_diagnostics_for_graph(
    &self,
    graph: &ModuleGraph,
    diagnostics_collector: &PublishDiagnosticsCollector,
  ) -> Result<(), AnyError> {
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
                    .get(jsr_req_ref.req())
                    .map(|nv| nv.version.clone());
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
      // search the entire graph and not just the fast check subset
      prefer_fast_check_graph: false,
      kind: deno_graph::GraphKind::All,
    };
    let mut iter = graph.walk(graph.roots.iter(), options);
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

      let parsed_source = self
        .parsed_source_cache
        .get_parsed_source_from_js_module(module)?;

      // surface syntax errors
      for diagnostic in parsed_source.diagnostics() {
        diagnostics_collector
          .push(PublishDiagnostic::SyntaxError(diagnostic.clone()));
      }

      check_for_banned_triple_slash_directives(
        &parsed_source,
        diagnostics_collector,
      );

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

    Ok(())
  }
}

fn check_for_banned_triple_slash_directives(
  parsed_source: &ParsedSource,
  diagnostics_collector: &PublishDiagnosticsCollector,
) {
  let triple_slash_re = lazy_regex::regex!(
    r#"^/\s+<reference\s+(no-default-lib\s*=\s*"true"|lib\s*=\s*("[^"]+"|'[^']+'))\s*/>\s*$"#
  );

  let Some(comments) = parsed_source.get_leading_comments() else {
    return;
  };
  for comment in comments {
    if comment.kind != CommentKind::Line {
      continue;
    }
    if triple_slash_re.is_match(&comment.text) {
      diagnostics_collector.push(
        PublishDiagnostic::BannedTripleSlashDirectives {
          specifier: parsed_source.specifier().clone(),
          range: comment.range(),
          text_info: parsed_source.text_info_lazy().clone(),
        },
      );
    }
  }
}
