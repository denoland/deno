// Copyright 2018-2026 the Deno authors. MIT license.

//! Graph-aware (cross-file) lint support.
//!
//! This builds a compact, read-only view of a `deno_graph::ModuleGraph` that
//! is handed to opt-in `createGraphRule` plugin hooks. It runs once per
//! `deno lint` invocation, after the per-file phase, and only when an enabled
//! plugin rule declares a graph hook.
//!
//! v1 is graph-only (no type checker). Per module we expose: the resolved
//! specifier, media type, resolved import edges, and resolved re-export edges
//! (`export * from` / `export { x } from`). Re-export edges are extracted from
//! each module's retained `ParsedSource` (via the CLI's `ParsedSourceCache`),
//! because `deno_graph` folds imports and re-exports into a single
//! `dependencies` map and does not distinguish them at the `Module` level.

use std::collections::HashMap;

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_ast::swc::ast::ModuleDecl;
use deno_ast::swc::ast::ModuleItem;
use deno_ast::swc::ast::Program;
use deno_ast::swc::common::Span;
use deno_core::serde_json;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_resolver::cache::ParsedSourceCache;
use serde::Serialize;

use crate::util::text_encoding::Utf16Map;

pub struct SerializedGraph {
  /// JSON payload handed to the JS runtime (see `LintModuleGraph` in
  /// `lib.deno.unstable.d.ts`).
  pub json: String,
  /// Per-module `(SourceTextInfo, Utf16Map)` keyed by specifier, so that a
  /// graph rule's `context.report({ specifier, range })` can be attributed
  /// back to a concrete file + span even though the report targets a module
  /// other than any single "current" file.
  pub files: HashMap<ModuleSpecifier, (SourceTextInfo, Utf16Map)>,
}

#[derive(Serialize)]
struct JsonGraph {
  roots: Vec<String>,
  modules: Vec<JsonModule>,
}

#[derive(Serialize)]
struct JsonModule {
  specifier: String,
  #[serde(rename = "mediaType")]
  media_type: String,
  dependencies: Vec<JsonDependency>,
  exports: Vec<JsonExport>,
}

#[derive(Serialize)]
struct JsonDependency {
  specifier: String,
  resolved: Option<String>,
  kind: &'static str,
  range: [u32; 2],
}

#[derive(Serialize)]
struct JsonExport {
  name: String,
  kind: &'static str,
  from: Option<String>,
  range: [u32; 2],
}

/// swc stores spans 1-indexed; convert to 0-based UTF-16 offsets within the
/// module (matching the convention used by the per-file AST serializer).
fn span_to_utf16(span: &Span, utf16_map: &Utf16Map) -> [u32; 2] {
  if span.lo.0 == 0 && span.hi.0 == 0 {
    return [0, 0];
  }
  let lo = span.lo.0.saturating_sub(1);
  let hi = span.hi.0.saturating_sub(1);
  let to16 = |v: u32| -> u32 {
    utf16_map
      .utf8_to_utf16_offset(v.into())
      .map(|t| u32::from(t))
      .unwrap_or(v)
  };
  [to16(lo), to16(hi)]
}

/// Extract resolved re-export edges from a module's parsed AST. Returns
/// `(exports, ())`. Each `export * from "x"` / `export { a } from "x"` yields
/// one entry whose `from` is the resolved target specifier (looked up in the
/// module's `deno_graph` dependency map keyed by the raw specifier string).
fn collect_exports(
  parsed_source: &ParsedSource,
  js_module: &deno_graph::JsModule,
  utf16_map: &Utf16Map,
) -> Vec<JsonExport> {
  let mut exports = Vec::new();
  let program = parsed_source.program();
  let Program::Module(module) = program.as_ref() else {
    return exports;
  };

  let resolve = |raw: &str| -> Option<String> {
    js_module
      .dependencies
      .get(raw)
      .and_then(|dep| dep.get_code())
      .map(|s| s.to_string())
  };

  for item in &module.body {
    let ModuleItem::ModuleDecl(decl) = item else {
      continue;
    };
    match decl {
      // `export * from "..."`
      ModuleDecl::ExportAll(node) => {
        let raw = node.src.value.to_string_lossy().into_owned();
        exports.push(JsonExport {
          name: "*".to_string(),
          kind: "reexport",
          from: resolve(&raw),
          range: span_to_utf16(&node.span, utf16_map),
        });
      }
      // `export { a, b } from "..."` (and `export * as ns from "..."`)
      ModuleDecl::ExportNamed(node) => {
        if let Some(src) = &node.src {
          let raw = src.value.to_string_lossy().into_owned();
          exports.push(JsonExport {
            name: "*".to_string(),
            kind: "reexport",
            from: resolve(&raw),
            range: span_to_utf16(&node.span, utf16_map),
          });
        }
      }
      _ => {}
    }
  }

  exports
}

/// Build a serialized, read-only view of `graph` plus the per-module source
/// info needed to attribute reported diagnostics.
pub fn serialize_graph(
  graph: &ModuleGraph,
  parsed_source_cache: &ParsedSourceCache,
) -> SerializedGraph {
  let mut modules = Vec::new();
  let mut files = HashMap::new();

  for module in graph.modules() {
    let Module::Js(js_module) = module else {
      continue;
    };

    let parsed_source =
      match parsed_source_cache.get_parsed_source_from_js_module(js_module) {
        Ok(ps) => ps,
        Err(_) => continue,
      };
    let source_text_info = parsed_source.text_info_lazy().clone();
    let utf16_map = Utf16Map::new(parsed_source.text().as_ref());

    let dependencies = js_module
      .dependencies
      .iter()
      .map(|(raw, dep)| JsonDependency {
        specifier: raw.clone(),
        resolved: dep.get_code().map(|s| s.to_string()),
        kind: if dep.is_dynamic { "dynamic" } else { "static" },
        // Dependency source ranges are line/col in deno_graph; for the v1
        // slice we leave them as whole-file. Rules attribute to export ranges
        // (below), which carry real spans.
        range: [0, 0],
      })
      .collect();

    let exports = collect_exports(&parsed_source, js_module, &utf16_map);

    modules.push(JsonModule {
      specifier: js_module.specifier.to_string(),
      media_type: js_module.media_type.to_string(),
      dependencies,
      exports,
    });

    files.insert(js_module.specifier.clone(), (source_text_info, utf16_map));
  }

  let roots = graph.roots.iter().map(|r| r.to_string()).collect();
  let json_graph = JsonGraph { roots, modules };
  let json = serde_json::to_string(&json_graph)
    .unwrap_or_else(|_| "{\"roots\":[],\"modules\":[]}".to_string());

  SerializedGraph { json, files }
}
