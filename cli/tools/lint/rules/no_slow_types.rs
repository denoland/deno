// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::diagnostics::Diagnostic;
use deno_ast::ModuleSpecifier;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ModuleGraph;
use deno_lint::diagnostic::LintDiagnostic;

use super::CliGraphPackageLintRule;

pub const CODE: &str = "no-slow-types";

#[derive(Debug)]
pub struct NoSlowTypesRule;

impl CliGraphPackageLintRule for NoSlowTypesRule {
  fn code(&self) -> &'static str {
    CODE
  }

  fn tags(&self) -> &'static [&'static str] {
    &["jsr"]
  }

  fn docs(&self) -> &'static str {
    include_str!("no_slow_types.md")
  }

  fn lint(
    &self,
    graph: &ModuleGraph,
    entrypoints: &[ModuleSpecifier],
  ) -> Vec<FastCheckDiagnostic> {
    collect_no_slow_type_diagnostics(graph, entrypoints)
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
