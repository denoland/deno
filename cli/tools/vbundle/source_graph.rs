// Copyright 2018-2026 the Deno authors. MIT license.

//! Source Module Graph - Layer 1 of the bundler's two-layer architecture.
//!
//! This module implements the source-level module graph that tracks all modules
//! before they are bundled into chunks. It is environment-aware, meaning it can
//! track modules across multiple target environments (server, browser, etc.)
//! and handle cross-environment references.
//!
//! The source graph is the input to the chunking algorithm, which produces
//! the Layer 2 chunk graphs (one per environment).

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::parking_lot::RwLock;

use super::environment::BundleEnvironment;
use super::environment::CrossEnvRef;
use super::types::TransformedModule;

/// A module in the source graph.
#[derive(Debug, Clone)]
pub struct SourceModule {
  /// The module's specifier (URL).
  pub specifier: ModuleSpecifier,
  /// The original source code (before transformation).
  pub source: Arc<str>,
  /// The media type of the original source.
  pub media_type: MediaType,
  /// The transformed module (after plugin processing).
  pub transformed: Option<TransformedModule>,
  /// Static imports (import statements).
  pub imports: Vec<ImportInfo>,
  /// Dynamic imports (import() calls).
  pub dynamic_imports: Vec<ImportInfo>,
  /// Re-exports from this module.
  pub re_exports: Vec<ReExportInfo>,
  /// Whether this module has side effects.
  pub side_effects: SideEffects,
  /// Environment(s) this module belongs to.
  pub environments: HashSet<BundleEnvironment>,
  /// Whether this is an entry point.
  pub is_entry: bool,
}

impl SourceModule {
  /// Create a new source module.
  pub fn new(
    specifier: ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> Self {
    Self {
      specifier,
      source,
      media_type,
      transformed: None,
      imports: Vec::new(),
      dynamic_imports: Vec::new(),
      re_exports: Vec::new(),
      side_effects: SideEffects::Unknown,
      environments: HashSet::new(),
      is_entry: false,
    }
  }

  /// Add this module to an environment.
  pub fn add_environment(&mut self, env: BundleEnvironment) {
    self.environments.insert(env);
  }

  /// Check if this module is used in a specific environment.
  pub fn is_in_environment(&self, env: &BundleEnvironment) -> bool {
    self.environments.contains(env)
  }

  /// Get all import specifiers (both static and dynamic).
  pub fn all_import_specifiers(
    &self,
  ) -> impl Iterator<Item = &ModuleSpecifier> {
    self
      .imports
      .iter()
      .chain(self.dynamic_imports.iter())
      .map(|i| &i.specifier)
  }
}

/// Information about an import.
#[derive(Debug, Clone)]
pub struct ImportInfo {
  /// The resolved specifier of the imported module.
  pub specifier: ModuleSpecifier,
  /// The original import string (before resolution).
  pub original: String,
  /// Named imports (e.g., `{ foo, bar as baz }`).
  pub named: Vec<NamedImport>,
  /// Default import name (e.g., `import Foo`).
  pub default_import: Option<String>,
  /// Namespace import name (e.g., `import * as mod`).
  pub namespace_import: Option<String>,
  /// Whether this is a type-only import.
  pub is_type_only: bool,
  /// The byte range in the source where this import appears.
  pub range: (usize, usize),
}

/// A named import binding.
#[derive(Debug, Clone)]
pub struct NamedImport {
  /// The exported name.
  pub name: String,
  /// The local alias (if renamed).
  pub alias: Option<String>,
  /// Whether this is a type-only import.
  pub is_type_only: bool,
}

/// Information about a re-export.
#[derive(Debug, Clone)]
pub struct ReExportInfo {
  /// The source module being re-exported from.
  pub specifier: ModuleSpecifier,
  /// Named re-exports.
  pub named: Vec<NamedReExport>,
  /// Whether this is a `export * from` re-export.
  pub is_all: bool,
}

/// A named re-export binding.
#[derive(Debug, Clone)]
pub struct NamedReExport {
  /// The name being re-exported.
  pub name: String,
  /// The alias for the re-export (if renamed).
  pub alias: Option<String>,
}

/// Side effects status of a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideEffects {
  /// Unknown, assume has side effects (safe default).
  Unknown,
  /// Explicitly marked as having no side effects.
  None,
  /// Has side effects.
  Some,
}

impl Default for SideEffects {
  fn default() -> Self {
    SideEffects::Unknown
  }
}

