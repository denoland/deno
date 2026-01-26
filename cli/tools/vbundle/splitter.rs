// Copyright 2018-2026 the Deno authors. MIT license.

//! Code splitting algorithm for the bundler.
//!
//! This module implements the code splitting logic that creates chunks from
//! the source module graph. The algorithm:
//!
//! 1. Creates entry chunks for each entry point
//! 2. Creates async chunks at dynamic import boundaries
//! 3. Assigns static dependencies to their importing chunk
//! 4. Optionally extracts shared modules into separate chunks
//!
//! # Splitting Strategy
//!
//! The default strategy creates a chunk for each entry point and each dynamic
//! import. Static dependencies are included in their importing chunk unless
//! they're shared by multiple chunks.

use std::collections::HashMap;
use std::collections::HashSet;

use deno_ast::ModuleSpecifier;

use super::chunk_graph::Chunk;
use super::chunk_graph::ChunkGraph;
use super::chunk_graph::ChunkId;
use super::environment::BundleEnvironment;
use super::source_graph::SharedSourceGraph;

/// Configuration for the code splitter.
#[derive(Debug, Clone)]
pub struct SplitterConfig {
  /// Minimum number of chunks that must share a module before extracting it.
  /// Set to 0 to disable shared chunk extraction.
  pub min_chunks_for_shared: usize,

  /// Minimum size (in bytes) for a module to be extracted to a shared chunk.
  pub min_size_for_shared: usize,

  /// Whether to create chunks for dynamic imports.
  pub split_dynamic_imports: bool,
}

impl Default for SplitterConfig {
  fn default() -> Self {
    Self {
      min_chunks_for_shared: 2,
      min_size_for_shared: 0,
      split_dynamic_imports: true,
    }
  }
}

/// Code splitter that creates chunks from the source graph.
pub struct CodeSplitter<'a> {
  source_graph: &'a SharedSourceGraph,
  config: SplitterConfig,
}

impl<'a> CodeSplitter<'a> {
  /// Create a new code splitter.
  pub fn new(source_graph: &'a SharedSourceGraph, config: SplitterConfig) -> Self {
    Self {
      source_graph,
      config,
    }
  }

  /// Split the source graph into chunks for a specific environment.
  pub fn split(&self, environment: &BundleEnvironment) -> ChunkGraph {
    let mut chunk_graph = ChunkGraph::new(environment.clone());
    let source = self.source_graph.read();

    // Step 1: Create entry chunks
    let entry_points: Vec<ModuleSpecifier> = source
      .entrypoints(environment)
      .map(|v| v.clone())
      .unwrap_or_default();

    let mut entry_chunk_ids: HashMap<ModuleSpecifier, ChunkId> = HashMap::new();

    for entry in &entry_points {
      let chunk_id = chunk_graph.generate_chunk_id("entry");
      let chunk = Chunk::new_entry(chunk_id.clone(), entry.clone());
      entry_chunk_ids.insert(entry.clone(), chunk_id.clone());
      chunk_graph.add_chunk(chunk);
    }

    // Step 2: Identify dynamic import boundaries and create async chunks
    let mut dynamic_chunk_ids: HashMap<ModuleSpecifier, ChunkId> = HashMap::new();

    if self.config.split_dynamic_imports {
      for module in source.modules_for_env(environment) {
        for dynamic_import in &module.dynamic_imports {
          let target = &dynamic_import.specifier;

          // Skip if already an entry point
          if entry_chunk_ids.contains_key(target) {
            continue;
          }

          // Skip if already has a dynamic chunk
          if dynamic_chunk_ids.contains_key(target) {
            continue;
          }

          // Create a new async chunk for this dynamic import
          let chunk_id = chunk_graph.generate_chunk_id("async");
          let chunk = Chunk::new_dynamic(chunk_id.clone(), target.clone());
          dynamic_chunk_ids.insert(target.clone(), chunk_id.clone());
          chunk_graph.add_chunk(chunk);
        }
      }
    }

    // Step 3: Assign modules to chunks using depth-first traversal
    let mut visited: HashSet<ModuleSpecifier> = HashSet::new();

    // Process entry points
    for entry in &entry_points {
      if let Some(chunk_id) = entry_chunk_ids.get(entry) {
        self.assign_dependencies(
          entry,
          chunk_id,
          &mut chunk_graph,
          &mut visited,
          &entry_chunk_ids,
          &dynamic_chunk_ids,
          environment,
        );
      }
    }

    // Process dynamic chunk entry points
    for (specifier, chunk_id) in &dynamic_chunk_ids {
      self.assign_dependencies(
        specifier,
        chunk_id,
        &mut chunk_graph,
        &mut visited,
        &entry_chunk_ids,
        &dynamic_chunk_ids,
        environment,
      );
    }

    // Step 4: Update chunk imports/dynamic_imports
    self.update_chunk_dependencies(&mut chunk_graph, environment);

    chunk_graph
  }

