// Copyright 2018-2026 the Deno authors. MIT license.

//! HMR boundary computation.
//!
//! Given a set of changed modules, walk up the importer graph to find modules
//! that accept the update (via `import.meta.hot.accept()`). This is
//! AST-independent — it only uses the module graph and HMR metadata.

use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;

/// Result of computing HMR boundaries.
#[derive(Debug)]
pub enum HmrBoundaryResult {
  /// Granular update is possible.
  Update(HmrUpdate),
  /// No accept boundary found — full reload required.
  FullReload,
}

/// A granular HMR update.
#[derive(Debug)]
pub struct HmrUpdate {
  /// Modules that form the accept boundary (they accept the update).
  pub boundaries: Vec<ModuleSpecifier>,
  /// All modules that need to be invalidated (changed modules + path
  /// from changed to boundary).
  pub invalidated: Vec<ModuleSpecifier>,
  /// The original changed module specifiers.
  pub changed: Vec<ModuleSpecifier>,
}

/// Trait for querying the module graph during HMR boundary computation.
/// This decouples the algorithm from the concrete graph implementation.
pub trait HmrGraph {
  /// Does this module self-accept HMR updates?
  fn self_accepts(&self, specifier: &ModuleSpecifier) -> bool;

  /// Does this module decline HMR updates?
  fn declines(&self, specifier: &ModuleSpecifier) -> bool;

  /// Does this module accept updates from a specific dependency?
  fn accepts_dep(
    &self,
    importer: &ModuleSpecifier,
    dep: &ModuleSpecifier,
  ) -> bool;

