// Copyright 2018-2026 the Deno authors. MIT license.

//! Chunk graph for code splitting.
//!
//! Splits a module graph into chunks based on entry points and dynamic imports.
//! The algorithm:
//! 1. Compute reachability from each chunk root (entry + dynamic imports)
//! 2. Assign modules to chunks based on reachability patterns
//! 3. Topologically sort modules within each chunk
//! 4. Compute cross-chunk imports and break cycles
//! 5. Merge single-user shared chunks back into their sole importer

use std::collections::HashSet;
use deno_ast::ModuleSpecifier;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::dependency::ImportKind;
use crate::graph::BundlerGraph;

/// Unique identifier for a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId(pub u32);

/// The type of a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkType {
  /// Entry point chunk (one per config entry).
  Entry,
  /// Chunk created by a dynamic `import()` boundary.
  DynamicImport,
  /// Chunk containing modules shared by multiple chunks.
  Shared,
  /// A single static asset (image, font, media, etc.).
  Asset,
}

/// A chunk is a group of modules bundled together.
#[derive(Debug, Clone)]
pub struct Chunk {
  /// Unique identifier for this chunk.
  pub id: ChunkId,
  /// Modules contained in this chunk, in topological (execution) order.
  pub modules: Vec<ModuleSpecifier>,
  /// The entry module, if this is an Entry or DynamicImport chunk.
  pub entry: Option<ModuleSpecifier>,
  /// Chunks that this chunk depends on (must be loaded first).
  pub imports: Vec<ChunkId>,
  /// The type of chunk.
  pub chunk_type: ChunkType,
  /// Whether this chunk contains a circular dependency cycle.
  pub has_circular: bool,
}

/// The complete graph of chunks created from a module graph.
#[derive(Debug, Clone)]
pub struct ChunkGraph {
  /// All chunks.
  chunks: Vec<Chunk>,
  /// Map from module specifier to which chunk contains it.
  module_to_chunk: FxHashMap<ModuleSpecifier, ChunkId>,
  /// Entry chunks (ordered same as config entries).
  entry_chunks: Vec<ChunkId>,
}

impl ChunkGraph {
  /// Get a chunk by ID.
  pub fn chunk(&self, id: ChunkId) -> &Chunk {
    &self.chunks[id.0 as usize]
  }

  /// Get a mutable reference to a chunk by ID.
  pub fn chunk_mut(&mut self, id: ChunkId) -> &mut Chunk {
    &mut self.chunks[id.0 as usize]
  }

  /// Get which chunk a module belongs to.
  pub fn module_chunk(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ChunkId> {
    self.module_to_chunk.get(specifier).copied()
  }

  /// Get the entry chunks.
  pub fn entry_chunks(&self) -> &[ChunkId] {
    &self.entry_chunks
  }

  /// Iterate over all chunks.
  pub fn chunks(&self) -> &[Chunk] {
    &self.chunks
  }

  /// Number of chunks.
  pub fn len(&self) -> usize {
    self.chunks.len()
  }

  /// Whether the chunk graph is empty.
  pub fn is_empty(&self) -> bool {
    self.chunks.is_empty()
  }
}

/// Build a chunk graph from a bundler graph.
pub fn build_chunk_graph(graph: &BundlerGraph) -> ChunkGraph {
  let mut builder = ChunkGraphBuilder {
    graph,
    chunks: Vec::new(),
    module_to_chunk: FxHashMap::default(),
    entry_chunks: Vec::new(),
    next_chunk_id: 0,
  };
  builder.build();
  ChunkGraph {
    chunks: builder.chunks,
    module_to_chunk: builder.module_to_chunk,
    entry_chunks: builder.entry_chunks,
  }
}

// ---- Internal builder ----

struct ChunkRoot {
  specifier: ModuleSpecifier,
  chunk_type: ChunkType,
}

struct ChunkGraphBuilder<'a> {
  graph: &'a BundlerGraph,
  chunks: Vec<Chunk>,
  module_to_chunk: FxHashMap<ModuleSpecifier, ChunkId>,
  entry_chunks: Vec<ChunkId>,
  next_chunk_id: u32,
}

impl<'a> ChunkGraphBuilder<'a> {
  fn alloc_chunk_id(&mut self) -> ChunkId {
    let id = ChunkId(self.next_chunk_id);
    self.next_chunk_id += 1;
    id
  }

