// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_config::WorkspaceMemberConfig;
use deno_core::error::AnyError;
use deno_graph::FastCheckDiagnostic;
use deno_graph::ModuleGraph;

/// Collects diagnostics from the module graph for the given packages.
/// Returns true if any diagnostics were collected.
pub fn collect_no_slow_type_diagnostics(
  member: &WorkspaceMemberConfig,
  graph: &ModuleGraph,
) -> Result<Vec<FastCheckDiagnostic>, AnyError> {
  let export_urls = member.config_file.resolve_export_value_urls()?;
  let mut js_exports = export_urls
    .iter()
    .filter_map(|url| graph.get(url).and_then(|m| m.js()));
  // fast check puts the same diagnostics in each entrypoint for the
  // package, so we only need to check the first one
  let Some(module) = js_exports.next() else {
    // could happen if all the exports are JSON
    return Ok(vec![]);
  };

  if let Some(diagnostics) = module.fast_check_diagnostics() {
    // todo(https://github.com/denoland/deno_graph/issues/384): move to deno_graph
    let mut diagnostics = diagnostics.iter().cloned().collect::<Vec<_>>();
    diagnostics.sort_by_cached_key(|d| {
      (d.specifier().clone(), d.range().map(|r| r.range.clone()))
    });
    Ok(diagnostics)
  } else {
    Ok(Vec::new())
  }
}
