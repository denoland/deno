// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_config::workspace::JsrPackageConfig;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;

pub struct PublishOrderGraph {
  packages: HashMap<String, HashSet<String>>,
  in_degree: HashMap<String, usize>,
  reverse_map: HashMap<String, Vec<String>>,
}

impl PublishOrderGraph {
  pub fn next(&mut self) -> Vec<String> {
    let mut package_names_with_depth = self
      .in_degree
      .iter()
      .filter_map(|(name, &degree)| if degree == 0 { Some(name) } else { None })
      .map(|item| (item.clone(), self.compute_depth(item, HashSet::new())))
      .collect::<Vec<_>>();

    // sort by depth to in order to prioritize those packages
    package_names_with_depth.sort_by(|a, b| match b.1.cmp(&a.1) {
      std::cmp::Ordering::Equal => a.0.cmp(&b.0),
      other => other,
    });

    let sorted_package_names = package_names_with_depth
      .into_iter()
      .map(|(name, _)| name)
      .collect::<Vec<_>>();
    for name in &sorted_package_names {
      self.in_degree.remove(name);
    }
    sorted_package_names
  }

  pub fn finish_package(&mut self, name: &str) {
    if let Some(package_names) = self.reverse_map.remove(name) {
      for name in package_names {
        *self.in_degree.get_mut(&name).unwrap() -= 1;
      }
    }
  }

  /// There could be pending packages if there's a circular dependency.
  pub fn ensure_no_pending(&self) -> Result<(), AnyError> {
    if self.in_degree.is_empty() {
      Ok(())
    } else {
      ensure_no_cycles(&self.packages)
    }
  }

  fn compute_depth(
    &self,
    package_name: &String,
    mut visited: HashSet<String>,
  ) -> usize {
    if visited.contains(package_name) {
      return 0; // cycle
    }

    visited.insert(package_name.clone());

    let Some(parents) = self.reverse_map.get(package_name) else {
      return 0;
    };
    let max_depth = parents
      .iter()
      .map(|child| self.compute_depth(child, visited.clone()))
      .max()
      .unwrap_or(0);
    1 + max_depth
  }
}

/// Looks for a cycle reachable from `current_name` using a depth first search.
///
/// `visited` is shared across every starting node so the whole graph is
/// explored in a single O(V + E) pass: a node that has been fully explored
/// without participating in a cycle is never revisited. `stack` holds the
/// current DFS path; encountering a node that is already on it closes a cycle,
/// which is returned in dependency order (e.g. `["a", "b", "c", "a"]` for
/// `a -> b -> c -> a`).
///
/// This runs on every publish (via [`ensure_no_cycles`]), including the common
/// no-cycle case, so it avoids the per-edge cloning a naive search would do.
fn identify_cycle(
  current_name: &str,
  visited: &mut HashSet<String>,
  stack: &mut Vec<String>,
  packages: &HashMap<String, HashSet<String>>,
) -> Option<Vec<String>> {
  if !visited.insert(current_name.to_string()) {
    return None;
  }
  stack.push(current_name.to_string());

  // A dep may not be a key in `packages` (e.g. an external, non-workspace
  // dependency); such a node has no outgoing edges and cannot be part of a
  // cycle, so treat it as a leaf.
  if let Some(deps) = packages.get(current_name) {
    // Sort for deterministic cycle reporting.
    let mut deps = deps.iter().collect::<Vec<_>>();
    deps.sort();
    for dep in deps {
      if let Some(start) = stack.iter().position(|name| name == dep) {
        let mut cycle = stack[start..].to_vec();
        cycle.push(dep.to_string());
        return Some(cycle);
      }
      if let Some(cycle) = identify_cycle(dep, visited, stack, packages) {
        return Some(cycle);
      }
    }
  }

  stack.pop();
  None
}

/// Detects a circular package dependency, returning an error describing the
/// cycle if one is found. Run this before any side effects (like publishing
/// authorization) so users get fast feedback.
fn ensure_no_cycles(
  packages: &HashMap<String, HashSet<String>>,
) -> Result<(), AnyError> {
  let mut visited = HashSet::new();
  let mut stack = Vec::new();
  let mut pkg_names = packages.keys().collect::<Vec<_>>();
  pkg_names.sort(); // determinism
  for pkg_name in pkg_names {
    stack.clear();
    if let Some(cycle) =
      identify_cycle(pkg_name, &mut visited, &mut stack, packages)
    {
      bail!(
        "Circular package dependency detected: {}",
        cycle.join(" -> ")
      );
    }
  }
  Ok(())
}

