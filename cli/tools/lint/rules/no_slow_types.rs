// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;

use deno_ast::diagnostics::Diagnostic;
use deno_ast::ModuleSpecifier;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ModuleGraph;
use deno_lint::diagnostic::LintDiagnostic;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;

use super::PackageLintRule;

const CODE: &str = "no-slow-types";

#[derive(Debug)]
pub struct NoSlowTypesRule;

impl PackageLintRule for NoSlowTypesRule {
  fn code(&self) -> &'static str {
    CODE
  }

  fn tags(&self) -> &'static [&'static str] {
    &["jsr"]
  }

  fn docs(&self) -> &'static str {
    include_str!("no_slow_types.md")
  }

  fn help_docs_url(&self) -> Cow<'static, str> {
    Cow::Borrowed("https://jsr.io/docs/about-slow-types")
  }

  fn lint_package(
    &self,
    graph: &ModuleGraph,
    entrypoints: &[ModuleSpecifier],
  ) -> Vec<LintDiagnostic> {
    collect_no_slow_type_diagnostics(graph, entrypoints)
      .into_iter()
      .map(|d| LintDiagnostic {
        specifier: d.specifier().clone(),
        range: d.range().map(|range| LintDiagnosticRange {
          text_info: range.text_info.clone(),
          range: range.range,
          description: d.range_description().map(|r| r.to_string()),
        }),
        details: LintDiagnosticDetails {
          message: d.message().to_string(),
          code: CODE.to_string(),
          hint: d.hint().map(|h| h.to_string()),
          info: d
            .info()
            .iter()
            .map(|info| Cow::Owned(info.to_string()))
            .collect(),
          fixes: vec![],
          custom_docs_url: d.docs_url().map(|u| u.into_owned()),
        },
      })
      .collect()
  }
}

/// Collects diagnostics from the module graph for the
/// given package's export URLs.
pub fn collect_no_slow_type_diagnostics(
  graph: &ModuleGraph,
  package_export_urls: &[ModuleSpecifier],
) -> Vec<FastCheckDiagnostic> {
  let mut js_exports = package_export_urls
    .iter()
    .filter_map(|url| graph.get(url).and_then(|m| m.js()));
  // fast check puts the same diagnostics in each entrypoint for the
  // package (since it's all or nothing), so we only need to check
  // the first one JS entrypoint
  let Some(module) = js_exports.next() else {
    // could happen if all the exports are JSON
    return vec![];
  };

  if let Some(diagnostics) = module.fast_check_diagnostics() {
    let mut diagnostics = diagnostics.clone();
    diagnostics.sort_by_cached_key(|d| {
      (
        d.specifier().clone(),
        d.range().map(|r| r.range),
        d.code().to_string(),
      )
    });
    diagnostics
  } else {
    Vec::new()
  }
}
