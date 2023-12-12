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

pub async fn analyze_workspace_publish_order(
  workspace_config: &WorkspaceConfig,
  module_graph_builder: &ModuleGraphBuilder,
) -> Result<Vec<Vec<String>>, AnyError> {
  let roots = get_workspace_roots(workspace_config)?;
  let graph = module_graph_builder
    .create_graph(
      deno_graph::GraphKind::All,
      roots.iter().flat_map(|r| r.exports.clone()).collect(),
    )
    .await?;

  let packages = build_pkg_deps(graph, roots);
  let publish_batches = batch_packages_by_publish_order(&packages)?;

  Ok(publish_batches)
}

struct MemberRoot {
  name: String,
  root: ModuleSpecifier,
  exports: Vec<ModuleSpecifier>,
}

fn get_workspace_roots(
  config: &WorkspaceConfig,
) -> Result<Vec<MemberRoot>, AnyError> {
  let mut members = Vec::with_capacity(config.members.len());
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
    let mut member_root = MemberRoot {
      name: member.package_name.clone(),
      root: member.config_file.specifier.join("../").unwrap().clone(),
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
  roots: Vec<MemberRoot>,
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
        if specifier.scheme() != "file" {
          continue;
        }
        if specifier.as_str().starts_with(root.root.as_str()) {
          if seen_modules.insert(specifier.clone()) {
            pending.push_back(specifier.clone());
          }
        } else {
          let found_root = roots
            .iter()
            .find(|root| specifier.as_str().starts_with(root.root.as_str()));
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

fn batch_packages_by_publish_order(
  packages: &HashMap<String, HashSet<String>>,
) -> Result<Vec<Vec<String>>, AnyError> {
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
        if let Some(mut cycle) = identify_cycle(dep, visited.clone(), packages)
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

  let mut in_degree = HashMap::new();
  let mut reverse_map: HashMap<&String, Vec<&String>> = HashMap::new();

  // build the graph, in-degree map, and set of all nodes
  for (pkg_name, deps) in packages {
    in_degree.insert(pkg_name, deps.len());
    for dep in deps {
      reverse_map.entry(dep).or_default().push(pkg_name);
    }
  }

  // queue for nodes with no incoming edges
  let mut next = Vec::new();
  for (node, &degree) in &in_degree {
    if degree == 0 {
      next.push(*node);
    }
  }

  // perform the topological sort
  let mut sorted = Vec::new();
  while !next.is_empty() {
    let mut current = next.drain(..).cloned().collect::<Vec<_>>();
    current.sort(); // determinism
    for package_name in &current {
      if let Some(deps) = reverse_map.get(package_name) {
        for dep in deps {
          let degree = in_degree.entry(dep).or_default();
          *degree -= 1;
          if *degree == 0 {
            next.push(dep);
          }
        }
      }
    }
    sorted.push(current);
  }

  // Check if all nodes were visited and identify cycles
  if sorted.iter().map(|s| s.len()).sum::<usize>() != packages.len() {
    let sorted_names = sorted.iter().flatten().collect::<HashSet<_>>();
    let mut pkg_names = packages
      .keys()
      .filter(|name| !sorted_names.contains(name))
      .collect::<Vec<_>>();
    pkg_names.sort(); // determinism
    let mut cycle =
      identify_cycle(pkg_names[0], HashSet::new(), packages).unwrap();
    cycle.reverse();
    bail!(
      "Circular package dependency detected: {}",
      cycle.join(" -> ")
    );
  }

  Ok(sorted)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_batch_no_deps() {
    let result = batch_packages_by_publish_order(&HashMap::from([
      ("a".to_string(), HashSet::new()),
      ("b".to_string(), HashSet::new()),
      ("c".to_string(), HashSet::new()),
    ]));
    assert_eq!(
      result.unwrap(),
      vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]],
    );
  }

  #[test]
  fn test_batch_single_dep() {
    let result = batch_packages_by_publish_order(&HashMap::from([
      ("a".to_string(), HashSet::from(["b".to_string()])),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::new()),
    ]));
    assert_eq!(
      result.unwrap(),
      vec![
        vec!["c".to_string()],
        vec!["b".to_string()],
        vec!["a".to_string()]
      ],
    );
  }

  #[test]
  fn test_batch_multiple_dep() {
    let result = batch_packages_by_publish_order(&HashMap::from([
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
      result.unwrap(),
      vec![
        vec!["c".to_string(), "d".to_string(), "f".to_string()],
        vec!["b".to_string(), "e".to_string()],
        vec!["a".to_string()]
      ],
    );
  }

  #[test]
  fn test_batch_circular_dep() {
    let result = batch_packages_by_publish_order(&HashMap::from([
      ("a".to_string(), HashSet::from(["b".to_string()])),
      ("b".to_string(), HashSet::from(["c".to_string()])),
      ("c".to_string(), HashSet::from(["a".to_string()])),
    ]));
    assert_eq!(
      result.unwrap_err().to_string(),
      "Circular dependency detected: a -> b -> c -> a"
    );
  }
}
