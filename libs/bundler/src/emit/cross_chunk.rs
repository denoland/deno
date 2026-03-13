// Copyright 2018-2026 the Deno authors. MIT license.

//! Cross-chunk binding analysis and content-hashed filename generation.
//!
//! When code splitting places modules in different chunks, imports that
//! cross chunk boundaries need to become explicit `import`/`export`
//! statements between chunk files. This module computes which symbols
//! need to cross chunk boundaries and assigns wire names for them.

use std::hash::Hash;
use std::hash::Hasher;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::chunk::ChunkGraph;
use crate::chunk::ChunkId;
use crate::graph::BundlerGraph;
use crate::js::module_info::ImportedName;
use crate::module::ModuleType;

/// A single cross-chunk import binding.
#[derive(Debug, Clone)]
pub struct CrossChunkImport {
  /// The chunk to import from.
  pub source_chunk: ChunkId,
  /// The wire name used in `import { <wire_name> } from "./chunk-X.js"`.
  pub wire_name: String,
}

/// All cross-chunk binding information for a chunk graph.
#[derive(Debug, Clone)]
pub struct CrossChunkBindings {
  /// For each chunk: list of imports it needs from other chunks.
  pub imports: FxHashMap<ChunkId, Vec<CrossChunkImport>>,
  /// For each chunk: list of (local_name, wire_name) it must export
  /// for other chunks to consume.
  pub exports: FxHashMap<ChunkId, Vec<(String, String)>>,
  /// Filenames for each chunk (content-hashed).
  pub chunk_filenames: FxHashMap<ChunkId, String>,
}

impl CrossChunkBindings {
  /// Get the filename for a chunk.
  pub fn filename(&self, chunk_id: ChunkId) -> &str {
    self.chunk_filenames.get(&chunk_id).map(|s| s.as_str()).unwrap_or("chunk.js")
  }
}

/// Compute cross-chunk bindings for all chunks in the graph.
///
/// For each module's imports, checks if the target module is in a
/// different chunk. If so, records the symbol as a cross-chunk binding
/// that needs explicit import/export statements.
pub fn compute_cross_chunk_bindings(
  chunk_graph: &ChunkGraph,
  graph: &BundlerGraph,
) -> CrossChunkBindings {
  let mut imports: FxHashMap<ChunkId, Vec<CrossChunkImport>> =
    FxHashMap::default();
  let mut export_map: FxHashMap<ChunkId, FxHashMap<String, String>> =
    FxHashMap::default();

  // Phase 1: Scan all modules for cross-chunk references.
  for chunk in chunk_graph.chunks() {
    for module_spec in &chunk.modules {
      let Some(module) = graph.get_module(module_spec) else {
        continue;
      };

      // Skip CJS modules — their imports are runtime `require()` calls.
      if module.module_type == ModuleType::Cjs {
        continue;
      }

      let Some(mi) = &module.module_info else {
        continue;
      };

      for import in &mi.imports {
        // Find the dependency this import comes from.
        let Some(dep) = module
          .dependencies
          .iter()
          .find(|d| d.specifier == import.source)
        else {
          continue;
        };

        // Check which chunk the target module is in.
        let Some(target_chunk_id) = chunk_graph.module_chunk(&dep.resolved)
        else {
          continue;
        };

        // Same chunk → no cross-chunk binding needed.
        if target_chunk_id == chunk.id {
          continue;
        }

        // Determine the wire name from the import.
        let wire_name = match &import.imported {
          ImportedName::Named(n) => n.clone(),
          ImportedName::Default => "default".to_string(),
          ImportedName::Namespace => {
            // Namespace imports are complex — for now, skip them.
            // They'll fall back to runtime behavior.
            continue;
          }
        };

        // Record import in the importing chunk.
        imports
          .entry(chunk.id)
          .or_default()
          .push(CrossChunkImport {
            source_chunk: target_chunk_id,
            wire_name: wire_name.clone(),
          });

        // Find the local name in the target chunk for this export.
        // Use the wire name as the local name — it matches the
        // exported declaration name (before deconflicting).
        let local_name = wire_name.clone();

        // Record export in the target chunk.
        export_map
          .entry(target_chunk_id)
          .or_default()
          .entry(wire_name.clone())
          .or_insert(local_name);
      }
    }
  }

  // Phase 2: Deduplicate imports (same wire_name from same source chunk).
  for chunk_imports in imports.values_mut() {
    let mut seen: FxHashSet<(u32, String)> = FxHashSet::default();
    chunk_imports.retain(|imp| {
      seen.insert((imp.source_chunk.0, imp.wire_name.clone()))
    });
  }

  // Phase 3: Build exports list from the map.
  let mut exports: FxHashMap<ChunkId, Vec<(String, String)>> =
    FxHashMap::default();
  for (chunk_id, name_map) in export_map {
    let mut export_list: Vec<(String, String)> = name_map
      .into_iter()
      .map(|(wire, local)| (local, wire))
      .collect();
    export_list.sort_by(|a, b| a.1.cmp(&b.1));
    exports.insert(chunk_id, export_list);
  }

  // Phase 4: Compute content-hashed filenames.
  let chunk_filenames = compute_chunk_filenames(chunk_graph, graph);

  CrossChunkBindings {
    imports,
    exports,
    chunk_filenames,
  }
}