/// The source module graph (Layer 1).
///
/// This graph tracks all source modules across all environments.
/// It is built by:
/// 1. Starting from entry points
/// 2. Resolving and loading each import (via plugins if needed)
/// 3. Transforming each module (via plugins)
/// 4. Extracting import/export information
#[derive(Debug)]
pub struct SourceModuleGraph {
  /// All modules in the graph, keyed by specifier.
  modules: HashMap<ModuleSpecifier, SourceModule>,
  /// Entry points for each environment.
  entrypoints: HashMap<BundleEnvironment, Vec<ModuleSpecifier>>,
  /// Cross-environment references.
  cross_env_refs: Vec<CrossEnvRef>,
  /// Modules that failed to load (for error reporting).
  errors: Vec<ModuleError>,
}

/// An error that occurred while loading a module.
#[derive(Debug, Clone)]
pub struct ModuleError {
  /// The specifier that failed.
  pub specifier: ModuleSpecifier,
  /// The referrer that imported it.
  pub referrer: Option<ModuleSpecifier>,
  /// The error message.
  pub message: String,
}

impl SourceModuleGraph {
  /// Create a new empty source graph.
  pub fn new() -> Self {
    Self {
      modules: HashMap::new(),
      entrypoints: HashMap::new(),
      cross_env_refs: Vec::new(),
      errors: Vec::new(),
    }
  }

  /// Add an entry point for an environment.
  pub fn add_entrypoint(
    &mut self,
    env: BundleEnvironment,
    specifier: ModuleSpecifier,
  ) {
    self
      .entrypoints
      .entry(env)
      .or_insert_with(Vec::new)
      .push(specifier);
  }

  /// Add a module to the graph.
  pub fn add_module(&mut self, module: SourceModule) {
    self.modules.insert(module.specifier.clone(), module);
  }

  /// Get a module by specifier.
  pub fn get_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&SourceModule> {
    self.modules.get(specifier)
  }

  /// Get a mutable reference to a module.
  pub fn get_module_mut(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<&mut SourceModule> {
    self.modules.get_mut(specifier)
  }

  /// Check if a module exists in the graph.
  pub fn has_module(&self, specifier: &ModuleSpecifier) -> bool {
    self.modules.contains_key(specifier)
  }

  /// Get all modules in the graph.
  pub fn modules(&self) -> impl Iterator<Item = &SourceModule> {
    self.modules.values()
  }

  /// Get entry points for an environment.
  pub fn entrypoints(
    &self,
    env: &BundleEnvironment,
  ) -> Option<&Vec<ModuleSpecifier>> {
    self.entrypoints.get(env)
  }

  /// Get all environments that have entry points.
  pub fn environments(&self) -> impl Iterator<Item = &BundleEnvironment> {
    self.entrypoints.keys()
  }

  /// Add a cross-environment reference.
  pub fn add_cross_env_ref(&mut self, cross_ref: CrossEnvRef) {
    self.cross_env_refs.push(cross_ref);
  }

  /// Get cross-environment references.
  pub fn cross_env_refs(&self) -> &[CrossEnvRef] {
    &self.cross_env_refs
  }

  /// Add an error for a module that failed to load.
  pub fn add_error(&mut self, error: ModuleError) {
    self.errors.push(error);
  }

  /// Get all errors.
  pub fn errors(&self) -> &[ModuleError] {
    &self.errors
  }

  /// Check if there are any errors.
  pub fn has_errors(&self) -> bool {
    !self.errors.is_empty()
  }

  /// Get the total number of modules.
  pub fn module_count(&self) -> usize {
    self.modules.len()
  }

  /// Update a module with new transformed content.
  ///
  /// This is used by HMR to update modules when they are re-transformed
  /// after a file change.
  pub fn update_module(&mut self, module: SourceModule) {
    self.modules.insert(module.specifier.clone(), module);
  }

  /// Get modules for a specific environment.
  pub fn modules_for_env(
    &self,
    env: &BundleEnvironment,
  ) -> impl Iterator<Item = &SourceModule> {
    self
      .modules
      .values()
      .filter(move |m| m.is_in_environment(env))
  }

  /// Get modules that import a given module (reverse dependencies).
  ///
  /// This is useful for HMR to determine which modules need to be updated
  /// when a dependency changes.
  pub fn get_importers(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    let mut importers = Vec::new();

    for module in self.modules.values() {
      // Check static imports
      for import in &module.imports {
        if &import.specifier == specifier {
          importers.push(module.specifier.clone());
          break;
        }
      }

      // Check dynamic imports (if not already added)
      if !importers.contains(&module.specifier) {
        for import in &module.dynamic_imports {
          if &import.specifier == specifier {
            importers.push(module.specifier.clone());
            break;
          }
        }
      }
    }

    importers
  }

  /// Build a reverse dependency map for all modules.
  ///
  /// Returns a map where each key is a module and the value is a set of
  /// modules that import it.
  pub fn build_reverse_dependency_map(
    &self,
  ) -> HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>> {
    let mut reverse_map: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>> =
      HashMap::new();

    for module in self.modules.values() {
      // Add static imports to reverse map
      for import in &module.imports {
        reverse_map
          .entry(import.specifier.clone())
          .or_default()
          .insert(module.specifier.clone());
      }

      // Add dynamic imports to reverse map
      for import in &module.dynamic_imports {
        reverse_map
          .entry(import.specifier.clone())
          .or_default()
          .insert(module.specifier.clone());
      }
    }

    reverse_map
  }

  /// Perform a topological sort of modules for an environment.
  ///
  /// Returns modules in dependency order (dependencies before dependents).
  pub fn toposort(
    &self,
    env: &BundleEnvironment,
  ) -> Result<Vec<&SourceModule>, TopoSortError> {
    let env_modules: HashSet<_> = self
      .modules
      .values()
      .filter(|m| m.is_in_environment(env))
      .map(|m| &m.specifier)
      .collect();

    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();

    for specifier in &env_modules {
      self.visit_topo(
        specifier,
        &env_modules,
        &mut visited,
        &mut stack,
        &mut result,
      )?;
    }

    Ok(result)
  }

  fn visit_topo<'a>(
    &'a self,
    specifier: &ModuleSpecifier,
    env_modules: &HashSet<&ModuleSpecifier>,
    visited: &mut HashSet<ModuleSpecifier>,
    stack: &mut HashSet<ModuleSpecifier>,
    result: &mut Vec<&'a SourceModule>,
  ) -> Result<(), TopoSortError> {
    if visited.contains(specifier) {
      return Ok(());
    }
    if stack.contains(specifier) {
      return Err(TopoSortError::CyclicDependency(specifier.clone()));
    }

    stack.insert(specifier.clone());

    if let Some(module) = self.modules.get(specifier) {
      // Visit static imports first
      for import in &module.imports {
        if env_modules.contains(&import.specifier) {
          self.visit_topo(
            &import.specifier,
            env_modules,
            visited,
            stack,
            result,
          )?;
        }
      }
      // Then dynamic imports
      for import in &module.dynamic_imports {
        if env_modules.contains(&import.specifier) {
          self.visit_topo(
            &import.specifier,
            env_modules,
            visited,
            stack,
            result,
          )?;
        }
      }
    }

    stack.remove(specifier);
    visited.insert(specifier.clone());

    if let Some(module) = self.modules.get(specifier) {
      result.push(module);
    }

    Ok(())
  }
}

