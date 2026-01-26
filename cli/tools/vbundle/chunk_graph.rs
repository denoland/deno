// Copyright 2018-2026 the Deno authors. MIT license.

//! Chunk graph for bundled output.
//!
//! This module implements Layer 2 of the bundler architecture: per-environment
//! chunk graphs. Each chunk contains one or more modules and represents a
//! single output file.
//!
//! # Code Splitting Strategy
//!
//! Chunks are created based on:
//! 1. Entry points - each entry point gets its own chunk
//! 2. Dynamic imports - each dynamic import boundary creates a new chunk
//! 3. Shared modules - modules imported by multiple chunks may be extracted
//!
//! # Chunk Types
//!
//! - Entry chunks: Contain entry point modules and their static dependencies
//! - Async chunks: Created from dynamic imports, loaded on demand
//! - Shared chunks: Common dependencies extracted from multiple chunks

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::parking_lot::RwLock;

use super::environment::BundleEnvironment;

/// Unique identifier for a chunk.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkId(String);

impl ChunkId {
  pub fn new(name: impl Into<String>) -> Self {
    Self(name.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for ChunkId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// A chunk of bundled modules.
#[derive(Debug, Clone)]
pub struct Chunk {
  /// Unique identifier for this chunk.
  pub id: ChunkId,

  /// The file name for this chunk (e.g., "main.js", "chunk-abc123.js").
  pub file_name: String,

  /// Whether this is an entry point chunk.
  pub is_entry: bool,

  /// Whether this chunk is loaded dynamically.
  pub is_dynamic: bool,

  /// Modules included in this chunk, in bundling order.
  pub modules: Vec<ModuleSpecifier>,

  /// Static imports from other chunks.
  pub imports: HashSet<ChunkId>,

  /// Dynamic imports to other chunks.
  pub dynamic_imports: HashSet<ChunkId>,

  /// The bundled code for this chunk (set after emission).
  pub code: Option<String>,

  /// The source map for this chunk (set after emission).
  pub source_map: Option<String>,

  /// Exports from this chunk (for entry chunks).
  pub exports: Vec<String>,
}

impl Chunk {
  /// Create a new entry chunk.
  pub fn new_entry(id: ChunkId, entry_specifier: ModuleSpecifier) -> Self {
    let file_name = Self::generate_entry_filename(&entry_specifier);
    Self {
      id,
      file_name,
      is_entry: true,
      is_dynamic: false,
      modules: vec![entry_specifier],
      imports: HashSet::new(),
      dynamic_imports: HashSet::new(),
      code: None,
      source_map: None,
      exports: Vec::new(),
    }
  }

  /// Create a new dynamic (async) chunk.
  pub fn new_dynamic(id: ChunkId, entry_specifier: ModuleSpecifier) -> Self {
    let file_name = Self::generate_dynamic_filename(&id);
    Self {
      id,
      file_name,
      is_entry: false,
      is_dynamic: true,
      modules: vec![entry_specifier],
      imports: HashSet::new(),
      dynamic_imports: HashSet::new(),
      code: None,
      source_map: None,
      exports: Vec::new(),
    }
  }

  /// Create a shared chunk for common dependencies.
  pub fn new_shared(id: ChunkId) -> Self {
    let file_name = Self::generate_shared_filename(&id);
    Self {
      id,
      file_name,
      is_entry: false,
      is_dynamic: false,
      modules: Vec::new(),
      imports: HashSet::new(),
      dynamic_imports: HashSet::new(),
      code: None,
      source_map: None,
      exports: Vec::new(),
    }
  }

  /// Add a module to this chunk.
  pub fn add_module(&mut self, specifier: ModuleSpecifier) {
    if !self.modules.contains(&specifier) {
      self.modules.push(specifier);
    }
  }

  /// Add a static import to another chunk.
  pub fn add_import(&mut self, chunk_id: ChunkId) {
    self.imports.insert(chunk_id);
  }

  /// Add a dynamic import to another chunk.
  pub fn add_dynamic_import(&mut self, chunk_id: ChunkId) {
    self.dynamic_imports.insert(chunk_id);
  }

  /// Generate a filename for an entry chunk based on the entry specifier.
  fn generate_entry_filename(specifier: &ModuleSpecifier) -> String {
    // Extract the base name from the specifier
    let path = specifier.path();
    let base = path
      .rsplit('/')
      .next()
      .unwrap_or("entry")
      .trim_end_matches(".ts")
      .trim_end_matches(".tsx")
      .trim_end_matches(".js")
      .trim_end_matches(".jsx")
      .trim_end_matches(".mts")
      .trim_end_matches(".mjs");
    format!("{}.js", base)
  }

  /// Generate a filename for a dynamic chunk.
  fn generate_dynamic_filename(id: &ChunkId) -> String {
    format!("chunk-{}.js", id.as_str())
  }

  /// Generate a filename for a shared chunk.
  fn generate_shared_filename(id: &ChunkId) -> String {
    format!("shared-{}.js", id.as_str())
  }
}

/// Per-environment chunk graph.
#[derive(Debug)]
pub struct ChunkGraph {
  /// The environment this graph is for.
  pub environment: BundleEnvironment,

  /// All chunks in this graph.
  chunks: HashMap<ChunkId, Chunk>,

  /// Mapping from module specifier to chunk.
  module_to_chunk: HashMap<ModuleSpecifier, ChunkId>,

  /// Counter for generating unique chunk IDs.
  chunk_counter: usize,
}

impl ChunkGraph {
  /// Create a new chunk graph for an environment.
  pub fn new(environment: BundleEnvironment) -> Self {
    Self {
      environment,
      chunks: HashMap::new(),
      module_to_chunk: HashMap::new(),
      chunk_counter: 0,
    }
  }

  /// Generate a unique chunk ID.
  pub fn generate_chunk_id(&mut self, prefix: &str) -> ChunkId {
    let id = ChunkId::new(format!("{}_{}", prefix, self.chunk_counter));
    self.chunk_counter += 1;
    id
  }

  /// Add a chunk to the graph.
  pub fn add_chunk(&mut self, chunk: Chunk) {
    let chunk_id = chunk.id.clone();
    for module in &chunk.modules {
      self
        .module_to_chunk
        .insert(module.clone(), chunk_id.clone());
    }
    self.chunks.insert(chunk_id, chunk);
  }

  /// Get a chunk by ID.
  pub fn get_chunk(&self, id: &ChunkId) -> Option<&Chunk> {
    self.chunks.get(id)
  }

  /// Get a mutable chunk by ID.
  pub fn get_chunk_mut(&mut self, id: &ChunkId) -> Option<&mut Chunk> {
    self.chunks.get_mut(id)
  }

  /// Get the chunk containing a module.
  pub fn get_chunk_for_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&ChunkId> {
    self.module_to_chunk.get(specifier)
  }

  /// Assign a module to a chunk.
  pub fn assign_module_to_chunk(
    &mut self,
    specifier: ModuleSpecifier,
    chunk_id: ChunkId,
  ) {
    self.module_to_chunk.insert(specifier, chunk_id);
  }

  /// Get all chunks.
  pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.chunks.values()
  }

  /// Get all chunks mutably.
  pub fn chunks_mut(&mut self) -> impl Iterator<Item = &mut Chunk> {
    self.chunks.values_mut()
  }

  /// Get entry chunks.
  pub fn entry_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.chunks.values().filter(|c| c.is_entry)
  }

  /// Get dynamic chunks.
  pub fn dynamic_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.chunks.values().filter(|c| c.is_dynamic)
  }

