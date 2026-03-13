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
use crate::config::SourceMapMode;
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
  source_map_mode: SourceMapMode,
) -> ChunkOutput {
  let mut code = String::new();
  let mut line_count: u32 = 0;
  // Collected (module_source_map_json, body_start_line) for later composition.
  let mut module_maps: Vec<(&str, u32)> = Vec::new();

  // Preamble: grab the runtime.
  code.push_str("var __d = globalThis.__dbundle;\n");
  line_count += 1;

  // Emit each module wrapped in __d.update().
  for specifier in &chunk.modules {
    if let Some(module) = graph.get_module(specifier) {
      let mid = graph.module_index(specifier).unwrap_or(0);
      let relative = make_relative_path(specifier);

      code.push_str(&format!("// Module: {}\n", relative));
      line_count += 1;

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
      line_count += 1;

      // CJS preamble.
      if is_cjs {
        code.push_str("  var module = { exports: exports };\n");
        line_count += 1;
      }

      // Record where the module body starts for source map offsetting.
      let body_start_line = line_count;
      if source_map_mode != SourceMapMode::None {
        if let Some(ref sm) = module.source_map {
          module_maps.push((sm, body_start_line));
        }
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
        line_count += 1;
      }

      // CJS epilogue: sync module.exports back.
      if is_cjs {
        code.push_str(&format!(
          "  if (exports !== module.exports) __d.setCjsExports({}, module.exports);\n",
          mid
        ));
        line_count += 1;
      }

      // Footer.
      if is_async {
        code.push_str("}, true);\n");
      } else {
        code.push_str("});\n");
      }
      line_count += 1;
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

  // Compose chunk-level source map from per-module source maps.
  let source_map = if source_map_mode != SourceMapMode::None
    && !module_maps.is_empty()
  {
    compose_source_map(&module_maps)
  } else {
    None
  };

  // Inline source map if requested.
  if source_map_mode == SourceMapMode::Inline {
    if let Some(ref map_json) = source_map {
      append_inline_source_map(&mut code, map_json);
    }
  }

  ChunkOutput {
    code,
    source_map: if source_map_mode == SourceMapMode::Inline {
      None // already inlined
    } else {
      source_map
    },
  }
}

/// Emit a single module for HMR update (standalone, not in a chunk).
///
/// Returns the `__d.update(...)` wrapper code and an optional source map.
pub fn emit_hmr_update(
  specifier: &ModuleSpecifier,
  graph: &BundlerGraph,
  source_map_mode: SourceMapMode,
) -> Option<(String, Option<String>)> {
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

  // 1 line for header + 1 line for CJS preamble if present.
  let body_start_line: u32 = if is_cjs { 2 } else { 1 };

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

  // Compose source map for this single module.
  let source_map = if source_map_mode != SourceMapMode::None {
    if let Some(ref sm) = module.source_map {
      let maps = [(sm.as_str(), body_start_line)];
      compose_source_map(&maps)
    } else {
      None
    }
  } else {
    None
  };

  // Inline source map if requested.
  if source_map_mode == SourceMapMode::Inline {
    if let Some(ref map_json) = source_map {
      append_inline_source_map(&mut code, map_json);
    }
  }

  let final_map = if source_map_mode == SourceMapMode::Inline {
    None
  } else {
    source_map
  };

  Some((code, final_map))
}

/// Compose a chunk-level source map from per-module source maps.
///
/// Each entry is `(module_source_map_json, body_start_line)` where
/// `body_start_line` is the 0-indexed line in the chunk output where
/// the module body starts. Module body lines are indented by 2 spaces.
fn compose_source_map(module_maps: &[(&str, u32)]) -> Option<String> {
  use deno_ast::swc::sourcemap::SourceMapBuilder;

  let mut builder = SourceMapBuilder::new(None);

  for &(map_json, body_start_line) in module_maps {
    let sm = deno_ast::swc::sourcemap::SourceMap::from_reader(
      map_json.as_bytes(),
    )
    .ok()?;

    // Build a mapping from source IDs in this module's map to the
    // combined builder's source IDs.
    let mut src_id_map: Vec<u32> =
      Vec::with_capacity(sm.get_source_count() as usize);
    for i in 0..sm.get_source_count() {
      let src_name = sm
        .get_source(i)
        .map(|s| s.to_string())
        .unwrap_or_default();
      let new_id = builder.add_source(src_name.into());
      if let Some(contents) = sm.get_source_contents(i) {
        builder
          .set_source_contents(new_id, Some(contents.to_string().into()));
      }
      src_id_map.push(new_id);
    }

    // Build a mapping from name IDs.
    let mut name_id_map: Vec<u32> =
      Vec::with_capacity(sm.get_name_count() as usize);
    for i in 0..sm.get_name_count() {
      let name = sm
        .get_name(i)
        .map(|s| s.to_string())
        .unwrap_or_default();
      let new_id = builder.add_name(name.into());
      name_id_map.push(new_id);
    }

    // Copy tokens with line/column offsets.
    for token in sm.tokens() {
      let raw = token.get_raw_token();
      let dst_line = raw.dst_line + body_start_line;
      // Add 2 columns for the indentation on non-empty lines.
      let dst_col = raw.dst_col + 2;
      let src_id = *src_id_map.get(raw.src_id as usize).unwrap_or(&raw.src_id);
      let name_id = if raw.name_id != !0 {
        Some(
          *name_id_map
            .get(raw.name_id as usize)
            .unwrap_or(&raw.name_id),
        )
      } else {
        None
      };

      builder.add_raw(
        dst_line,
        dst_col,
        raw.src_line,
        raw.src_col,
        Some(src_id),
        name_id,
        false,
      );
    }
  }

  let sm = builder.into_sourcemap();
  let mut buf = Vec::new();
  sm.to_writer(&mut buf).ok()?;
  String::from_utf8(buf).ok()
}

/// Append an inline source map comment to the code string.
fn append_inline_source_map(code: &mut String, map_json: &str) {
  use deno_ast::swc::sourcemap::SourceMap;
  // Re-serialize to get a data URL.
  if let Ok(sm) = SourceMap::from_reader(map_json.as_bytes()) {
    if let Ok(data_url) = sm.to_data_url() {
      code.push_str("//# sourceMappingURL=");
      code.push_str(&data_url);
      code.push('\n');
    }
  }
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
      source_map: None,
      source_hash: None,
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
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph, SourceMapMode::None);

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
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph, SourceMapMode::None);

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
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph, SourceMapMode::None);

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
    let output = emit_dev_chunk(chunk, &graph, &chunk_graph, SourceMapMode::None);

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

    let (code, _source_map) = emit_hmr_update(&entry, &graph, SourceMapMode::None).unwrap();
    assert!(code.contains("__d.update(0,"));
    assert!(code.contains("console.log('updated');"));
    assert!(!code.contains("__d.require"));
  }
}