pub fn build_publish_order_graph(
  graph: &ModuleGraph,
  roots: &[JsrPackageConfig],
) -> Result<PublishOrderGraph, AnyError> {
  let packages = build_pkg_deps(graph, roots)?;
  ensure_no_cycles(&packages)?;
  Ok(build_publish_order_graph_from_pkgs_deps(packages))
}

fn build_pkg_deps(
  graph: &deno_graph::ModuleGraph,
  roots: &[JsrPackageConfig],
) -> Result<HashMap<String, HashSet<String>>, AnyError> {
  let mut members = HashMap::with_capacity(roots.len());
  let mut seen_modules = HashSet::with_capacity(graph.modules().count());
  let roots = roots
    .iter()
    .map(|r| {
      (
        ModuleSpecifier::from_directory_path(r.config_file.dir_path()).unwrap(),
        r,
      )
    })
    .collect::<Vec<_>>();
  for (root_dir_url, pkg_config) in &roots {
    let mut deps = HashSet::new();
    let mut pending = VecDeque::new();
    pending.extend(pkg_config.config_file.resolve_export_value_urls()?);
    while let Some(specifier) = pending.pop_front() {
      let Some(module) = graph.get(&specifier).and_then(|m| m.js()) else {
        continue;
      };
      let mut dep_specifiers =
        Vec::with_capacity(module.dependencies.len() + 1);
      if let Some(types_dep) = &module.maybe_types_dependency
        && let Some(specifier) = types_dep.dependency.maybe_specifier()
      {
        dep_specifiers.push(specifier);
      }
      for (_, dep) in &module.dependencies {
        if let Some(specifier) = dep.maybe_code.maybe_specifier() {
          dep_specifiers.push(specifier);
        }
        if let Some(specifier) = dep.maybe_type.maybe_specifier() {
          dep_specifiers.push(specifier);
        }
      }

      for specifier in dep_specifiers {
        let specifier = graph.resolve(specifier);
        if specifier.scheme() != "file" {
          continue;
        }
        if specifier.as_str().starts_with(root_dir_url.as_str()) {
          if seen_modules.insert(specifier.clone()) {
            pending.push_back(specifier.clone());
          }
        } else {
          let found_root = roots.iter().find(|(dir_url, _)| {
            specifier.as_str().starts_with(dir_url.as_str())
          });
          if let Some(root) = found_root {
            deps.insert(root.1.name.clone());
          }
        }
      }
    }
    members.insert(pkg_config.name.clone(), deps);
  }
  Ok(members)
}

