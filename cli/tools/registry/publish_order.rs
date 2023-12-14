// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_config::WorkspaceConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;

use crate::graph_util::ModuleGraphBuilder;

pub struct PublishOrderGraph {
  packages: HashMap<String, HashSet<String>>,
  in_degree: HashMap<String, usize>,
  reverse_map: HashMap<String, Vec<String>>,
}

impl PublishOrderGraph {
  pub fn new_single(package_name: String) -> Self {
    Self {
      packages: HashMap::from([(package_name.clone(), HashSet::new())]),
      in_degree: HashMap::from([(package_name.clone(), 0)]),
      reverse_map: HashMap::from([(package_name, Vec::new())]),
    }
  }

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
    // this is inefficient, but that's ok because it's simple and will
    // only ever happen when there's an error
    fn identify_cycle<'a>(
      current_name: &'a String,
      mut visited: HashSet<&'a String>,
      packages: &HashMap<String, HashSet<String>>,
    ) -> Option<Vec<String>> {
      if visited.insert(current_name) {
        let deps = packages.get(current_name).unwrap();
        for dep in deps {
          if let Some(mut cycle) =
            identify_cycle(dep, visited.clone(), packages)
          {
            cycle.push(current_name.to_string());
            return Some(cycle);
          }
        }
        None
      } else {
        Some(vec![current_name.to_string()])
      }
    }

    if self.in_degree.is_empty() {
      Ok(())
    } else {
      let mut pkg_names = self.in_degree.keys().collect::<Vec<_>>();
      pkg_names.sort(); // determinism
      let mut cycle =
        identify_cycle(pkg_names[0], HashSet::new(), &self.packages).unwrap();
      cycle.reverse();
      bail!(
        "Circular package dependency detected: {}",
        cycle.join(" -> ")
      );
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

pub async fn build_publish_graph(
  workspace_config: &WorkspaceConfig,
  module_graph_builder: &ModuleGraphBuilder,
) -> Result<PublishOrderGraph, AnyError> {
  let roots = get_workspace_roots(workspace_config)?;
  let graph = module_graph_builder
    .create_graph(
      deno_graph::GraphKind::All,
      roots.iter().flat_map(|r| r.exports.clone()).collect(),
    )
    .await?;
  graph.valid()?;

  let packages = build_pkg_deps(graph, roots);
  Ok(build_graph(packages))
}

#[derive(Debug)]
struct MemberRoots {
  name: String,
  dir_url: ModuleSpecifier,
  exports: Vec<ModuleSpecifier>,
}

fn get_workspace_roots(
  config: &WorkspaceConfig,
) -> Result<Vec<MemberRoots>, AnyError> {
  let mut members = Vec::with_capacity(config.members.len());
  let mut seen_names = HashSet::with_capacity(config.members.len());
  for member in &config.members {
    let exports_config = member
      .config_file
      .to_exports_config()
      .with_context(|| {
        format!(
          "Failed to parse exports at {}",
          member.config_file.specifier
        )
      })?
      .into_map();
    if !seen_names.insert(&member.package_name) {
      bail!(
        "Cannot have two workspace packages with the same name ('{}' at {})",
        member.package_name,
        member.path.display(),
      );
    }
    let mut member_root = MemberRoots {
      name: member.package_name.clone(),
      dir_url: member.config_file.specifier.join("./").unwrap().clone(),
      exports: Vec::with_capacity(exports_config.len()),
    };
    for (_, value) in exports_config {
      let entry_point =
        member.config_file.specifier.join(&value).with_context(|| {
          format!(
            "Failed to join {} with {}",
            member.config_file.specifier, value
          )
        })?;
      member_root.exports.push(entry_point);
    }
    members.push(member_root);
  }
  Ok(members)
}

fn build_pkg_deps(
  graph: deno_graph::ModuleGraph,
  roots: Vec<MemberRoots>,
) -> HashMap<String, HashSet<String>> {
  let mut members = HashMap::with_capacity(roots.len());
  let mut seen_modules = HashSet::with_capacity(graph.modules().count());
  for root in &roots {
    let mut deps = HashSet::new();
    let mut pending = VecDeque::new();
    pending.extend(root.exports.clone());
    while let Some(specifier) = pending.pop_front() {
      let Some(module) = graph.get(&specifier).and_then(|m| m.esm()) else {
        continue;
      };
      let mut dep_specifiers =
        Vec::with_capacity(module.dependencies.len() + 1);
      if let Some(types_dep) = &module.maybe_types_dependency {
        if let Some(specifier) = types_dep.dependency.maybe_specifier() {
          dep_specifiers.push(specifier);
        }
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
        if specifier.as_str().starts_with(root.dir_url.as_str()) {
          if seen_modules.insert(specifier.clone()) {
            pending.push_back(specifier.clone());
          }
        } else {
          let found_root = roots
            .iter()
            .find(|root| specifier.as_str().starts_with(root.dir_url.as_str()));
          if let Some(root) = found_root {
            deps.insert(root.name.clone());
          }
        }
      }
    }
    members.insert(root.name.clone(), deps);
  }
  members
}

fn build_graph(
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
    let mut graph = build_graph(HashMap::from([
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
    let mut graph = build_graph(HashMap::from([
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
    let mut graph = build_graph(HashMap::from([
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
    let mut graph = build_graph(HashMap::from([
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
}
