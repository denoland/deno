// Copyright 2018-2026 the Deno authors. MIT license.

//! Graph processing: runs the plugin transform chain on all modules.
//!
//! This replaces the separate `transpile_graph()` call — transpilation is now
//! folded into the plugin transform chain as a built-in hook.

use std::path::Path;

use crate::graph::BundlerGraph;
use crate::loader::Loader;
use crate::plugin::PluginDriver;

/// Run the plugin transform chain on all modules in the graph.
///
/// For each module, runs the `PluginDriver::transform()` pipeline which
/// includes both plugin transforms and the built-in TypeScript transpiler.
/// Updates each module's `source`, `loader`, and clears stale analysis data.
///
/// Should be called after building the graph and before `analyze_graph()`.
pub fn transform_modules(graph: &mut BundlerGraph, driver: &PluginDriver) {
  let specifiers: Vec<_> = graph
    .modules()
    .filter(|m| is_transformable(m.loader))
    .map(|m| (m.specifier.clone(), m.loader))
    .collect();

  for (specifier, loader) in specifiers {
    let module = graph.get_module(&specifier).unwrap();
    let source = module.source.clone();

    // Determine the path for matching hook filters.
    let path = if specifier.scheme() == "file" {
      specifier
        .to_file_path()
        .unwrap_or_else(|_| Path::new(specifier.path()).to_path_buf())
    } else {
      Path::new(specifier.path()).to_path_buf()
    };

    let output = driver.transform(source, &path, "file", loader);

    if let Some(module) = graph.get_module_mut(&specifier) {
      module.source = output.content;
      module.loader = output.loader;
      // Clear stale analysis/parse data — will be repopulated later.
      module.parsed = None;
      module.transformed_program = None;
      module.module_info = None;
      module.hmr_info = None;
      module.is_async = false;
      // Eagerly parse JS output so downstream passes reuse the cached AST.
      if matches!(module.loader, Loader::Js) {
        module.ensure_parsed();
      }
    }
  }
}

/// Whether a loader type should go through the transform chain.
fn is_transformable(loader: Loader) -> bool {
  matches!(
    loader,
    Loader::Js
      | Loader::Jsx
      | Loader::Ts
      | Loader::Tsx
      | Loader::Css
      | Loader::Html
      | Loader::Text
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::EnvironmentId;
  use crate::module::BundlerModule;
  use crate::module::ModuleType;
  use crate::module::SideEffectFlag;
  use crate::plugin::create_default_plugin_driver;
  use deno_ast::ModuleSpecifier;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
    loader: Loader,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: loader,
      loader,
      module_type: ModuleType::Esm,
      dependencies: Vec::new(),
      side_effects: SideEffectFlag::Unknown,
      source: source.to_string(),
      parsed: None,
      transformed_program: None,
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    }
  }

  #[test]
  fn test_transform_modules_transpiles_ts() {
    let s = spec("mod.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &s,
      "const x: number = 42;\nexport default x;",
      Loader::Ts,
    ));
    graph.add_entry(s.clone());

    let driver = create_default_plugin_driver();
    transform_modules(&mut graph, &driver);

    let module = graph.get_module(&s).unwrap();
    assert!(!module.source.contains(": number"));
    assert!(module.source.contains("42"));
    assert_eq!(module.loader, Loader::Js);
  }

  #[test]
  fn test_transform_modules_skips_js() {
    let s = spec("mod.js");
    let source = "const x = 42;";
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&s, source, Loader::Js));
    graph.add_entry(s.clone());

    let driver = create_default_plugin_driver();
    transform_modules(&mut graph, &driver);

    let module = graph.get_module(&s).unwrap();
    assert_eq!(module.source, source);
    assert_eq!(module.loader, Loader::Js);
  }

  #[test]
  fn test_transform_modules_skips_assets() {
    let s = spec("image.png");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&s, "", Loader::Asset));
    graph.add_entry(s.clone());

    let driver = create_default_plugin_driver();
    transform_modules(&mut graph, &driver);

    let module = graph.get_module(&s).unwrap();
    assert_eq!(module.loader, Loader::Asset); // Unchanged.
  }

  #[test]
  fn test_transform_modules_clears_stale_analysis() {
    let s = spec("mod.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    let mut module = make_module(
      &s,
      "const x: number = 42;",
      Loader::Ts,
    );
    // Pretend this module was previously analyzed.
    module.hmr_info = Some(crate::js::hmr_info::HmrInfo {
      self_accepts: true,
      accepted_deps: vec![],
      declines: false,
      has_dispose: false,
      has_hot_api: true,
    });
    module.is_async = true;
    graph.add_module(module);
    graph.add_entry(s.clone());

    let driver = create_default_plugin_driver();
    transform_modules(&mut graph, &driver);

    let module = graph.get_module(&s).unwrap();
    // Stale analysis should be cleared since content changed.
    assert!(module.hmr_info.is_none());
    assert!(!module.is_async);
  }
}