fn build_publish_order_graph_from_pkgs_deps(
  packages: HashMap<String, HashSet<String>>,
) -> PublishOrderGraph {
  let mut in_degree = HashMap::new();
  let mut reverse_map: HashMap<String, Vec<String>> = HashMap::new();

  // build the graph, in-degree map, and set of all nodes
  for (pkg_name, deps) in &packages {
    in_degree.insert(pkg_name.clone(), deps.len());
    for dep in deps {
      reverse_map
        .entry(dep.clone())
        .or_default()
        .push(pkg_name.clone());
    }
  }

  PublishOrderGraph {
    packages: packages.clone(),
    in_degree,
    reverse_map,
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_graph_no_deps() {
    let mut graph = build_publish_order_graph_from_pkgs_deps(HashMap::from([
      ("a".to_string(), HashSet::new()),
      ("b".to_string(), HashSet::new()),
      ("c".to_string(), HashSet::new()),
    ]));
    assert_eq!(
      graph.next(),
      vec!["a".to_string(), "b".to_string(), "c".to_string()],
    );
    graph.finish_package("a");
    assert!(graph.next().is_empty());
    graph.finish_package("b");
    assert!(graph.next().is_empty());
    graph.finish_package("c");
    assert!(graph.next().is_empty());
    graph.ensure_no_pending().unwrap();
  }

  #[test]
  fn test_graph_single_dep() {
    let mut graph = build_publish_order_graph_from_pkgs_deps(HashMap::from([
      ("a".to_string(), HashSet::from(["b".to_string()])),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::new()),
    ]));
    assert_eq!(graph.next(), vec!["c".to_string()]);
    graph.finish_package("c");
    assert_eq!(graph.next(), vec!["b".to_string()]);
    graph.finish_package("b");
    assert_eq!(graph.next(), vec!["a".to_string()]);
    graph.finish_package("a");
    assert!(graph.next().is_empty());
    graph.ensure_no_pending().unwrap();
  }

  #[test]
  fn test_graph_multiple_dep() {
    let mut graph = build_publish_order_graph_from_pkgs_deps(HashMap::from([
      (
        "a".to_string(),
        HashSet::from(["b".to_string(), "c".to_string()]),
      ),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::new()),
      ("d".to_string(), HashSet::new()),
      ("e".to_string(), HashSet::from(["f".to_string()])),
      ("f".to_string(), HashSet::new()),
    ]));
    assert_eq!(
      graph.next(),
      vec!["c".to_string(), "f".to_string(), "d".to_string()]
    );
    graph.finish_package("f");
    assert_eq!(graph.next(), vec!["e".to_string()]);
    graph.finish_package("e");
    assert!(graph.next().is_empty());
    graph.finish_package("d");
    assert!(graph.next().is_empty());
    graph.finish_package("c");
    assert_eq!(graph.next(), vec!["b".to_string()]);
    graph.finish_package("b");
    assert_eq!(graph.next(), vec!["a".to_string()]);
    graph.finish_package("a");
    assert!(graph.next().is_empty());
    graph.ensure_no_pending().unwrap();
  }

  #[test]
  fn test_graph_circular_dep() {
    let mut graph = build_publish_order_graph_from_pkgs_deps(HashMap::from([
      ("a".to_string(), HashSet::from(["b".to_string()])),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::from(["a".to_string()])),
    ]));
    assert!(graph.next().is_empty());
    assert_eq!(
      graph.ensure_no_pending().unwrap_err().to_string(),
      "Circular package dependency detected: a -> b -> c -> a"
    );
  }

  #[test]
  fn test_ensure_no_cycles() {
    // no cycle
    ensure_no_cycles(&HashMap::from([
      ("a".to_string(), HashSet::from(["b".to_string()])),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::new()),
    ]))
    .unwrap();

    // direct cycle between two packages
    assert_eq!(
      ensure_no_cycles(&HashMap::from([
        ("a".to_string(), HashSet::from(["b".to_string()])),
        ("b".to_string(), HashSet::from(["a".to_string()])),
      ]))
      .unwrap_err()
      .to_string(),
      "Circular package dependency detected: a -> b -> a"
    );

    // longer cycle, detected before any side effects
    assert_eq!(
      ensure_no_cycles(&HashMap::from([
        ("a".to_string(), HashSet::from(["b".to_string()])),
        ("b".to_string(), HashSet::from(["c".to_string()])),
        ("c".to_string(), HashSet::from(["a".to_string()])),
      ]))
      .unwrap_err()
      .to_string(),
      "Circular package dependency detected: a -> b -> c -> a"
    );

    // cycle reachable through a non-cyclic lead-in node: the reported cycle
    // starts at the node where it actually closes, not the entry point.
    assert_eq!(
      ensure_no_cycles(&HashMap::from([
        ("a".to_string(), HashSet::from(["b".to_string()])),
        ("b".to_string(), HashSet::from(["c".to_string()])),
        ("c".to_string(), HashSet::from(["b".to_string()])),
      ]))
      .unwrap_err()
      .to_string(),
      "Circular package dependency detected: b -> c -> b"
    );

    // an acyclic component is fully explored (and its nodes marked visited)
    // before the cyclic one is reached in the shared single pass.
    assert_eq!(
      ensure_no_cycles(&HashMap::from([
        ("a".to_string(), HashSet::from(["b".to_string()])),
        ("b".to_string(), HashSet::new()),
        ("y".to_string(), HashSet::from(["z".to_string()])),
        ("z".to_string(), HashSet::from(["y".to_string()])),
      ]))
      .unwrap_err()
      .to_string(),
      "Circular package dependency detected: y -> z -> y"
    );
  }
}
