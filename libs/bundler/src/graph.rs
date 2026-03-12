// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::ModuleSpecifier;
use rustc_hash::FxHashMap;

use crate::config::EnvironmentId;
use crate::module::BundlerModule;

/// A bundler-specific module graph for a single environment.
///
/// Wraps a `deno_graph::ModuleGraph` (used for module discovery and loading)
/// and adds bundler-specific metadata: scope analysis, HMR info, dependency
/// edges with import kind tracking, etc.
///
/// Each environment gets its own `BundlerGraph` instance.
#[derive(Debug)]
pub struct BundlerGraph {
  /// The environment this graph belongs to.
  pub environment_id: EnvironmentId,
  /// Modules indexed by specifier.
  modules: FxHashMap<ModuleSpecifier, BundlerModule>,
  /// Entry point specifiers for this environment.
  entries: Vec<ModuleSpecifier>,
  /// Dense index for chunk graph operations (specifier → u32).
  specifier_to_index: FxHashMap<ModuleSpecifier, u32>,
  /// Reverse mapping (u32 → specifier).
  index_to_specifier: Vec<ModuleSpecifier>,
}

impl BundlerGraph {
  pub fn new(environment_id: EnvironmentId) -> Self {
    Self {
      environment_id,
      modules: FxHashMap::default(),
      entries: Vec::new(),
      specifier_to_index: FxHashMap::default(),
      index_to_specifier: Vec::new(),
    }
  }

  /// Add a module to the graph.
  pub fn add_module(&mut self, module: BundlerModule) {
    let specifier = module.specifier.clone();
    let index = self.index_to_specifier.len() as u32;
    self.specifier_to_index.insert(specifier.clone(), index);
    self.index_to_specifier.push(specifier.clone());
    self.modules.insert(specifier, module);
  }

  /// Add an entry point.
  pub fn add_entry(&mut self, specifier: ModuleSpecifier) {
    self.entries.push(specifier);
  }

  /// Get a module by specifier.
  pub fn get_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&BundlerModule> {
    self.modules.get(specifier)
  }

  /// Get a mutable reference to a module by specifier.
  pub fn get_module_mut(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<&mut BundlerModule> {
    self.modules.get_mut(specifier)
  }

  /// Get the dense index for a module specifier.
  pub fn module_index(&self, specifier: &ModuleSpecifier) -> Option<u32> {
    self.specifier_to_index.get(specifier).copied()
  }

  /// Get the specifier for a dense index.
  pub fn specifier_at(&self, index: u32) -> Option<&ModuleSpecifier> {
    self.index_to_specifier.get(index as usize)
  }

  /// Get the entry points.
  pub fn entries(&self) -> &[ModuleSpecifier] {
    &self.entries
  }

  /// Iterate over all modules.
  pub fn modules(&self) -> impl Iterator<Item = &BundlerModule> {
    self.modules.values()
  }

  /// Number of modules in the graph.
  pub fn len(&self) -> usize {
    self.modules.len()
  }

  /// Whether the graph is empty.
  pub fn is_empty(&self) -> bool {
    self.modules.is_empty()
  }
}