  fn build(&mut self) {
    // Step 1: Compute reachability.
    let (chunk_roots, reachability) = self.compute_reachability();

    if chunk_roots.is_empty() {
      return;
    }

    // Step 2: Assign modules to chunks.
    self.assign_modules_to_chunks(&chunk_roots, &reachability);

    // Step 3: Topological sort within each chunk.
    self.topological_sort_chunks();

    // Step 4: Compute cross-chunk imports.
    self.compute_chunk_imports();

    // Step 4b: Break chunk cycles.
    self.break_chunk_cycles();

    // Step 4c: Merge single-user shared chunks.
    self.merge_single_user_shared_chunks();
  }

  /// Step 1: Compute which modules are reachable from each chunk root.
  /// Returns (chunk_roots, module_reachability) where reachability maps
  /// each module to a bitmask of which roots can reach it.
  fn compute_reachability(
    &self,
  ) -> (Vec<ChunkRoot>, FxHashMap<ModuleSpecifier, u128>) {
    let mut chunk_roots = Vec::new();
    let mut reachability: FxHashMap<ModuleSpecifier, u128> =
      FxHashMap::default();

    // Add configured entry points.
    for entry in self.graph.entries() {
      chunk_roots.push(ChunkRoot {
        specifier: entry.clone(),
        chunk_type: ChunkType::Entry,
      });
    }

    // Discover dynamic imports iteratively.
    let mut discovered = true;
    while discovered {
      discovered = false;
      let root_count = chunk_roots.len();

      for root_idx in 0..root_count {
        let root_spec = chunk_roots[root_idx].specifier.clone();
        let mut visited = FxHashSet::default();
        self.dfs_reachability(
          &root_spec,
          root_idx,
          &mut reachability,
          &mut visited,
        );
      }

      // Find dynamic import targets not yet tracked as roots.
      let mut new_roots = Vec::new();
      for module in self.graph.modules() {
        for dep in &module.dependencies {
          if dep.kind == ImportKind::DynamicImport {
            let target = &dep.resolved;
            let already_root =
              chunk_roots.iter().any(|r| &r.specifier == target);
            if !already_root
              && !new_roots.iter().any(|s: &ModuleSpecifier| s == target)
            {
              new_roots.push(target.clone());
            }
          }
        }
      }

      for spec in new_roots {
        chunk_roots.push(ChunkRoot {
          specifier: spec,
          chunk_type: ChunkType::DynamicImport,
        });
        discovered = true;
      }
    }

    (chunk_roots, reachability)
  }

  /// DFS from a chunk root, marking reachability.
  fn dfs_reachability(
    &self,
    specifier: &ModuleSpecifier,
    root_idx: usize,
    reachability: &mut FxHashMap<ModuleSpecifier, u128>,
    visited: &mut FxHashSet<ModuleSpecifier>,
  ) {
    if !visited.insert(specifier.clone()) {
      return;
    }

    // Mark this module as reachable from root_idx.
    let entry = reachability.entry(specifier.clone()).or_insert(0);
    *entry |= 1u128 << root_idx;

    // Follow static dependencies (not dynamic imports).
    if let Some(module) = self.graph.get_module(specifier) {
      for dep in &module.dependencies {
        if dep.kind != ImportKind::DynamicImport {
          self.dfs_reachability(
            &dep.resolved,
            root_idx,
            reachability,
            visited,
          );
        }
      }
    }
  }

