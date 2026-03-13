// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::ModuleSpecifier;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::config::EnvironmentId;
use crate::dependency::ImportKind;
use crate::js::module_info::ExportKind;
use crate::js::module_info::ImportedName;
use crate::js::scope::DeclId;
use crate::module::BundlerModule;
use crate::symbol::SymbolId;
use crate::union_find::SymbolUnionFind;

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
  /// Cross-module symbol union-find, built by `resolve_cross_module_bindings()`.
  pub symbol_uf: SymbolUnionFind,
  /// Per-module live declarations. `None` = all live (entry points, etc.).
  /// Built by `compute_used_exports()`.
  pub used_exports: FxHashMap<ModuleSpecifier, Option<FxHashSet<DeclId>>>,
}

impl BundlerGraph {
  pub fn new(environment_id: EnvironmentId) -> Self {
    Self {
      environment_id,
      modules: FxHashMap::default(),
      entries: Vec::new(),
      specifier_to_index: FxHashMap::default(),
      index_to_specifier: Vec::new(),
      symbol_uf: SymbolUnionFind::empty(),
      used_exports: FxHashMap::default(),
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

  /// Resolve a named export from a module to its canonical `SymbolId`,
  /// following re-export chains transitively.
  ///
  /// Per ES spec, `export * from` does NOT re-export `default`.
  pub fn resolve_named_export(
    &self,
    module_spec: &ModuleSpecifier,
    name: &str,
    visited: &mut FxHashSet<ModuleSpecifier>,
  ) -> Option<SymbolId> {
    if !visited.insert(module_spec.clone()) {
      return None; // Cycle detection
    }

    let module = self.get_module(module_spec)?;
    let module_info = module.module_info.as_ref()?;
    let module_idx = self.module_index(module_spec)?;

    // Search for a direct named export.
    for export in &module_info.exports {
      if export.exported_name != name {
        continue;
      }
      match &export.kind {
        ExportKind::ReExport { source } => {
          // Named re-export: follow the chain.
          let dep = module
            .dependencies
            .iter()
            .find(|d| d.specifier == *source)?;
          let lookup_name = export
            .local_name
            .as_deref()
            .unwrap_or(&export.exported_name);
          return self.resolve_named_export(
            &dep.resolved,
            lookup_name,
            visited,
          );
        }
        ExportKind::ReExportAll { .. } => {
          // Star re-exports have exported_name "*", skip.
          continue;
        }
        ExportKind::Local | ExportKind::Default | ExportKind::DefaultExpression => {
          if let Some(decl_id) = export.decl_id {
            // Check if the decl is actually an import (re-export through local import).
            let decl = module_info.scope_analysis.get_decl(decl_id);
            if decl.kind == crate::js::scope::DeclKind::Import {
              // Find the matching ImportBinding to follow the chain.
              if let Some(import) = module_info
                .imports
                .iter()
                .find(|i| i.local_name == decl.name)
              {
                let lookup_name = match &import.imported {
                  ImportedName::Named(n) => n.as_str(),
                  ImportedName::Default => "default",
                  ImportedName::Namespace => return None,
                };
                let dep = module
                  .dependencies
                  .iter()
                  .find(|d| d.specifier == import.source)?;
                return self.resolve_named_export(
                  &dep.resolved,
                  lookup_name,
                  visited,
                );
              }
            }
            return Some(SymbolId::new(module_idx, decl_id));
          }
          return None;
        }
      }
    }

    // No direct match. If name != "default", search `export *` entries.
    if name != "default" {
      for export in &module_info.exports {
        if let ExportKind::ReExportAll { source } = &export.kind {
          let dep = module
            .dependencies
            .iter()
            .find(|d| d.specifier == *source);
          if let Some(dep) = dep {
            if let Some(sym) =
              self.resolve_named_export(&dep.resolved, name, visited)
            {
              return Some(sym);
            }
          }
        }
      }
    }

    None
  }

  /// Resolve an `ExportBinding` to its `SymbolId` within the given module.
  fn resolve_export_to_symbol(
    &self,
    module_spec: &ModuleSpecifier,
    export: &crate::js::module_info::ExportBinding,
  ) -> Option<SymbolId> {
    let module_idx = self.module_index(module_spec)?;
    match &export.kind {
      ExportKind::Local | ExportKind::Default | ExportKind::DefaultExpression => {
        export.decl_id.map(|id| SymbolId::new(module_idx, id))
      }
      ExportKind::ReExport { source } => {
        let module = self.get_module(module_spec)?;
        let dep = module
          .dependencies
          .iter()
          .find(|d| d.specifier == *source)?;
        let lookup_name = export
          .local_name
          .as_deref()
          .unwrap_or(&export.exported_name);
        let mut visited = FxHashSet::default();
        self.resolve_named_export(&dep.resolved, lookup_name, &mut visited)
      }
      ExportKind::ReExportAll { .. } => None,
    }
  }

  /// Build the symbol union-find and resolve cross-module bindings.
  ///
  /// Must be called after `analyze_graph()` has populated `module_info`.
  pub fn resolve_cross_module_bindings(&mut self) {
    let uf = build_symbol_union_find(self);
    self.symbol_uf = uf;
  }

  /// Compute which declarations in each module are used as exports by importers.
  ///
  /// Entry modules, dynamic import targets, and namespace-imported modules
  /// have all declarations marked as used (`None` = all used).
  ///
  /// Must be called after `resolve_cross_module_bindings()`.
  pub fn compute_used_exports(&mut self) {
    let mut live_symbols: FxHashSet<SymbolId> = FxHashSet::default();
    let mut all_live_modules: FxHashSet<ModuleSpecifier> =
      FxHashSet::default();

    // Phase 1: Seed all-live modules.
    let mut all_live_worklist: Vec<ModuleSpecifier> = Vec::new();

    // Entry points are all-live.
    for entry in &self.entries {
      if all_live_modules.insert(entry.clone()) {
        all_live_worklist.push(entry.clone());
      }
    }

    // Dynamic import and require targets are all-live.
    let dynamic_targets: Vec<ModuleSpecifier> = self
      .modules
      .values()
      .flat_map(|m| {
        m.dependencies.iter().filter_map(|dep| {
          if dep.kind == ImportKind::DynamicImport
            || dep.kind == ImportKind::Require
          {
            Some(dep.resolved.clone())
          } else {
            None
          }
        })
      })
      .collect();
    for target in dynamic_targets {
      if all_live_modules.insert(target.clone()) {
        all_live_worklist.push(target);
      }
    }

    // Namespace import targets are all-live.
    let namespace_targets: Vec<ModuleSpecifier> = self
      .modules
      .values()
      .filter_map(|m| {
        let mi = m.module_info.as_ref()?;
        let targets: Vec<_> = mi
          .imports
          .iter()
          .filter(|i| matches!(i.imported, ImportedName::Namespace))
          .filter_map(|i| {
            m.dependencies
              .iter()
              .find(|d| d.specifier == i.source)
              .map(|d| d.resolved.clone())
          })
          .collect();
        Some(targets)
      })
      .flatten()
      .collect();
    for target in namespace_targets {
      if all_live_modules.insert(target.clone()) {
        all_live_worklist.push(target);
      }
    }

    // Propagate all-live through `export *` chains.
    while let Some(spec) = all_live_worklist.pop() {
      let Some(module) = self.get_module(&spec) else {
        continue;
      };
      let Some(mi) = &module.module_info else {
        continue;
      };
      let star_targets: Vec<ModuleSpecifier> = mi
        .exports
        .iter()
        .filter_map(|e| {
          if let ExportKind::ReExportAll { source } = &e.kind {
            module
              .dependencies
              .iter()
              .find(|d| d.specifier == *source)
              .map(|d| d.resolved.clone())
          } else {
            None
          }
        })
        .collect();
      for target in star_targets {
        if all_live_modules.insert(target.clone()) {
          all_live_worklist.push(target);
        }
      }
    }

    // Phase 2: Walk imports for symbol-level liveness.
    let specifiers: Vec<ModuleSpecifier> =
      self.modules.keys().cloned().collect();
    for spec in &specifiers {
      let module = self.get_module(spec).unwrap();
      let Some(mi) = &module.module_info else {
        continue;
      };
      let module_idx = self.module_index(spec).unwrap();

      for import in &mi.imports {
        if matches!(import.imported, ImportedName::Namespace) {
          continue;
        }
        let import_sym = SymbolId::new(module_idx, import.decl_id);
        let canonical = self.symbol_uf.find(import_sym);
        live_symbols.insert(canonical);
      }

      // For all-live modules: resolve all exports to canonical symbols.
      if all_live_modules.contains(spec) {
        let exports: Vec<_> = mi.exports.clone();
        for export in &exports {
          if matches!(
            export.kind,
            ExportKind::ReExportAll { .. }
          ) {
            continue;
          }
          if let Some(sym) = self.resolve_export_to_symbol(spec, export) {
            let canonical = self.symbol_uf.find(sym);
            live_symbols.insert(canonical);
          }
        }
      }
    }

    // Phase 3: Fixed-point re-export propagation.
    loop {
      let mut changed = false;

      for spec in &specifiers {
        if all_live_modules.contains(spec) {
          continue;
        }
        let module = self.get_module(spec).unwrap();
        let Some(mi) = &module.module_info else {
          continue;
        };
        let exports: Vec<_> = mi.exports.clone();
        for export in &exports {
          if let Some(sym) = self.resolve_export_to_symbol(spec, export) {
            let canonical = self.symbol_uf.find(sym);
            if live_symbols.contains(&canonical) && live_symbols.insert(sym) {
              changed = true;
            }
          }
        }
      }

      if !changed {
        break;
      }
    }

    // Phase 4: Project to per-module FxHashSet<DeclId>.
    let mut result: FxHashMap<ModuleSpecifier, Option<FxHashSet<DeclId>>> =
      FxHashMap::default();
    for spec in &specifiers {
      if all_live_modules.contains(spec) {
        result.insert(spec.clone(), None);
      } else {
        result.insert(spec.clone(), Some(FxHashSet::default()));
      }
    }
    for sym in &live_symbols {
      if let Some(spec) = self.specifier_at(sym.module) {
        if all_live_modules.contains(spec) {
          continue;
        }
        let spec = spec.clone();
        if let Some(Some(set)) = result.get_mut(&spec) {
          set.insert(sym.decl);
        }
      }
    }

    self.used_exports = result;
  }
}

/// Build the symbol union-find from the module graph.
///
/// Iterates all imports in all modules, resolves each to its target
/// declaration via `resolve_named_export`, then unions the import's
/// SymbolId with the target's SymbolId.
fn build_symbol_union_find(graph: &BundlerGraph) -> SymbolUnionFind {
  let mut uf = SymbolUnionFind::empty();

  let specifiers: Vec<ModuleSpecifier> =
    graph.modules.keys().cloned().collect();

  for spec in &specifiers {
    let module = graph.get_module(spec).unwrap();
    let Some(mi) = &module.module_info else {
      continue;
    };
    let module_idx = graph.module_index(spec).unwrap();

    for import in &mi.imports {
      if matches!(import.imported, ImportedName::Namespace) {
        continue;
      }

      let dep = module
        .dependencies
        .iter()
        .find(|d| d.specifier == import.source);
      let Some(dep) = dep else {
        continue;
      };

      let lookup_name = match &import.imported {
        ImportedName::Named(n) => n.as_str(),
        ImportedName::Default => "default",
        ImportedName::Namespace => unreachable!(),
      };

      let mut visited = FxHashSet::default();
      let target =
        graph.resolve_named_export(&dep.resolved, lookup_name, &mut visited);

      if let Some(target_sym) = target {
        let import_sym = SymbolId::new(module_idx, import.decl_id);
        uf.union(import_sym, target_sym);
      }
    }
  }

  uf.flatten();
  uf
}