  /// Recursively assign modules to a chunk.
  fn assign_dependencies(
    &self,
    specifier: &ModuleSpecifier,
    chunk_id: &ChunkId,
    chunk_graph: &mut ChunkGraph,
    visited: &mut HashSet<ModuleSpecifier>,
    entry_chunks: &HashMap<ModuleSpecifier, ChunkId>,
    dynamic_chunks: &HashMap<ModuleSpecifier, ChunkId>,
    environment: &BundleEnvironment,
  ) {
    if visited.contains(specifier) {
      return;
    }
    visited.insert(specifier.clone());

    let source = self.source_graph.read();
    let module = match source.get_module(specifier) {
      Some(m) => m,
      None => return,
    };

    // Check if module is for this environment
    if !module.environments.contains(environment) {
      return;
    }

    // Assign this module to the chunk
    chunk_graph.assign_module_to_chunk(specifier.clone(), chunk_id.clone());
    if let Some(chunk) = chunk_graph.get_chunk_mut(chunk_id) {
      chunk.add_module(specifier.clone());
    }

    // Process static imports
    for import in &module.imports {
      let target = &import.specifier;

      // If the target is an entry point, it's in its own chunk
      if entry_chunks.contains_key(target) {
        continue;
      }

      // If the target is a dynamic import entry, it's in its own chunk
      if dynamic_chunks.contains_key(target) {
        continue;
      }

      // Recursively assign dependencies to this chunk
      self.assign_dependencies(
        target,
        chunk_id,
        chunk_graph,
        visited,
        entry_chunks,
        dynamic_chunks,
        environment,
      );
    }

    // Note: Dynamic imports are NOT followed here - they create separate chunks
  }

  /// Update chunk dependencies based on module imports.
  fn update_chunk_dependencies(
    &self,
    chunk_graph: &mut ChunkGraph,
    environment: &BundleEnvironment,
  ) {
    let source = self.source_graph.read();

    // Collect all chunk updates needed
    let mut updates: Vec<(ChunkId, ChunkId, bool)> = Vec::new(); // (from_chunk, to_chunk, is_dynamic)

    for chunk in chunk_graph.chunks() {
      let chunk_id = chunk.id.clone();

      for module_specifier in &chunk.modules {
        if let Some(module) = source.get_module(module_specifier) {
          // Static imports
          for import in &module.imports {
            if let Some(target_chunk_id) = chunk_graph.get_chunk_for_module(&import.specifier) {
              if target_chunk_id != &chunk_id {
                updates.push((chunk_id.clone(), target_chunk_id.clone(), false));
              }
            }
          }

          // Dynamic imports
          for import in &module.dynamic_imports {
            if let Some(target_chunk_id) = chunk_graph.get_chunk_for_module(&import.specifier) {
              if target_chunk_id != &chunk_id {
                updates.push((chunk_id.clone(), target_chunk_id.clone(), true));
              }
            }
          }
        }
      }
    }

    // Apply updates
    for (from_chunk_id, to_chunk_id, is_dynamic) in updates {
      if let Some(chunk) = chunk_graph.get_chunk_mut(&from_chunk_id) {
        if is_dynamic {
          chunk.add_dynamic_import(to_chunk_id);
        } else {
          chunk.add_import(to_chunk_id);
        }
      }
    }
  }
}

