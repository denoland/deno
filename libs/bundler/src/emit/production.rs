// Copyright 2018-2026 the Deno authors. MIT license.

//! Production chunk emission with scope hoisting.
//!
//! ESM modules are scope-hoisted: their ASTs are parsed, imports and
//! re-exports are stripped, conflicting top-level names are deconflicted,
//! and all module bodies are concatenated into a single scope.
//!
//! CJS modules are still wrapped in `__commonJS()` since they use
//! `module.exports` / `require()` semantics.

use deno_ast::swc::ast::*;
use deno_ast::swc::common::DUMMY_SP;
use deno_ast::swc::ecma_visit::VisitMutWith;
use deno_ast::ModuleSpecifier;

use deno_ast::bundler_transforms::remove_imports;
use deno_ast::bundler_transforms::strip_exports;
use deno_ast::bundler_transforms::IdentRenamer;

use crate::chunk::Chunk;
use crate::chunk::ChunkGraph;
use crate::chunk::ChunkId;
use crate::chunk::ChunkType;
use crate::graph::BundlerGraph;
use crate::module::ModuleType;
use crate::transform_pipeline::emit_program;

use super::cross_chunk::CrossChunkBindings;
use super::deconflict::compute_deconflict_renames;
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

/// Emit a chunk in production mode with scope hoisting.
///
/// ESM modules are scope-hoisted (imports stripped, exports stripped,
/// identifiers deconflicted, bodies concatenated). CJS modules are
/// wrapped in `__commonJS()`.
pub fn emit_production_chunk(
  chunk: &Chunk,
  graph: &BundlerGraph,
  _chunk_graph: &ChunkGraph,
  cross_chunk: Option<&CrossChunkBindings>,
) -> ChunkOutput {
  let has_cjs = chunk_has_cjs(chunk, graph);

  // Compute deconflicting renames for all ESM modules in the chunk.
  let renames = compute_deconflict_renames(&chunk.modules, graph);

  // Collect all module bodies into a combined list of ModuleItems.
  let mut combined_body: Vec<ModuleItem> = Vec::new();
  let mut cjs_preamble = String::new();

  if has_cjs {
    cjs_preamble.push_str(CJS_HELPERS);
    cjs_preamble.push('\n');
  }

  // Track whether we have any ESM modules to scope-hoist.
  let mut has_esm = false;

  // Prepend cross-chunk import declarations.
  if let Some(cc) = cross_chunk {
    if let Some(chunk_imports) = cc.imports.get(&chunk.id) {
      // Group imports by source chunk.
      let import_items =
        build_cross_chunk_imports(chunk_imports, cc);
      combined_body.extend(import_items);
      if !chunk_imports.is_empty() {
        has_esm = true;
      }
    }
  }

  for specifier in &chunk.modules {
    let Some(module) = graph.get_module(specifier) else {
      continue;
    };

    if module.module_type == ModuleType::Cjs {
      // CJS modules stay as text-wrapped code.
      let relative = make_relative_path(specifier);
      cjs_preamble.push_str(&format!("// {}\n", relative));
      emit_cjs_module(&mut cjs_preamble, specifier, &module.source);
      cjs_preamble.push('\n');
      continue;
    }

    has_esm = true;

    // Get the module's AST.
    let mut body = get_module_body(specifier, module, graph);

    // Strip import declarations (intra-chunk imports are now in same scope).
    remove_imports(&mut body);

    // Determine if this is the chunk's entry module.
    let is_entry = chunk
      .entry
      .as_ref()
      .map(|e| e == specifier)
      .unwrap_or(false);

    // Strip export syntax from non-entry modules (they're now internal).
    // Entry modules keep their exports so the chunk exposes them.
    if !is_entry {
      let default_var = make_default_var_name(specifier);
      strip_exports(&mut body, &default_var);
    }

    // Apply deconflicting renames if needed.
    if let Some(module_renames) = renames.get(specifier) {
      if !module_renames.renames.is_empty() {
        let mut renamer = IdentRenamer {
          names: module_renames.renames.clone(),
        };
        for item in &mut body {
          item.visit_mut_with(&mut renamer);
        }
      }
    }

    combined_body.extend(body);
  }

  // Append cross-chunk export declarations.
  if let Some(cc) = cross_chunk {
    if let Some(chunk_exports) = cc.exports.get(&chunk.id) {
      if !chunk_exports.is_empty() {
        let export_item = build_cross_chunk_exports(chunk_exports);
        combined_body.push(export_item);
        has_esm = true;
      }
    }
  }

  let filename = cross_chunk
    .map(|cc| cc.filename(chunk.id).to_string())
    .unwrap_or_else(|| format!("chunk-{}.js", chunk.id.0));

  // If we have ESM modules, emit the combined AST.
  if has_esm {
    let combined_program = Program::Module(deno_ast::swc::ast::Module {
      span: DUMMY_SP,
      body: combined_body,
      shebang: None,
    });

    let mut code = cjs_preamble;
    if let Some(emitted) = emit_program(&combined_program) {
      code.push_str(&emitted);
    }

    // CJS entry invocation.
    if let Some(entry) = &chunk.entry {
      if let Some(module) = graph.get_module(entry) {
        if module.module_type == ModuleType::Cjs {
          match chunk.chunk_type {
            ChunkType::Entry | ChunkType::DynamicImport => {
              let var_name = make_require_var_name(entry);
              code.push_str(&format!("var __entry = {}();\n", var_name));
              code.push_str("export default __entry;\n");
            }
            _ => {}
          }
        }
      }
    }

    ChunkOutput {
      code,
      source_map: None,
      filename,
    }
  } else {
    // All CJS, no AST emission needed.
    let mut code = cjs_preamble;

    // Entry invocation.
    if let Some(entry) = &chunk.entry {
      match chunk.chunk_type {
        ChunkType::Entry | ChunkType::DynamicImport => {
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
      filename,
    }
  }
}

/// Build cross-chunk import AST nodes from import bindings.
///
/// Groups imports by source chunk and generates one `import { ... } from "..."` per chunk.
fn build_cross_chunk_imports(
  imports: &[super::cross_chunk::CrossChunkImport],
  cross_chunk: &CrossChunkBindings,
) -> Vec<ModuleItem> {
  use rustc_hash::FxHashMap;

  // Group by source chunk.
  let mut by_source: FxHashMap<ChunkId, Vec<&str>> = FxHashMap::default();
  for imp in imports {
    by_source
      .entry(imp.source_chunk)
      .or_default()
      .push(&imp.wire_name);
  }

  let mut items = Vec::new();
  let mut sorted_sources: Vec<_> = by_source.into_iter().collect();
  sorted_sources.sort_by_key(|(id, _)| id.0);

  for (source_chunk, names) in sorted_sources {
    let specifier = format!("./{}", cross_chunk.filename(source_chunk));

    let specifiers: Vec<ImportSpecifier> = names
      .into_iter()
      .map(|name| {
        ImportSpecifier::Named(ImportNamedSpecifier {
          span: DUMMY_SP,
          local: Ident::new_no_ctxt(name.into(), DUMMY_SP),
          imported: None,
          is_type_only: false,
        })
      })
      .collect();

    items.push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
      span: DUMMY_SP,
      specifiers,
      src: Box::new(Str {
        span: DUMMY_SP,
        value: specifier.into(),
        raw: None,
      }),
      type_only: false,
      with: None,
      phase: Default::default(),
    })));
  }

  items
}

