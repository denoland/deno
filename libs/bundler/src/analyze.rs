// Copyright 2018-2026 the Deno authors. MIT license.

//! Module analysis pass: populates module_info and hmr_info on the graph.
//!
//! This runs after transpilation so all modules are JS. It parses each
//! module with deno_ast and extracts import/export bindings, top-level
//! declarations, and HMR metadata.

use deno_ast::MediaType;
use deno_ast::ParseParams;

use crate::graph::BundlerGraph;
use crate::js::hmr_info_swc::extract_hmr_info;
use crate::js::module_info_swc::extract_module_info;
use crate::loader::Loader;

/// Analyze all JS modules in the graph, populating `module_info` and `hmr_info`.
///
/// Should be called after `transpile_graph()` so all modules are JS.
pub fn analyze_graph(graph: &mut BundlerGraph) {
  let specifiers: Vec<_> = graph
    .modules()
    .filter(|m| is_analyzable(m.loader))
    .map(|m| m.specifier.clone())
    .collect();

  for specifier in specifiers {
    let module = graph.get_module(&specifier).unwrap();
    let source = module.source.clone();

    let parsed = match deno_ast::parse_module(ParseParams {
      specifier: specifier.clone(),
      text: source.into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    }) {
      Ok(p) => p,
      Err(e) => {
        eprintln!("Failed to parse {} for analysis: {}", specifier, e);
        continue;
      }
    };

    let module_info = extract_module_info(&parsed);
    let hmr_info = extract_hmr_info(&parsed);

    // Check for top-level await.
    let is_async = module_info.has_tla;

    if let Some(module) = graph.get_module_mut(&specifier) {
      module.module_info = Some(module_info);
      module.hmr_info = Some(hmr_info);
      module.is_async = is_async;
    }
  }
}

fn is_analyzable(loader: Loader) -> bool {
  matches!(loader, Loader::Js | Loader::Jsx | Loader::Ts | Loader::Tsx)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::EnvironmentId;
  use crate::module::BundlerModule;
  use crate::module::ModuleType;
  use crate::module::SideEffectFlag;
  use deno_ast::ModuleSpecifier;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type: ModuleType::Esm,
      dependencies: Vec::new(),
      side_effects: SideEffectFlag::Unknown,
      source: source.to_string(),
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    }
  }

  #[test]
  fn test_analyze_extracts_module_info() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "import { foo } from './dep.js';\nexport const bar = 1;",
    ));
    graph.add_entry(s.clone());

    analyze_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    let info = module.module_info.as_ref().unwrap();
    assert!(!info.imports.is_empty());
    assert!(!info.exports.is_empty());
  }

  #[test]
  fn test_analyze_extracts_hmr_info() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "if (import.meta.hot) { import.meta.hot.accept(); }",
    ));
    graph.add_entry(s.clone());

    analyze_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    let hmr = module.hmr_info.as_ref().unwrap();
    assert!(hmr.self_accepts);
  }

  #[test]
  fn test_analyze_detects_tla() {
    let s = spec("mod.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "const data = await fetch('/api');",
    ));
    graph.add_entry(s.clone());

    analyze_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    assert!(module.is_async);
  }

  #[test]
  fn test_analyze_skips_non_js() {
    let s = spec("data.json");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    let mut module = make_module(&s, r#"{"key": "value"}"#);
    module.loader = Loader::Json;
    graph.add_module(module);
    graph.add_entry(s.clone());

    analyze_graph(&mut graph);

    let module = graph.get_module(&s).unwrap();
    assert!(module.module_info.is_none());
  }
}