  /// Step 2: Assign modules to chunks based on reachability.
  fn assign_modules_to_chunks(
    &mut self,
    chunk_roots: &[ChunkRoot],
    reachability: &FxHashMap<ModuleSpecifier, u128>,
  ) {
    // Group modules by reachability pattern.
    let mut single_root: FxHashMap<usize, Vec<ModuleSpecifier>> =
      FxHashMap::default();
    let mut shared: FxHashMap<u128, Vec<ModuleSpecifier>> =
      FxHashMap::default();

    for (specifier, &mask) in reachability {
      let bit_count = mask.count_ones();
      if bit_count == 1 {
        let root_idx = mask.trailing_zeros() as usize;
        single_root
          .entry(root_idx)
          .or_default()
          .push(specifier.clone());
      } else if bit_count > 1 {
        shared.entry(mask).or_default().push(specifier.clone());
      }
    }

    // Create entry/dynamic chunks.
    for (root_idx, root) in chunk_roots.iter().enumerate() {
      let id = self.alloc_chunk_id();
      let modules = single_root.remove(&root_idx).unwrap_or_default();

      for spec in &modules {
        self.module_to_chunk.insert(spec.clone(), id);
      }

      self.chunks.push(Chunk {
        id,
        modules,
        entry: Some(root.specifier.clone()),
        imports: Vec::new(),
        chunk_type: root.chunk_type,
        has_circular: false,
      });

      if root.chunk_type == ChunkType::Entry {
        self.entry_chunks.push(id);
      }
    }

    // Create shared chunks.
    let mut sorted_shared: Vec<_> = shared.into_iter().collect();
    sorted_shared.sort_by_key(|(mask, _)| *mask);

    for (_mask, modules) in sorted_shared {
      let id = self.alloc_chunk_id();
      for spec in &modules {
        self.module_to_chunk.insert(spec.clone(), id);
      }
      self.chunks.push(Chunk {
        id,
        modules,
        entry: None,
        imports: Vec::new(),
        chunk_type: ChunkType::Shared,
        has_circular: false,
      });
    }
  }

  /// Step 3: Topologically sort modules within each chunk.
  fn topological_sort_chunks(&mut self) {
    for chunk in &mut self.chunks {
      let module_set: HashSet<&ModuleSpecifier> =
        chunk.modules.iter().collect();
      let mut sorted = Vec::with_capacity(chunk.modules.len());
      let mut visited = HashSet::new();
      let mut in_stack = HashSet::new();
      let mut has_circular = false;

      // Start from entry if present.
      let roots: Vec<ModuleSpecifier> = if let Some(entry) = &chunk.entry {
        vec![entry.clone()]
      } else {
        // For shared chunks, all modules are potential roots.
        chunk.modules.clone()
      };

      for root in &roots {
        if module_set.contains(root) {
          Self::dfs_sort(
            root,
            &module_set,
            &|spec| {
              self
                .graph
                .get_module(spec)
                .map(|m| {
                  m.dependencies
                    .iter()
                    .filter(|d| d.kind != ImportKind::DynamicImport)
                    .map(|d| &d.resolved)
                    .collect::<Vec<_>>()
                })
                .unwrap_or_default()
            },
            &mut visited,
            &mut in_stack,
            &mut sorted,
            &mut has_circular,
          );
        }
      }

      chunk.modules = sorted;
      chunk.has_circular = has_circular;
    }
  }