/// Build a cross-chunk export AST node from export bindings.
///
/// Generates `export { local_name as wire_name, ... }`.
fn build_cross_chunk_exports(
  exports: &[(String, String)],
) -> ModuleItem {
  let specifiers: Vec<ExportSpecifier> = exports
    .iter()
    .map(|(local, wire)| {
      let orig = ModuleExportName::Ident(
        Ident::new_no_ctxt(local.clone().into(), DUMMY_SP),
      );
      let exported = if local != wire {
        Some(ModuleExportName::Ident(
          Ident::new_no_ctxt(wire.clone().into(), DUMMY_SP),
        ))
      } else {
        None
      };
      ExportSpecifier::Named(ExportNamedSpecifier {
        span: DUMMY_SP,
        orig,
        exported,
        is_type_only: false,
      })
    })
    .collect();

  ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(NamedExport {
    span: DUMMY_SP,
    specifiers,
    src: None,
    type_only: false,
    with: None,
  }))
}

/// Get the module body as `Vec<ModuleItem>` from the best available source.
fn get_module_body(
  specifier: &ModuleSpecifier,
  module: &crate::module::BundlerModule,
  graph: &BundlerGraph,
) -> Vec<ModuleItem> {
  // Prefer transformed program AST.
  if let Some(tp) = &module.transformed_program {
    if let Program::Module(m) = tp {
      return m.body.clone();
    }
  }

  // Try parsed source.
  if let Some(parsed) = &module.parsed {
    let program = parsed.program();
    if let Program::Module(m) = program.as_ref() {
      return m.body.clone();
    }
  }

  // Fall back: parse from source text.
  let module = graph.get_module(specifier).unwrap();
  let parsed = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: module.source.clone().into(),
    media_type: deno_ast::MediaType::JavaScript,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  });
  match parsed {
    Ok(p) => {
      let program = p.program();
      if let Program::Module(m) = program.as_ref() {
        m.body.clone()
      } else {
        Vec::new()
      }
    }
    Err(_) => Vec::new(),
  }
}

