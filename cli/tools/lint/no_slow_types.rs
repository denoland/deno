// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
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
  root_dir: &ModuleSpecifier,
  graph: &ModuleGraph,
) -> NoSlowTypesOutput {
  let mut seen_modules = HashSet::with_capacity(graph.specifiers_count());
  let mut found_diagnostics = Vec::new();
  let mut pending = VecDeque::new();
  for export in &graph.roots {
    if seen_modules.insert(export.clone()) {
      pending.push_back(export.clone());
    }
  }

  while let Some(specifier) = pending.pop_front() {
    let Ok(Some(module)) = graph.try_get_prefer_types(&specifier) else {
      continue;
    };
    let Some(es_module) = module.js() else {
      continue;
    };
    if let Some(diagnostics) = es_module.fast_check_diagnostics() {
      if diagnostics.iter().any(|diagnostic| {
        matches!(
          diagnostic,
          FastCheckDiagnostic::UnsupportedJavaScriptEntrypoint { .. }
        )
      }) {
        return NoSlowTypesOutput::HasJsExport;
      }
      found_diagnostics.extend(diagnostics.iter().cloned());
    }

    // analyze the next dependencies
    for dep in es_module.dependencies_prefer_fast_check().values() {
      let Some(specifier) = graph.resolve_dependency_from_dep(dep, true) else {
        continue;
      };

      let dep_in_same_package =
        specifier.as_str().starts_with(root_dir.as_str());
      if dep_in_same_package {
        let is_new = seen_modules.insert(specifier.clone());
        if is_new {
          pending.push_back(specifier.clone());
        }
      }
    }
  }

  if found_diagnostics.is_empty() {
    NoSlowTypesOutput::Pass
  } else {
    NoSlowTypesOutput::Fail(found_diagnostics)
  }
}