impl Default for SourceModuleGraph {
  fn default() -> Self {
    Self::new()
  }
}

/// Error during topological sort.
#[derive(Debug)]
pub enum TopoSortError {
  /// Cyclic dependency detected.
  CyclicDependency(ModuleSpecifier),
}

impl std::fmt::Display for TopoSortError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TopoSortError::CyclicDependency(spec) => {
        write!(f, "Cyclic dependency detected involving: {}", spec)
      }
    }
  }
}

impl std::error::Error for TopoSortError {}

/// Thread-safe wrapper for SourceModuleGraph.
#[derive(Clone)]
pub struct SharedSourceGraph {
  inner: Arc<RwLock<SourceModuleGraph>>,
}

impl SharedSourceGraph {
  /// Create a new shared source graph.
  pub fn new() -> Self {
    Self {
      inner: Arc::new(RwLock::new(SourceModuleGraph::new())),
    }
  }

  /// Get read access to the graph.
  pub fn read(&self) -> impl std::ops::Deref<Target = SourceModuleGraph> + '_ {
    self.inner.read()
  }

  /// Get write access to the graph.
  pub fn write(
    &self,
  ) -> impl std::ops::DerefMut<Target = SourceModuleGraph> + '_ {
    self.inner.write()
  }
}

impl Default for SharedSourceGraph {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_source_module_environments() {
    let spec = ModuleSpecifier::parse("file:///test.ts").unwrap();
    let mut module = SourceModule::new(spec, "".into(), MediaType::TypeScript);

    assert!(!module.is_in_environment(&BundleEnvironment::Server));

    module.add_environment(BundleEnvironment::Server);
    assert!(module.is_in_environment(&BundleEnvironment::Server));
    assert!(!module.is_in_environment(&BundleEnvironment::Browser));

    module.add_environment(BundleEnvironment::Browser);
    assert!(module.is_in_environment(&BundleEnvironment::Browser));
  }

  #[test]
  fn test_source_graph_entrypoints() {
    let mut graph = SourceModuleGraph::new();
    let spec = ModuleSpecifier::parse("file:///main.ts").unwrap();

    graph.add_entrypoint(BundleEnvironment::Server, spec.clone());

    let entries = graph.entrypoints(&BundleEnvironment::Server);
    assert!(entries.is_some());
    assert_eq!(entries.unwrap().len(), 1);
  }

