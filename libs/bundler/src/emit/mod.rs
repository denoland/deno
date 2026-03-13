// Copyright 2018-2026 the Deno authors. MIT license.

//! Code emission for bundled chunks.
//!
//! Provides both dev-mode emission (module registration wrappers) and
//! production emission (concatenated modules with CJS interop helpers).

mod production;

use deno_ast::ModuleSpecifier;

use crate::chunk::Chunk;
use crate::chunk::ChunkGraph;
use crate::chunk::ChunkType;
use crate::graph::BundlerGraph;
use crate::module::ModuleType;

pub use production::emit_production_chunk;

/// Output from emitting a chunk.
#[derive(Debug, Clone)]
pub struct ChunkOutput {
  /// The generated JavaScript code.
  pub code: String,
  /// Optional source map JSON.
  pub source_map: Option<String>,
}

/// Emit a chunk in dev mode (no minification, no scope hoisting).
///
/// Dev format wraps each module in a `__d.update()` registration call.
/// Modules are evaluated lazily via `__d.require()`.
///
/// Format:
/// ```js
/// var __d = globalThis.__dbundle;
/// // Module: ./dep.ts
/// __d.update(0, function(exports, __require, __hot) {
///   // transformed module code
/// });
/// // Module: ./entry.ts
/// __d.update(1, function(exports, __require, __hot) {
///   // transformed module code
/// });
/// var __entry = await __d.require(1);
/// export default __entry.default;
/// ```
pub fn emit_dev_chunk(
  chunk: &Chunk,
  graph: &BundlerGraph,
  _chunk_graph: &ChunkGraph,
) -> ChunkOutput {
  let mut code = String::new();

  // Preamble: grab the runtime.
  code.push_str("var __d = globalThis.__dbundle;\n");

  // Emit each module wrapped in __d.update().
  for specifier in &chunk.modules {
    if let Some(module) = graph.get_module(specifier) {
      let mid = graph.module_index(specifier).unwrap_or(0);
      let relative = make_relative_path(specifier);

      code.push_str(&format!("// Module: {}\n", relative));

      let is_async = module.is_async;
      let is_cjs = module.module_type == ModuleType::Cjs;

      // Header.
      if is_async {
        code.push_str(&format!(
          "__d.update({}, async function(exports, __require, __hot) {{\n",
          mid
        ));
      } else {
        code.push_str(&format!(
          "__d.update({}, function(exports, __require, __hot) {{\n",
          mid
        ));
      }

      // CJS preamble.
      if is_cjs {
        code.push_str("  var module = { exports: exports };\n");
      }

      // Module body (indented).
      for line in module.source.lines() {
        if line.is_empty() {
          code.push('\n');
        } else {
          code.push_str("  ");
          code.push_str(line);
          code.push('\n');
        }
      }

      // CJS epilogue: sync module.exports back.
      if is_cjs {
        code.push_str(&format!(
          "  if (exports !== module.exports) __d.setCjsExports({}, module.exports);\n",
          mid
        ));
      }

      // Footer.
      if is_async {
        code.push_str("}, true);\n");
      } else {
        code.push_str("});\n");
      }
    }
  }

  // Entry evaluation.
  if let Some(entry) = &chunk.entry {
    if let Some(mid) = graph.module_index(entry) {
      match chunk.chunk_type {
        ChunkType::Entry | ChunkType::DynamicImport => {
          code.push_str(&format!(
            "var __entry = await __d.require({});\n",
            mid
          ));
          code.push_str("export default __entry.default;\n");
        }
        _ => {}
      }
    }
  }

  ChunkOutput {
    code,
    source_map: None,
  }
}

/// Emit a single module for HMR update (standalone, not in a chunk).
///
/// Returns just the `__d.update(...)` wrapper for the changed module.
pub fn emit_hmr_update(
  specifier: &ModuleSpecifier,
  graph: &BundlerGraph,
) -> Option<String> {
  let module = graph.get_module(specifier)?;
  let mid = graph.module_index(specifier)?;

  let mut code = String::new();
  let is_async = module.is_async;
  let is_cjs = module.module_type == ModuleType::Cjs;

  if is_async {
    code.push_str(&format!(
      "__d.update({}, async function(exports, __require, __hot) {{\n",
      mid
    ));
  } else {
    code.push_str(&format!(
      "__d.update({}, function(exports, __require, __hot) {{\n",
      mid
    ));
  }

  if is_cjs {
    code.push_str("  var module = { exports: exports };\n");
  }

  for line in module.source.lines() {
    if line.is_empty() {
      code.push('\n');
    } else {
      code.push_str("  ");
      code.push_str(line);
      code.push('\n');
    }
  }

  if is_cjs {
    code.push_str(&format!(
      "  if (exports !== module.exports) __d.setCjsExports({}, module.exports);\n",
      mid
    ));
  }

  if is_async {
    code.push_str("}, true);\n");
  } else {
    code.push_str("});\n");
  }

  Some(code)
}