  fn dfs_sort<'b>(
    specifier: &ModuleSpecifier,
    chunk_modules: &HashSet<&ModuleSpecifier>,
    get_deps: &dyn Fn(&ModuleSpecifier) -> Vec<&'b ModuleSpecifier>,
    visited: &mut HashSet<ModuleSpecifier>,
    in_stack: &mut HashSet<ModuleSpecifier>,
    sorted: &mut Vec<ModuleSpecifier>,
    has_circular: &mut bool,
  ) {
    if visited.contains(specifier) {
      return;
    }
    if !chunk_modules.contains(specifier) {
      return;
    }
    if !in_stack.insert(specifier.clone()) {
      *has_circular = true;
      return;
    }

    for dep in get_deps(specifier) {
      if chunk_modules.contains(dep) {
        Self::dfs_sort(
          dep,
          chunk_modules,
          get_deps,
          visited,
          in_stack,
          sorted,
          has_circular,
        );
      }
    }

    in_stack.remove(specifier);
    visited.insert(specifier.clone());
    sorted.push(specifier.clone());
  }

  /// Step 4: Compute cross-chunk imports.
  fn compute_chunk_imports(&mut self) {
    let module_to_chunk = &self.module_to_chunk;
    for chunk in &mut self.chunks {
      let mut imports = FxHashSet::default();
      for module_spec in &chunk.modules {
        if let Some(module) = self.graph.get_module(module_spec) {
          for dep in &module.dependencies {
            if dep.kind == ImportKind::DynamicImport {
              continue;
            }
            if let Some(&dep_chunk) = module_to_chunk.get(&dep.resolved)
            {
              if dep_chunk != chunk.id {
                imports.insert(dep_chunk);
              }
            }
          }
        }
      }
      let mut imports: Vec<ChunkId> = imports.into_iter().collect();
      imports.sort();
      chunk.imports = imports;
    }
  }

  /// Step 4b: Break cycles in the chunk import graph.
  fn break_chunk_cycles(&mut self) {
    // Simple cycle detection and breaking via priority.
    let mut visited = FxHashSet::default();
    let mut in_stack = FxHashSet::default();
    let mut edges_to_remove = Vec::new();

    for chunk in &self.chunks {
      self.detect_chunk_cycles(
        chunk.id,
        &mut visited,
        &mut in_stack,
        &mut edges_to_remove,
      );
    }

    for (chunk_id, import_to_remove) in edges_to_remove {
      let chunk = &mut self.chunks[chunk_id.0 as usize];
      chunk.imports.retain(|&id| id != import_to_remove);
    }
  }

  fn detect_chunk_cycles(
    &self,
    chunk_id: ChunkId,
    visited: &mut FxHashSet<ChunkId>,
    in_stack: &mut FxHashSet<ChunkId>,
    edges_to_remove: &mut Vec<(ChunkId, ChunkId)>,
  ) {
    if visited.contains(&chunk_id) {
      return;
    }
    if !in_stack.insert(chunk_id) {
      return;
    }

    let chunk = &self.chunks[chunk_id.0 as usize];
    for &import_id in &chunk.imports {
      if in_stack.contains(&import_id) {
        // Back edge — decide which to break based on priority.
        let my_priority = chunk_type_priority(chunk.chunk_type);
        let their_priority =
          chunk_type_priority(self.chunks[import_id.0 as usize].chunk_type);
        if my_priority <= their_priority {
          edges_to_remove.push((chunk_id, import_id));
        } else {
          edges_to_remove.push((import_id, chunk_id));
        }
      } else {
        self.detect_chunk_cycles(
          import_id,
          visited,
          in_stack,
          edges_to_remove,
        );
      }
    }

    in_stack.remove(&chunk_id);
    visited.insert(chunk_id);
  }

  /// Step 4c: Merge shared chunks imported by only one chunk.
  fn merge_single_user_shared_chunks(&mut self) {
    loop {
      let mut merged = false;
      let mut import_counts: FxHashMap<ChunkId, usize> =
        FxHashMap::default();

      for chunk in &self.chunks {
        for &import_id in &chunk.imports {
          *import_counts.entry(import_id).or_insert(0) += 1;
        }
      }

      let mut to_merge: Vec<(ChunkId, ChunkId)> = Vec::new();
      for chunk in &self.chunks {
        if chunk.chunk_type == ChunkType::Shared
          && chunk.modules.is_empty()
        {
          continue;
        }
        if chunk.chunk_type == ChunkType::Shared {
          let count = import_counts.get(&chunk.id).copied().unwrap_or(0);
          if count == 1 {
            // Find the sole importer.
            let importer = self
              .chunks
              .iter()
              .find(|c| c.imports.contains(&chunk.id))
              .map(|c| c.id);
            if let Some(importer_id) = importer {
              to_merge.push((importer_id, chunk.id));
            }
          }
        }
      }

      for (importer_id, shared_id) in to_merge {
        let shared_modules: Vec<ModuleSpecifier> =
          self.chunks[shared_id.0 as usize].modules.drain(..).collect();
        let shared_imports: Vec<ChunkId> =
          self.chunks[shared_id.0 as usize].imports.drain(..).collect();

        for spec in &shared_modules {
          self.module_to_chunk.insert(spec.clone(), importer_id);
        }

        let importer = &mut self.chunks[importer_id.0 as usize];
        // Prepend shared modules (they're dependencies).
        let mut new_modules = shared_modules;
        new_modules.append(&mut importer.modules);
        importer.modules = new_modules;

        // Adopt imports from the shared chunk.
        for imp in shared_imports {
          if imp != importer_id && !importer.imports.contains(&imp) {
            importer.imports.push(imp);
          }
        }
        importer.imports.retain(|&id| id != shared_id);

        merged = true;
      }

      if !merged {
        break;
      }
    }

    // Remove empty chunks.
    self.chunks.retain(|c| !c.modules.is_empty());
  }
}

fn chunk_type_priority(t: ChunkType) -> u8 {
  match t {
    ChunkType::Entry => 2,
    ChunkType::DynamicImport => 1,
    ChunkType::Shared | ChunkType::Asset => 0,
  }
}