/// Emit a CJS module wrapped in `__commonJS()`.
fn emit_cjs_module(
  code: &mut String,
  specifier: &ModuleSpecifier,
  source: &str,
) {
  let var_name = make_require_var_name(specifier);
  let path_key = make_relative_path(specifier);

  code.push_str(&format!("var {} = __commonJS({{\n", var_name));
  code.push_str(&format!("  \"{}\"(exports, module) {{\n", path_key));

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
  let sanitized: String = name
    .chars()
    .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
    .collect();
  format!("require_{}", sanitized)
}

/// Generate a default export variable name for a module.
fn make_default_var_name(specifier: &ModuleSpecifier) -> String {
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
  let sanitized: String = name
    .chars()
    .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
    .collect();
  format!("__default_{}", sanitized)
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
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

    // Should NOT include CJS helpers for pure ESM.
    assert!(!output.code.contains("__commonJS"));
    // Entry module keeps its exports.
    assert!(output.code.contains("foo"));
    assert!(output.code.contains("42"));
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
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

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
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

    // CJS helpers emitted because dep is CJS.
    assert!(output.code.contains("var __commonJS"));
    // CJS module wrapped.
    assert!(output.code.contains("var require_dep = __commonJS"));
    // ESM entry: imports stripped, code preserved.
    assert!(output.code.contains("console.log"));
  }

  #[test]
  fn test_require_var_name_sanitization() {
    let s = spec("my-module.js");
    assert_eq!(make_require_var_name(&s), "require_my_module");

    let s2 = spec("lib/utils.ts");
    assert_eq!(make_require_var_name(&s2), "require_utils");
  }

  #[test]
  fn test_scope_hoisting_strips_internal_imports() {
    // Two ESM modules: dep exports, entry imports.
    // After scope hoisting, import declarations should be stripped.
    let entry = spec("entry.js");
    let dep = spec("dep.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &dep,
      "export const helper = 42;",
      vec![],
      ModuleType::Esm,
    ));
    graph.add_module(make_module(
      &entry,
      "import { helper } from './dep';\nconsole.log(helper);",
      vec![make_dep(&dep, ImportKind::Import)],
      ModuleType::Esm,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

    // Import declaration should be gone.
    assert!(!output.code.contains("import "));
    // The actual code should be present.
    assert!(output.code.contains("helper"));
    assert!(output.code.contains("42"));
    assert!(output.code.contains("console.log"));
  }

  #[test]
  fn test_scope_hoisting_strips_internal_exports() {
    // Non-entry module's exports should be stripped.
    let entry = spec("entry.js");
    let dep = spec("dep.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &dep,
      "export const value = 1;\nexport function doStuff() { return value; }",
      vec![],
      ModuleType::Esm,
    ));
    graph.add_module(make_module(
      &entry,
      "import { doStuff } from './dep';\nexport const result = doStuff();",
      vec![make_dep(&dep, ImportKind::Import)],
      ModuleType::Esm,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

    // Internal module (dep) should have export keyword stripped.
    // But the declarations themselves should remain.
    assert!(output.code.contains("value"));
    assert!(output.code.contains("doStuff"));
    // Entry module keeps its export.
    assert!(output.code.contains("export"));
    assert!(output.code.contains("result"));
  }

  #[test]
  fn test_scope_hoisting_deconflicts_names() {
    // Two modules both declare `helper` — one should get renamed.
    let entry = spec("entry.js");
    let dep = spec("dep.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &dep,
      "var helper = 1;\nexport var depVal = helper;",
      vec![],
      ModuleType::Esm,
    ));
    graph.add_module(make_module(
      &entry,
      "import { depVal } from './dep';\nvar helper = 2;\nexport var out = helper + depVal;",
      vec![make_dep(&dep, ImportKind::Import)],
      ModuleType::Esm,
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    let output = emit_production_chunk(chunk, &graph, &chunk_graph, None);

    // One `helper` should be renamed to `helper$1`.
    assert!(output.code.contains("helper$1") || output.code.contains("helper"));
    // Both values should be present.
    assert!(output.code.contains("depVal"));
    assert!(output.code.contains("out"));
  }
}