/// Convert a module specifier to a relative-looking path for comments.
fn make_relative_path(specifier: &ModuleSpecifier) -> String {
  if specifier.scheme() == "file" {
    specifier
      .path()
      .rsplit('/')
      .take(2)
      .collect::<Vec<_>>()
      .into_iter()
      .rev()
      .collect::<Vec<_>>()
      .join("/")
  } else {
    specifier.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::chunk::build_chunk_graph;
  use crate::config::EnvironmentId;
  use crate::dependency::Dependency;
  use crate::dependency::ImportKind;
  use crate::loader::Loader;
  use crate::module::BundlerModule;
  use crate::module::ModuleType;
  use crate::module::SideEffectFlag;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
    deps: Vec<Dependency>,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type: ModuleType::Esm,
      dependencies: deps,
      side_effects: SideEffectFlag::Unknown,
      source: source.to_string(),
      parsed: None,
      module_info: None,
      hmr_info: None,
      is_async: false,
      external_imports: Vec::new(),
    }
  }

  fn make_dep(target: &ModuleSpecifier, kind: ImportKind) -> Dependency {
    Dependency {
      specifier: target.to_string(),
      resolved: target.clone(),
      kind,
      range: None,
    }
  }

  #[test]
  fn test_emit_dev_single_module() {
    let entry = spec("entry.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "console.log(\"hello\");",
      vec![],
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph);

    assert!(output.code.contains("var __d = globalThis.__dbundle;"));
    assert!(output.code.contains("__d.update(0,"));
    assert!(output.code.contains("console.log(\"hello\");"));
    assert!(output.code.contains("await __d.require(0)"));
    assert!(output.code.contains("export default __entry.default;"));
  }

  #[test]
  fn test_emit_dev_with_dependency() {
    let entry = spec("entry.ts");
    let dep = spec("dep.ts");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "import { foo } from './dep';\nfoo();",
      vec![make_dep(&dep, ImportKind::Import)],
    ));
    graph.add_module(make_module(
      &dep,
      "export function foo() { return 42; }",
      vec![],
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph);

    // Both modules should be in the output.
    assert!(output.code.contains("__d.update(0,"));
    assert!(output.code.contains("__d.update(1,"));
    // dep should appear before entry in topological order.
    let dep_pos = output.code.find("Module: /dep.ts").unwrap();
    let entry_pos = output.code.find("Module: /entry.ts").unwrap();
    assert!(dep_pos < entry_pos);
  }

  #[test]
  fn test_emit_dev_cjs_module() {
    let entry = spec("entry.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    let mut module = make_module(
      &entry,
      "module.exports = 42;",
      vec![],
    );
    module.module_type = ModuleType::Cjs;
    graph.add_module(module);
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph);

    assert!(output.code.contains("var module = { exports: exports };"));
    assert!(output
      .code
      .contains("__d.setCjsExports(0, module.exports)"));
  }

  #[test]
  fn test_emit_dev_async_module() {
    let entry = spec("entry.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    let mut module = make_module(
      &entry,
      "const data = await fetch('/api');",
      vec![],
    );
    module.is_async = true;
    graph.add_module(module);
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph);

    assert!(output.code.contains("async function(exports"));
    assert!(output.code.contains("}, true);"));
  }

  #[test]
  fn test_emit_hmr_update() {
    let entry = spec("entry.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "console.log('updated');",
      vec![],
    ));
    graph.add_entry(entry.clone());

    let code = emit_hmr_update(&entry, &graph).unwrap();
    assert!(code.contains("__d.update(0,"));
    assert!(code.contains("console.log('updated');"));
    assert!(!code.contains("__d.require"));
  }
}