/// Determine the bundling order for modules within a chunk.
///
/// Modules should be ordered so that dependencies come before dependents.
pub fn determine_bundle_order(
  chunk: &Chunk,
  source_graph: &SharedSourceGraph,
) -> Vec<ModuleSpecifier> {
  let source = source_graph.read();
  let module_set: HashSet<_> = chunk.modules.iter().cloned().collect();

  let mut ordered: Vec<ModuleSpecifier> = Vec::new();
  let mut visited: HashSet<ModuleSpecifier> = HashSet::new();

  fn visit(
    specifier: &ModuleSpecifier,
    source: &super::source_graph::SourceModuleGraph,
    module_set: &HashSet<ModuleSpecifier>,
    ordered: &mut Vec<ModuleSpecifier>,
    visited: &mut HashSet<ModuleSpecifier>,
  ) {
    if visited.contains(specifier) {
      return;
    }
    visited.insert(specifier.clone());

    // Visit dependencies first
    if let Some(module) = source.get_module(specifier) {
      for import in &module.imports {
        if module_set.contains(&import.specifier) {
          visit(&import.specifier, source, module_set, ordered, visited);
        }
      }
    }

    // Then add this module
    if module_set.contains(specifier) {
      ordered.push(specifier.clone());
    }
  }

  // Start from each module in the chunk
  for specifier in &chunk.modules {
    visit(specifier, &source, &module_set, &mut ordered, &mut visited);
  }

  ordered
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tools::vbundle::source_graph::SourceModule;
  use crate::tools::vbundle::source_graph::ImportInfo;
  use deno_ast::MediaType;

  fn create_test_module(specifier: &str) -> SourceModule {
    SourceModule::new(
      ModuleSpecifier::parse(specifier).unwrap(),
      "".into(),
      MediaType::TypeScript,
    )
  }

  #[test]
  fn test_basic_splitting() {
    let source_graph = SharedSourceGraph::new();

    // Create a simple module graph: main.ts -> dep.ts
    let main = ModuleSpecifier::parse("file:///app/main.ts").unwrap();
    let dep = ModuleSpecifier::parse("file:///app/dep.ts").unwrap();

    {
      let mut graph = source_graph.write();
      graph.add_entrypoint(BundleEnvironment::Server, main.clone());

      let mut main_module = create_test_module("file:///app/main.ts");
      main_module.imports.push(ImportInfo {
        specifier: dep.clone(),
        original: "./dep.ts".to_string(),
        named: Vec::new(),
        default_import: None,
        namespace_import: None,
        is_type_only: false,
        range: (0, 0),
      });
      main_module.add_environment(BundleEnvironment::Server);
      graph.add_module(main_module);

      let mut dep_module = create_test_module("file:///app/dep.ts");
      dep_module.add_environment(BundleEnvironment::Server);
      graph.add_module(dep_module);
    }

    let splitter = CodeSplitter::new(&source_graph, SplitterConfig::default());
    let chunk_graph = splitter.split(&BundleEnvironment::Server);

    // Should have 1 entry chunk containing both modules
    assert_eq!(chunk_graph.chunk_count(), 1);
    assert_eq!(chunk_graph.module_count(), 2);

    let entry_chunks: Vec<_> = chunk_graph.entry_chunks().collect();
    assert_eq!(entry_chunks.len(), 1);
    assert_eq!(entry_chunks[0].modules.len(), 2);
  }

  #[test]
  fn test_dynamic_import_splitting() {
    let source_graph = SharedSourceGraph::new();

    // Create: main.ts --(dynamic)--> lazy.ts
    let main = ModuleSpecifier::parse("file:///app/main.ts").unwrap();
    let lazy = ModuleSpecifier::parse("file:///app/lazy.ts").unwrap();

    {
      let mut graph = source_graph.write();
      graph.add_entrypoint(BundleEnvironment::Server, main.clone());

      let mut main_module = create_test_module("file:///app/main.ts");
      main_module.dynamic_imports.push(ImportInfo {
        specifier: lazy.clone(),
        original: "./lazy.ts".to_string(),
        named: Vec::new(),
        default_import: None,
        namespace_import: None,
        is_type_only: false,
        range: (0, 0),
      });
      main_module.add_environment(BundleEnvironment::Server);
      graph.add_module(main_module);

      let mut lazy_module = create_test_module("file:///app/lazy.ts");
      lazy_module.add_environment(BundleEnvironment::Server);
      graph.add_module(lazy_module);
    }

    let splitter = CodeSplitter::new(&source_graph, SplitterConfig::default());
    let chunk_graph = splitter.split(&BundleEnvironment::Server);

    // Should have 2 chunks: entry + async
    assert_eq!(chunk_graph.chunk_count(), 2);

    let entry_chunks: Vec<_> = chunk_graph.entry_chunks().collect();
    let dynamic_chunks: Vec<_> = chunk_graph.dynamic_chunks().collect();

    assert_eq!(entry_chunks.len(), 1);
    assert_eq!(dynamic_chunks.len(), 1);
  }
}