  #[test]
  fn test_get_importers() {
    let mut graph = SourceModuleGraph::new();

    let main_spec = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let dep_spec = ModuleSpecifier::parse("file:///dep.ts").unwrap();
    let other_spec = ModuleSpecifier::parse("file:///other.ts").unwrap();

    // Create main module that imports dep
    let mut main_module =
      SourceModule::new(main_spec.clone(), "".into(), MediaType::TypeScript);
    main_module.imports.push(ImportInfo {
      specifier: dep_spec.clone(),
      original: "./dep.ts".to_string(),
      named: vec![],
      default_import: None,
      namespace_import: None,
      is_type_only: false,
      range: (0, 0),
    });
    graph.add_module(main_module);

    // Create dep module (no imports)
    let dep_module =
      SourceModule::new(dep_spec.clone(), "".into(), MediaType::TypeScript);
    graph.add_module(dep_module);

    // Create other module (no imports)
    let other_module =
      SourceModule::new(other_spec.clone(), "".into(), MediaType::TypeScript);
    graph.add_module(other_module);

    // Get importers of dep - should be main
    let importers = graph.get_importers(&dep_spec);
    assert_eq!(importers.len(), 1);
    assert_eq!(importers[0], main_spec);

    // Get importers of main - should be empty (no one imports main)
    let importers = graph.get_importers(&main_spec);
    assert!(importers.is_empty());

    // Get importers of other - should be empty
    let importers = graph.get_importers(&other_spec);
    assert!(importers.is_empty());
  }

  #[test]
  fn test_build_reverse_dependency_map() {
    let mut graph = SourceModuleGraph::new();

    let main_spec = ModuleSpecifier::parse("file:///main.ts").unwrap();
    let dep1_spec = ModuleSpecifier::parse("file:///dep1.ts").unwrap();
    let dep2_spec = ModuleSpecifier::parse("file:///dep2.ts").unwrap();
    let shared_spec = ModuleSpecifier::parse("file:///shared.ts").unwrap();

    // main imports dep1 and dep2
    let mut main_module =
      SourceModule::new(main_spec.clone(), "".into(), MediaType::TypeScript);
    main_module.imports.push(ImportInfo {
      specifier: dep1_spec.clone(),
      original: "./dep1.ts".to_string(),
      named: vec![],
      default_import: None,
      namespace_import: None,
      is_type_only: false,
      range: (0, 0),
    });
    main_module.imports.push(ImportInfo {
      specifier: dep2_spec.clone(),
      original: "./dep2.ts".to_string(),
      named: vec![],
      default_import: None,
      namespace_import: None,
      is_type_only: false,
      range: (0, 0),
    });
    graph.add_module(main_module);

    // dep1 imports shared
    let mut dep1_module =
      SourceModule::new(dep1_spec.clone(), "".into(), MediaType::TypeScript);
    dep1_module.imports.push(ImportInfo {
      specifier: shared_spec.clone(),
      original: "./shared.ts".to_string(),
      named: vec![],
      default_import: None,
      namespace_import: None,
      is_type_only: false,
      range: (0, 0),
    });
    graph.add_module(dep1_module);

    // dep2 imports shared (shared has multiple importers)
    let mut dep2_module =
      SourceModule::new(dep2_spec.clone(), "".into(), MediaType::TypeScript);
    dep2_module.imports.push(ImportInfo {
      specifier: shared_spec.clone(),
      original: "./shared.ts".to_string(),
      named: vec![],
      default_import: None,
      namespace_import: None,
      is_type_only: false,
      range: (0, 0),
    });
    graph.add_module(dep2_module);

    // shared has no imports
    let shared_module =
      SourceModule::new(shared_spec.clone(), "".into(), MediaType::TypeScript);
    graph.add_module(shared_module);

    // Build reverse map
    let reverse_map = graph.build_reverse_dependency_map();

    // Check that shared is imported by both dep1 and dep2
    let shared_importers = reverse_map.get(&shared_spec).unwrap();
    assert_eq!(shared_importers.len(), 2);
    assert!(shared_importers.contains(&dep1_spec));
    assert!(shared_importers.contains(&dep2_spec));

    // Check that dep1 is imported by main
    let dep1_importers = reverse_map.get(&dep1_spec).unwrap();
    assert_eq!(dep1_importers.len(), 1);
    assert!(dep1_importers.contains(&main_spec));

    // Check that dep2 is imported by main
    let dep2_importers = reverse_map.get(&dep2_spec).unwrap();
    assert_eq!(dep2_importers.len(), 1);
    assert!(dep2_importers.contains(&main_spec));
  }
}
