// Copyright 2018-2026 the Deno authors. MIT license.

//! Production chunk emission.
//!
//! Concatenates modules into chunks with CJS interop helpers. This is an
//! intermediate step towards full scope hoisting — modules are concatenated
//! as IIFEs rather than scope-hoisted, but the output is self-contained and
//! doesn't require a module runtime.

use deno_ast::ModuleSpecifier;

use crate::chunk::Chunk;
use crate::chunk::ChunkGraph;
use crate::chunk::ChunkType;
use crate::graph::BundlerGraph;
use crate::module::ModuleType;

use super::ChunkOutput;

/// CJS interop runtime helpers, emitted once per chunk that uses CJS modules.
const CJS_HELPERS: &str = r#"var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __copyProps = (to, from, except) => {
  if (from && (typeof from === "object" || typeof from === "function"))
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  return to;
};
var __commonJS = (cb, mod) => function __require() {
  if (mod) return mod.exports;
  var key = __getOwnPropNames(cb)[0];
  (0, cb[key])((mod = { exports: {} }).exports, mod);
  return mod.exports;
};
var __toESM = (mod, isNodeMode, target) => (
  target = mod != null ? Object.create(Object.getPrototypeOf(mod)) : {},
  __copyProps(
    isNodeMode || !mod || !mod.__esModule
      ? __defProp(target, "default", { value: mod, enumerable: true })
      : target,
    mod
  )
);
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
"#;

/// Emit a chunk in production mode.
///
/// Production format concatenates modules, wrapping CJS modules in
/// `__commonJS()` and ESM modules as plain code blocks. Modules within
/// a chunk can reference each other through the concatenated scope.
pub fn emit_production_chunk(
  chunk: &Chunk,
  graph: &BundlerGraph,
  _chunk_graph: &ChunkGraph,
) -> ChunkOutput {
  let mut code = String::new();
  let has_cjs = chunk_has_cjs(chunk, graph);

  // Emit CJS helpers if any module in the chunk is CJS.
  if has_cjs {
    code.push_str(CJS_HELPERS);
    code.push('\n');
  }

  // Emit each module.
  for specifier in &chunk.modules {
    if let Some(module) = graph.get_module(specifier) {
      let relative = make_relative_path(specifier);
      code.push_str(&format!("// {}\n", relative));

      if module.module_type == ModuleType::Cjs {
        emit_cjs_module(&mut code, specifier, &module.source, graph);
      } else {
        emit_esm_module(&mut code, &module.source);
      }
      code.push('\n');
    }
  }

  // Entry chunk exports.
  if let Some(entry) = &chunk.entry {
    match chunk.chunk_type {
      ChunkType::Entry | ChunkType::DynamicImport => {
        // For CJS entries, invoke the require wrapper.
        if let Some(module) = graph.get_module(entry) {
          if module.module_type == ModuleType::Cjs {
            let var_name = make_require_var_name(entry);
            code.push_str(&format!("var __entry = {}();\n", var_name));
            code.push_str("export default __entry;\n");
          }
        }
      }
      _ => {}
    }
  }

  ChunkOutput {
    code,
    source_map: None,
  }
}

/// Emit a CJS module wrapped in `__commonJS()`.
fn emit_cjs_module(
  code: &mut String,
  specifier: &ModuleSpecifier,
  source: &str,
  _graph: &BundlerGraph,
) {
  let var_name = make_require_var_name(specifier);
  let path_key = make_relative_path(specifier);

  code.push_str(&format!(
    "var {} = __commonJS({{\n",
    var_name
  ));
  code.push_str(&format!(
    "  \"{}\"(exports, module) {{\n",
    path_key
  ));

  // Module body (indented 4 spaces).
  for line in source.lines() {
    if line.is_empty() {
      code.push('\n');
    } else {
      code.push_str("    ");
      code.push_str(line);
      code.push('\n');
    }
  }

  code.push_str("  }\n");
  code.push_str("});\n");
}

/// Emit an ESM module as plain code.
fn emit_esm_module(code: &mut String, source: &str) {
  for line in source.lines() {
    code.push_str(line);
    code.push('\n');
  }
}

/// Generate a `require_<name>` variable name from a specifier.
fn make_require_var_name(specifier: &ModuleSpecifier) -> String {
  let name = specifier
    .path_segments()
    .and_then(|s| s.last())
    .unwrap_or("module");
  let name = name
    .strip_suffix(".js")
    .or_else(|| name.strip_suffix(".cjs"))
    .or_else(|| name.strip_suffix(".ts"))
    .or_else(|| name.strip_suffix(".tsx"))
    .or_else(|| name.strip_suffix(".jsx"))
    .unwrap_or(name);
  // Sanitize: replace non-alphanumeric with underscore.
  let sanitized: String = name
    .chars()
    .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
    .collect();
  format!("require_{}", sanitized)
}

/// Check if any module in a chunk is CJS.
fn chunk_has_cjs(chunk: &Chunk, graph: &BundlerGraph) -> bool {
  chunk.modules.iter().any(|s| {
    graph
      .get_module(s)
      .map(|m| m.module_type == ModuleType::Cjs)
      .unwrap_or(false)
  })
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
  use crate::module::SideEffectFlag;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    source: &str,
    deps: Vec<Dependency>,
    module_type: ModuleType,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type,
      dependencies: deps,
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

  fn make_dep(target: &ModuleSpecifier, kind: ImportKind) -> Dependency {
    Dependency {
      specifier: target.to_string(),
      resolved: target.clone(),
      kind,
      range: None,
    }
  }

  #[test]
  fn test_production_esm_single_module() {
    let entry = spec("entry.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "export const foo = 42;",
      vec![],
      ModuleType::Esm,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph);

    // Should NOT include CJS helpers for pure ESM.
    assert!(!output.code.contains("__commonJS"));
    assert!(output.code.contains("export const foo = 42;"));
  }

  #[test]
  fn test_production_cjs_module() {
    let entry = spec("entry.cjs");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "module.exports = 42;",
      vec![],
      ModuleType::Cjs,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph);

    // Should include CJS helpers.
    assert!(output.code.contains("var __commonJS"));
    assert!(output.code.contains("var require_entry = __commonJS"));
    assert!(output.code.contains("module.exports = 42;"));
    // Entry should be invoked.
    assert!(output.code.contains("require_entry()"));
  }

  #[test]
  fn test_production_mixed_esm_cjs() {
    let entry = spec("entry.js");
    let dep = spec("dep.cjs");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "import dep from './dep';\nconsole.log(dep);",
      vec![make_dep(&dep, ImportKind::Import)],
      ModuleType::Esm,
    ));
    graph.add_module(make_module(
      &dep,
      "module.exports = { value: 1 };",
      vec![],
      ModuleType::Cjs,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph);

    // CJS helpers emitted because dep is CJS.
    assert!(output.code.contains("var __commonJS"));
    // CJS module wrapped.
    assert!(output.code.contains("var require_dep = __commonJS"));
    // ESM entry emitted as plain code.
    assert!(output.code.contains("console.log(dep)"));
  }

  #[test]
  fn test_require_var_name_sanitization() {
    let s = spec("my-module.js");
    assert_eq!(make_require_var_name(&s), "require_my_module");

    let s2 = spec("lib/utils.ts");
    assert_eq!(make_require_var_name(&s2), "require_utils");
  }
}