/// Compute content-hashed filenames for all chunks.
fn compute_chunk_filenames(
  chunk_graph: &ChunkGraph,
  graph: &BundlerGraph,
) -> FxHashMap<ChunkId, String> {
  let mut filenames = FxHashMap::default();

  for chunk in chunk_graph.chunks() {
    let filename = compute_single_chunk_filename(chunk, graph);
    filenames.insert(chunk.id, filename);
  }

  filenames
}

/// Compute a content-hashed filename for a single chunk.
fn compute_single_chunk_filename(
  chunk: &crate::chunk::Chunk,
  graph: &BundlerGraph,
) -> String {
  // Hash the chunk's module sources for content-based cache busting.
  let mut hasher = rustc_hash::FxHasher::default();
  for spec in &chunk.modules {
    if let Some(m) = graph.get_module(spec) {
      m.source.hash(&mut hasher);
    }
  }
  let hash = hasher.finish();
  let hash_str = format!("{:08x}", hash as u32);

  if let Some(entry) = &chunk.entry {
    // Entry/DynamicImport chunks: use entry module name.
    let name = entry
      .path_segments()
      .and_then(|s| s.last())
      .unwrap_or("chunk");
    let name = name
      .strip_suffix(".js")
      .or_else(|| name.strip_suffix(".ts"))
      .or_else(|| name.strip_suffix(".tsx"))
      .or_else(|| name.strip_suffix(".jsx"))
      .or_else(|| name.strip_suffix(".mjs"))
      .or_else(|| name.strip_suffix(".cjs"))
      .unwrap_or(name);
    let sanitized: String = name
      .chars()
      .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
      .collect();
    format!("{}-{}.js", sanitized, hash_str)
  } else {
    // Shared chunks: hash-only name.
    format!("chunk-{}.js", hash_str)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::chunk::build_chunk_graph;
  use crate::config::EnvironmentId;
  use deno_ast::ModuleSpecifier;
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
  fn test_no_cross_chunk_in_single_chunk() {
    let entry = spec("entry.js");
    let dep = spec("dep.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &dep,
      "export const foo = 1;",
      vec![],
    ));
    graph.add_module(make_module(
      &entry,
      "import { foo } from './dep';\nconsole.log(foo);",
      vec![make_dep(&dep, ImportKind::Import)],
    ));
    graph.add_entry(entry.clone());

    // Both in same chunk → no cross-chunk bindings.
    let chunk_graph = build_chunk_graph(&graph);
    let bindings = compute_cross_chunk_bindings(&chunk_graph, &graph);

    assert!(bindings.imports.is_empty() || bindings.imports.values().all(|v| v.is_empty()));
    assert!(bindings.exports.is_empty() || bindings.exports.values().all(|v| v.is_empty()));
  }

  #[test]
  fn test_cross_chunk_with_dynamic_import() {
    // entry.js dynamically imports lazy.js → separate chunks.
    // lazy.js imports shared.js (which is in lazy's chunk since only
    // reachable from lazy). But if shared is only reachable through
    // the dynamic import, it stays in the lazy chunk.
    let entry = spec("entry.js");
    let lazy = spec("lazy.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "const m = import('./lazy');",
      vec![make_dep(&lazy, ImportKind::DynamicImport)],
    ));
    graph.add_module(make_module(
      &lazy,
      "export const value = 42;",
      vec![],
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    assert_eq!(chunk_graph.chunks().len(), 2);

    // No cross-chunk bindings needed (entry doesn't import specific
    // symbols from lazy — it uses dynamic import).
    let bindings = compute_cross_chunk_bindings(&chunk_graph, &graph);
    assert!(bindings.imports.is_empty() || bindings.imports.values().all(|v| v.is_empty()));
  }

  #[test]
  fn test_content_hashed_filenames() {
    let entry = spec("app.js");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      "console.log('hello');",
      vec![],
    ));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let bindings = compute_cross_chunk_bindings(&chunk_graph, &graph);

    let chunk_id = chunk_graph.entry_chunks()[0];
    let filename = bindings.filename(chunk_id);

    // Should be "app-XXXXXXXX.js" format.
    assert!(filename.starts_with("app-"));
    assert!(filename.ends_with(".js"));
    assert!(filename.len() > "app-.js".len());
  }

  #[test]
  fn test_content_hash_changes_with_content() {
    let entry = spec("app.js");

    // Build 1.
    let mut graph1 = BundlerGraph::new(EnvironmentId::new(0));
    graph1.add_module(make_module(&entry, "console.log('v1');", vec![]));
    graph1.add_entry(entry.clone());
    let cg1 = build_chunk_graph(&graph1);
    let b1 = compute_cross_chunk_bindings(&cg1, &graph1);

    // Build 2 with different content.
    let mut graph2 = BundlerGraph::new(EnvironmentId::new(0));
    graph2.add_module(make_module(&entry, "console.log('v2');", vec![]));
    graph2.add_entry(entry.clone());
    let cg2 = build_chunk_graph(&graph2);
    let b2 = compute_cross_chunk_bindings(&cg2, &graph2);

    let f1 = b1.filename(cg1.entry_chunks()[0]);
    let f2 = b2.filename(cg2.entry_chunks()[0]);

    // Different content → different hash.
    assert_ne!(f1, f2);
  }

  #[test]
  fn test_shared_chunk_filename() {
    // Two entries sharing a module → shared chunk.
    let a = spec("a.js");
    let b = spec("b.js");
    let shared = spec("shared.js");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &a,
      "import './shared';",
      vec![make_dep(&shared, ImportKind::Import)],
    ));
    graph.add_module(make_module(
      &b,
      "import './shared';",
      vec![make_dep(&shared, ImportKind::Import)],
    ));
    graph.add_module(make_module(&shared, "export const x = 1;", vec![]));
    graph.add_entry(a.clone());
    graph.add_entry(b.clone());

    let chunk_graph = build_chunk_graph(&graph);
    let bindings = compute_cross_chunk_bindings(&chunk_graph, &graph);

    // Find the shared chunk.
    for chunk in chunk_graph.chunks() {
      let filename = bindings.filename(chunk.id);
      if chunk.entry.is_none() {
        // Shared chunk → "chunk-XXXXXXXX.js" format.
        assert!(filename.starts_with("chunk-"));
      } else {
        // Entry chunk → has entry name.
        assert!(filename.contains("-"));
      }
      assert!(filename.ends_with(".js"));
    }
  }
}
