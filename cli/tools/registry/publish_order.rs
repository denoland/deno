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
) -> Result<Vec<String>, AnyError> {
  let roots = get_workspace_roots(workspace_config)?;
  let graph = module_graph_builder
    .create_graph(
      deno_graph::GraphKind::All,
      roots.iter().flat_map(|r| r.exports.clone()).collect(),
    )
    .await?;

  let packages = build_pkg_deps(graph, roots);
  let sorted_package_names = sort_packages_by_publish_order(&packages)?;

  Ok(sorted_package_names)
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

struct PackageNameWithDeps {
  name: String,
  deps: HashSet<String>,
}

fn build_pkg_deps(
  graph: deno_graph::ModuleGraph,
  roots: Vec<MemberRoot>,
) -> Vec<PackageNameWithDeps> {
  let mut members = Vec::with_capacity(roots.len());
  let mut seen_modules = HashSet::with_capacity(graph.modules().count());
  for root in &roots {
    let mut member = PackageNameWithDeps {
      name: root.name.clone(),
      deps: HashSet::new(),
    };
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
            member.deps.insert(root.name.clone());
          }
        }
      }
    }
    members.push(member);
  }
  members
}

fn sort_packages_by_publish_order(
  packages: &[PackageNameWithDeps],
) -> Result<Vec<String>, AnyError> {
  let mut graph: HashMap<&String, Vec<&String>> = HashMap::new();
  let mut in_degree = HashMap::new();
  let mut all_nodes = HashSet::new();

  // build the graph, in-degree map, and set of all nodes
  for package in packages {
    all_nodes.insert(&package.name);
    in_degree.entry(&package.name).or_insert(0);
    for dep in &package.deps {
      graph.entry(dep).or_default().push(&package.name);
      *in_degree.entry(&package.name).or_insert(0) += 1;
    }
  }

  // queue for nodes with no incoming edges
  let mut queue = VecDeque::new();
  for (node, &degree) in &in_degree {
    if degree == 0 {
      queue.push_back((*node, vec![*node]));
    }
  }

  // perform the topological sort
  let mut sorted = Vec::new();
  while let Some((node, mut path)) = queue.pop_front() {
    sorted.push(node.to_string());

    if let Some(neighbors) = graph.get(node) {
      for neighbor in neighbors {
        let degree = in_degree.entry(neighbor).or_default();
        *degree -= 1;
        if *degree == 0 {
          let mut new_path = path.clone();
          new_path.push(neighbor);
          queue.push_back((*neighbor, new_path));
        }
      }
    }
  }

  // check if all nodes were visited and identify cycles
  if sorted.len() != all_nodes.len() {
    for package in all_nodes {
      if !sorted.contains(package) {
        let mut cycle = Vec::new();
        for (node, path) in &queue {
          if path.contains(&package) && *node == package {
            cycle = path.iter().map(ToString::to_string).collect();
            break;
          }
        }
        if !cycle.is_empty() {
          bail!("Circular dependency detected: {}", cycle.join(" -> "));
        }
      }
    }

    // bug in the code above
    bail!(
      "Circular dependency detected, but specific path could not be determined"
    );
  }

  Ok(sorted)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_circular() {
    let result = sort_packages_by_publish_order(&[
      PackageNameWithDeps {
        name: "a".to_string(),
        deps: HashSet::from(["b".to_string()]),
      },
      PackageNameWithDeps {
        name: "b".to_string(),
        deps: HashSet::from(["c".to_string()]),
      },
      PackageNameWithDeps {
        name: "c".to_string(),
        deps: HashSet::from(["a".to_string()]),
      },
    ]);
    assert_eq!(
      result.unwrap_err().to_string(),
      "Circular dependency detected: a -> b -> c"
    );
  }
}
