// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_config::WorkspaceMemberConfig;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ModuleGraph;

#[derive(Debug, Clone)]
pub enum NoSlowTypesOutput {
  Pass,
  HasJsExport,
  Fail(Vec<FastCheckDiagnostic>),
}

/// Collects diagnostics from the module graph for the given packages.
/// Returns true if any diagnostics were collected.
pub fn collect_no_slow_type_diagnostics(
  member: &WorkspaceMemberConfig,
  graph: &ModuleGraph,
) -> Result<NoSlowTypesOutput, AnyError> {
  let export_urls = member.config_file.resolve_export_value_urls()?;
  let mut js_exports = export_urls
    .iter()
    .filter_map(|url| graph.get(url).and_then(|m| m.js()));
  // fast check puts the same diagnostics in each entrypoint for the
  // package, so we only need to check the first one
  let Some(module) = js_exports.next() else {
    // could happen if all the exports are JSON
    return Ok(NoSlowTypesOutput::Pass);
  };

  if let Some(diagnostics) = module.fast_check_diagnostics() {
    if diagnostics.iter().any(|diagnostic| {
      matches!(
        diagnostic,
        FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint { .. }
      )
    }) {
      Ok(NoSlowTypesOutput::HasJsExport)
    } else {
      Ok(NoSlowTypesOutput::Fail(
        diagnostics.iter().cloned().collect(),
      ))
    }
  } else {
    Ok(NoSlowTypesOutput::Pass)
  }
}