  /// Get all importers of a module.
  fn importers(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier>;

  /// Is this module an entry point?
  fn is_entry(&self, specifier: &ModuleSpecifier) -> bool;

  /// Does this module exist in the graph?
  fn has_module(&self, specifier: &ModuleSpecifier) -> bool;
}

/// Compute HMR boundaries for a set of changed modules.
///
/// For each changed module, walks up the importer chain (BFS) to find
/// modules that accept the update. If any changed module has no accepting
/// boundary, returns `FullReload`.
pub fn compute_hmr_boundaries(
  graph: &dyn HmrGraph,
  changed: &[ModuleSpecifier],
) -> HmrBoundaryResult {
  let mut boundaries = Vec::new();
  let mut invalidated = HashSet::new();

  for changed_specifier in changed {
    if !graph.has_module(changed_specifier) {
      return HmrBoundaryResult::FullReload;
    }

    if graph.declines(changed_specifier) {
      return HmrBoundaryResult::FullReload;
    }

    invalidated.insert(changed_specifier.clone());

    // Check if module self-accepts.
    if graph.self_accepts(changed_specifier) {
      boundaries.push(changed_specifier.clone());
      continue;
    }

    // BFS up the importer chain.
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    visited.insert(changed_specifier.clone());

    for importer in graph.importers(changed_specifier) {
      queue.push_back(importer);
    }

    let mut found_boundary = false;

    while let Some(importer) = queue.pop_front() {
      if !visited.insert(importer.clone()) {
        continue;
      }

      invalidated.insert(importer.clone());

      // Does this importer accept the changed dep?
      if graph.accepts_dep(&importer, changed_specifier)
        || graph.self_accepts(&importer)
      {
        boundaries.push(importer.clone());
        found_boundary = true;
        continue;
      }

      // If importer is an entry point with no accept, full reload.
      if graph.is_entry(&importer) {
        return HmrBoundaryResult::FullReload;
      }

      // Enqueue this importer's importers.
      for parent in graph.importers(&importer) {
        queue.push_back(parent);
      }
    }

    if !found_boundary {
      return HmrBoundaryResult::FullReload;
    }
  }

  HmrBoundaryResult::Update(HmrUpdate {
    boundaries,
    invalidated: invalidated.into_iter().collect(),
    changed: changed.to_vec(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Simple test graph for HMR boundary tests.
  struct TestGraph {
    modules: Vec<TestModule>,
    entries: Vec<ModuleSpecifier>,
  }

  struct TestModule {
    specifier: ModuleSpecifier,
    self_accepts: bool,
    declines: bool,
    accepted_deps: Vec<ModuleSpecifier>,
    importers: Vec<ModuleSpecifier>,
  }

  impl TestGraph {
    fn spec(s: &str) -> ModuleSpecifier {
      ModuleSpecifier::parse(&format!("file:///{}", s)).unwrap()
    }
  }

  impl HmrGraph for TestGraph {
    fn self_accepts(&self, specifier: &ModuleSpecifier) -> bool {
      self
        .modules
        .iter()
        .find(|m| &m.specifier == specifier)
        .map_or(false, |m| m.self_accepts)
    }

    fn declines(&self, specifier: &ModuleSpecifier) -> bool {
      self
        .modules
        .iter()
        .find(|m| &m.specifier == specifier)
        .map_or(false, |m| m.declines)
    }

    fn accepts_dep(
      &self,
      importer: &ModuleSpecifier,
      dep: &ModuleSpecifier,
    ) -> bool {
      self
        .modules
        .iter()
        .find(|m| &m.specifier == importer)
        .map_or(false, |m| m.accepted_deps.contains(dep))
    }

    fn importers(
      &self,
      specifier: &ModuleSpecifier,
    ) -> Vec<ModuleSpecifier> {
      self
        .modules
        .iter()
        .find(|m| &m.specifier == specifier)
        .map_or_else(Vec::new, |m| m.importers.clone())
    }

    fn is_entry(&self, specifier: &ModuleSpecifier) -> bool {
      self.entries.contains(specifier)
    }

    fn has_module(&self, specifier: &ModuleSpecifier) -> bool {
      self.modules.iter().any(|m| &m.specifier == specifier)
    }
  }

  #[test]
  fn test_self_accepting_module() {
    let a = TestGraph::spec("a.ts");
    let graph = TestGraph {
      modules: vec![TestModule {
        specifier: a.clone(),
        self_accepts: true,
        declines: false,
        accepted_deps: vec![],
        importers: vec![],
      }],
      entries: vec![a.clone()],
    };

    match compute_hmr_boundaries(&graph, &[a.clone()]) {
      HmrBoundaryResult::Update(update) => {
        assert_eq!(update.boundaries, vec![a.clone()]);
        assert_eq!(update.changed, vec![a]);
      }
      HmrBoundaryResult::FullReload => panic!("expected Update"),
    }
  }

  #[test]
  fn test_declining_module() {
    let a = TestGraph::spec("a.ts");
    let graph = TestGraph {
      modules: vec![TestModule {
        specifier: a.clone(),
        self_accepts: false,
        declines: true,
        accepted_deps: vec![],
        importers: vec![],
      }],
      entries: vec![a.clone()],
    };

    assert!(matches!(
      compute_hmr_boundaries(&graph, &[a]),
      HmrBoundaryResult::FullReload
    ));
  }

  #[test]
  fn test_importer_accepts_dep() {
    let dep = TestGraph::spec("dep.ts");
    let app = TestGraph::spec("app.ts");
    let graph = TestGraph {
      modules: vec![
        TestModule {
          specifier: dep.clone(),
          self_accepts: false,
          declines: false,
          accepted_deps: vec![],
          importers: vec![app.clone()],
        },
        TestModule {
          specifier: app.clone(),
          self_accepts: false,
          declines: false,
          accepted_deps: vec![dep.clone()],
          importers: vec![],
        },
      ],
      entries: vec![app.clone()],
    };

    match compute_hmr_boundaries(&graph, &[dep.clone()]) {
      HmrBoundaryResult::Update(update) => {
        assert_eq!(update.boundaries, vec![app]);
        assert!(update.invalidated.contains(&dep));
      }
      HmrBoundaryResult::FullReload => panic!("expected Update"),
    }
  }

  #[test]
  fn test_no_boundary_full_reload() {
    let dep = TestGraph::spec("dep.ts");
    let entry = TestGraph::spec("entry.ts");
    let graph = TestGraph {
      modules: vec![
        TestModule {
          specifier: dep.clone(),
          self_accepts: false,
          declines: false,
          accepted_deps: vec![],
          importers: vec![entry.clone()],
        },
        TestModule {
          specifier: entry.clone(),
          self_accepts: false,
          declines: false,
          accepted_deps: vec![],
          importers: vec![],
        },
      ],
      entries: vec![entry],
    };

    assert!(matches!(
      compute_hmr_boundaries(&graph, &[dep]),
      HmrBoundaryResult::FullReload
    ));
  }
}