  /// Get the number of chunks.
  pub fn chunk_count(&self) -> usize {
    self.chunks.len()
  }

  /// Get the number of modules.
  pub fn module_count(&self) -> usize {
    self.module_to_chunk.len()
  }
}

/// Thread-safe wrapper around ChunkGraph.
pub struct SharedChunkGraph(Arc<RwLock<ChunkGraph>>);

impl SharedChunkGraph {
  pub fn new(environment: BundleEnvironment) -> Self {
    Self(Arc::new(RwLock::new(ChunkGraph::new(environment))))
  }

  pub fn read(&self) -> deno_core::parking_lot::RwLockReadGuard<ChunkGraph> {
    self.0.read()
  }

  pub fn write(&self) -> deno_core::parking_lot::RwLockWriteGuard<ChunkGraph> {
    self.0.write()
  }
}

impl Clone for SharedChunkGraph {
  fn clone(&self) -> Self {
    Self(Arc::clone(&self.0))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_chunk_creation() {
    let specifier = ModuleSpecifier::parse("file:///app/main.ts").unwrap();
    let chunk = Chunk::new_entry(ChunkId::new("entry_0"), specifier.clone());

    assert!(chunk.is_entry);
    assert!(!chunk.is_dynamic);
    assert_eq!(chunk.file_name, "main.js");
    assert_eq!(chunk.modules.len(), 1);
    assert_eq!(chunk.modules[0], specifier);
  }

  #[test]
  fn test_chunk_graph() {
    let mut graph = ChunkGraph::new(BundleEnvironment::Server);

    let specifier = ModuleSpecifier::parse("file:///app/main.ts").unwrap();
    let chunk_id = graph.generate_chunk_id("entry");
    let chunk = Chunk::new_entry(chunk_id.clone(), specifier.clone());

    graph.add_chunk(chunk);

    assert_eq!(graph.chunk_count(), 1);
    assert_eq!(graph.module_count(), 1);
    assert_eq!(graph.get_chunk_for_module(&specifier), Some(&chunk_id));
  }
}