#[cfg(test)]
mod tests {
  use crate::config::EnvironmentId;
  use crate::dependency::Dependency;
  use crate::loader::Loader;
  use crate::module::BundlerModule;
  use crate::module::ModuleType;
  use crate::module::SideEffectFlag;

  use super::*;

  fn spec(s: &str) -> ModuleSpecifier {
    ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
  }

  fn make_module(
    specifier: &ModuleSpecifier,
    deps: Vec<Dependency>,
  ) -> BundlerModule {
    BundlerModule {
      specifier: specifier.clone(),
      original_loader: Loader::Js,
      loader: Loader::Js,
      module_type: ModuleType::Esm,
      dependencies: deps,
      side_effects: SideEffectFlag::Unknown,
      source: String::new(),
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

  fn make_dep(
    target: &ModuleSpecifier,
    kind: ImportKind,
  ) -> Dependency {
    Dependency {
      specifier: target.to_string(),
      resolved: target.clone(),
      kind,
      range: None,
    }
  }

  #[test]
  fn test_single_entry_single_module() {
    let entry = spec("entry.ts");
    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(&entry, vec![]));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    assert_eq!(chunk_graph.len(), 1);
    assert_eq!(chunk_graph.entry_chunks().len(), 1);

    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    assert_eq!(chunk.chunk_type, ChunkType::Entry);
    assert_eq!(chunk.modules, vec![entry]);
  }

  #[test]
  fn test_entry_with_dependency() {
    let entry = spec("entry.ts");
    let dep = spec("dep.ts");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      vec![make_dep(&dep, ImportKind::Import)],
    ));
    graph.add_module(make_module(&dep, vec![]));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    assert_eq!(chunk_graph.len(), 1);

    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    assert_eq!(chunk.modules.len(), 2);
    // dep should come before entry (topological order).
    assert_eq!(chunk.modules[0], dep);
    assert_eq!(chunk.modules[1], entry);
  }

  #[test]
  fn test_dynamic_import_creates_new_chunk() {
    let entry = spec("entry.ts");
    let lazy = spec("lazy.ts");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &entry,
      vec![make_dep(&lazy, ImportKind::DynamicImport)],
    ));
    graph.add_module(make_module(&lazy, vec![]));
    graph.add_entry(entry.clone());

    let chunk_graph = build_chunk_graph(&graph);
    assert_eq!(chunk_graph.len(), 2);

    let entry_chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    assert_eq!(entry_chunk.chunk_type, ChunkType::Entry);
    assert_eq!(entry_chunk.modules, vec![entry]);

    // Find the dynamic import chunk.
    let dyn_chunk = chunk_graph
      .chunks()
      .iter()
      .find(|c| c.chunk_type == ChunkType::DynamicImport)
      .expect("should have dynamic import chunk");
    assert_eq!(dyn_chunk.modules, vec![lazy]);
  }

  #[test]
  fn test_shared_module_creates_shared_chunk() {
    let a = spec("a.ts");
    let b = spec("b.ts");
    let shared = spec("shared.ts");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &a,
      vec![make_dep(&shared, ImportKind::Import)],
    ));
    graph.add_module(make_module(
      &b,
      vec![make_dep(&shared, ImportKind::Import)],
    ));
    graph.add_module(make_module(&shared, vec![]));
    graph.add_entry(a.clone());
    graph.add_entry(b.clone());

    let chunk_graph = build_chunk_graph(&graph);

    // Should have 2 entry chunks + 1 shared chunk (or merged if single-user).
    assert!(chunk_graph.len() >= 2);

    // shared module should exist somewhere in the chunks.
    let shared_chunk_id = chunk_graph.module_chunk(&shared);
    assert!(shared_chunk_id.is_some());
  }

  #[test]
  fn test_circular_dependency() {
    let a = spec("a.ts");
    let b = spec("b.ts");

    let mut graph = BundlerGraph::new(EnvironmentId::new(0));
    graph.add_module(make_module(
      &a,
      vec![make_dep(&b, ImportKind::Import)],
    ));
    graph.add_module(make_module(
      &b,
      vec![make_dep(&a, ImportKind::Import)],
    ));
    graph.add_entry(a.clone());

    let chunk_graph = build_chunk_graph(&graph);
    assert_eq!(chunk_graph.len(), 1);

    let chunk = chunk_graph.chunk(chunk_graph.entry_chunks()[0]);
    assert!(chunk.has_circular);
    assert_eq!(chunk.modules.len(), 2);
  }
}
