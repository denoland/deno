// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_graph::ModuleGraph;
use deno_graph::Resolved;
use serde::Deserialize;
use serde::Serialize;

use super::super::semver::NpmVersion;
use super::super::semver::SpecifierVersionReq;
use super::NpmVersionMatcher;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpmPackageReference {
  pub req: NpmPackageReq,
  pub sub_path: Option<String>,
}

impl NpmPackageReference {
  pub fn from_specifier(
    specifier: &ModuleSpecifier,
  ) -> Result<NpmPackageReference, AnyError> {
    Self::from_str(specifier.as_str())
  }

  pub fn from_str(specifier: &str) -> Result<NpmPackageReference, AnyError> {
    let specifier = match specifier.strip_prefix("npm:") {
      Some(s) => s,
      None => {
        return Err(generic_error(format!(
          "Not an npm specifier: {}",
          specifier
        )));
      }
    };
    let parts = specifier.split('/').collect::<Vec<_>>();
    let name_part_len = if specifier.starts_with('@') { 2 } else { 1 };
    if parts.len() < name_part_len {
      return Err(generic_error(format!("Not a valid package: {}", specifier)));
    }
    let name_parts = &parts[0..name_part_len];
    let last_name_part = &name_parts[name_part_len - 1];
    let (name, version_req) = if let Some(at_index) = last_name_part.rfind('@')
    {
      let version = &last_name_part[at_index + 1..];
      let last_name_part = &last_name_part[..at_index];
      let version_req = SpecifierVersionReq::parse(version)
        .with_context(|| "Invalid version requirement.")?;
      let name = if name_part_len == 1 {
        last_name_part.to_string()
      } else {
        format!("{}/{}", name_parts[0], last_name_part)
      };
      (name, Some(version_req))
    } else {
      (name_parts.join("/"), None)
    };
    let sub_path = if parts.len() == name_parts.len() {
      None
    } else {
      Some(parts[name_part_len..].join("/"))
    };

    if let Some(sub_path) = &sub_path {
      if let Some(at_index) = sub_path.rfind('@') {
        let (new_sub_path, version) = sub_path.split_at(at_index);
        let msg = format!(
          "Invalid package specifier 'npm:{}/{}'. Did you mean to write 'npm:{}{}/{}'?",
          name, sub_path, name, version, new_sub_path
        );
        return Err(generic_error(msg));
      }
    }

    Ok(NpmPackageReference {
      req: NpmPackageReq { name, version_req },
      sub_path,
    })
  }
}

impl std::fmt::Display for NpmPackageReference {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(sub_path) = &self.sub_path {
      write!(f, "npm:{}/{}", self.req, sub_path)
    } else {
      write!(f, "npm:{}", self.req)
    }
  }
}

#[derive(
  Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageReq {
  pub name: String,
  pub version_req: Option<SpecifierVersionReq>,
}

impl std::fmt::Display for NpmPackageReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.version_req {
      Some(req) => write!(f, "{}@{}", self.name, req),
      None => write!(f, "{}", self.name),
    }
  }
}

impl NpmPackageReq {
  pub fn from_str(text: &str) -> Result<Self, AnyError> {
    // probably should do something more targetted in the future
    let reference = NpmPackageReference::from_str(&format!("npm:{}", text))?;
    Ok(reference.req)
  }
}

impl NpmVersionMatcher for NpmPackageReq {
  fn tag(&self) -> Option<&str> {
    match &self.version_req {
      Some(version_req) => version_req.tag(),
      None => Some("latest"),
    }
  }

  fn matches(&self, version: &NpmVersion) -> bool {
    match self.version_req.as_ref() {
      Some(req) => {
        assert_eq!(self.tag(), None);
        match req.range() {
          Some(range) => range.satisfies(version),
          None => false,
        }
      }
      None => version.pre.is_empty(),
    }
  }

  fn version_text(&self) -> String {
    self
      .version_req
      .as_ref()
      .map(|v| format!("{}", v))
      .unwrap_or_else(|| "non-prerelease".to_string())
  }
}

enum NpmSpecifierTreeNode {
  Parent(NpmSpecifierTreeParentNode),
  Leaf(NpmSpecifierTreeLeafNode),
}

impl NpmSpecifierTreeNode {
  pub fn mut_to_leaf(&mut self) {
    if let NpmSpecifierTreeNode::Parent(node) = self {
      let node = std::mem::replace(
        node,
        NpmSpecifierTreeParentNode {
          specifier: node.specifier.clone(),
          dependencies: Default::default(),
        },
      );
      *self = NpmSpecifierTreeNode::Leaf(node.into_leaf());
    }
  }
}

struct NpmSpecifierTreeParentNode {
  specifier: ModuleSpecifier,
  dependencies: HashMap<String, NpmSpecifierTreeNode>,
}

impl NpmSpecifierTreeParentNode {
  pub fn into_leaf(self) -> NpmSpecifierTreeLeafNode {
    fn fill_new_leaf(
      deps: HashMap<String, NpmSpecifierTreeNode>,
      new_leaf: &mut NpmSpecifierTreeLeafNode,
    ) {
      for node in deps.into_values() {
        match node {
          NpmSpecifierTreeNode::Parent(node) => {
            fill_new_leaf(node.dependencies, new_leaf)
          }
          NpmSpecifierTreeNode::Leaf(leaf) => {
            for dep in leaf.dependencies {
              // don't insert if the dependency is found within the new leaf
              if !dep.as_str().starts_with(new_leaf.specifier.as_str()) {
                new_leaf.dependencies.insert(dep);
              }
            }
            new_leaf.reqs.extend(leaf.reqs);
          }
        }
      }
    }

    let mut new_leaf = NpmSpecifierTreeLeafNode {
      specifier: self.specifier,
      reqs: Default::default(),
      dependencies: Default::default(),
    };
    fill_new_leaf(self.dependencies, &mut new_leaf);
    new_leaf
  }
}

struct NpmSpecifierTreeLeafNode {
  specifier: ModuleSpecifier,
  reqs: HashSet<NpmPackageReq>,
  dependencies: HashSet<ModuleSpecifier>,
}

#[derive(Default)]
struct NpmSpecifierTree {
  root_nodes: HashMap<ModuleSpecifier, NpmSpecifierTreeNode>,
}

impl NpmSpecifierTree {
  pub fn get_leaf(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> &mut NpmSpecifierTreeLeafNode {
    let root_specifier = {
      let mut specifier = specifier.clone();
      specifier.set_path("");
      specifier
    };
    let root_node = self
      .root_nodes
      .entry(root_specifier.clone())
      .or_insert_with(|| {
        NpmSpecifierTreeNode::Parent(NpmSpecifierTreeParentNode {
          specifier: root_specifier.clone(),
          dependencies: Default::default(),
        })
      });
    let mut current_node = root_node;
    if !matches!(specifier.path(), "" | "/") {
      let mut current_parts = Vec::new();
      for part in specifier.path()[1..].split('/') {
        current_parts.push(part);
        match current_node {
          NpmSpecifierTreeNode::Leaf(leaf) => return leaf,
          NpmSpecifierTreeNode::Parent(node) => {
            current_node = node
              .dependencies
              .entry(part.to_string())
              .or_insert_with(|| {
                NpmSpecifierTreeNode::Parent(NpmSpecifierTreeParentNode {
                  specifier: {
                    let mut specifier = specifier.clone();
                    specifier.set_path(&current_parts.join("/"));
                    specifier
                  },
                  dependencies: Default::default(),
                })
              });
          }
        }
      }
    }
    current_node.mut_to_leaf();
    match current_node {
      NpmSpecifierTreeNode::Leaf(leaf) => leaf,
      _ => unreachable!(),
    }
  }
}

/// Resolves the npm package requirements from the graph. The order returned
/// is the order they should be resolved in.
pub fn resolve_npm_package_reqs(graph: &ModuleGraph) -> Vec<NpmPackageReq> {
  fn get_parent_path_specifier(specifier: &ModuleSpecifier) -> ModuleSpecifier {
    let mut parent_specifier = specifier.clone();
    parent_specifier.set_query(None);
    parent_specifier.set_fragment(None);
    // remove the last path part, but keep the trailing slash
    let mut path_parts = parent_specifier.path().split('/').collect::<Vec<_>>();
    if path_parts[path_parts.len() - 1].is_empty() {
      path_parts.pop();
    }
    let path_parts_len = path_parts.len(); // make borrow checker happy for some reason
    if path_parts_len > 0 {
      path_parts[path_parts_len - 1] = "";
    }
    parent_specifier.set_path(&path_parts.join("/"));
    parent_specifier
  }

  fn analyze_module(
    module: &deno_graph::Module,
    graph: &ModuleGraph,
    specifier_graph: &mut NpmSpecifierTree,
    seen: &mut HashSet<ModuleSpecifier>,
  ) {
    if !seen.insert(module.specifier.clone()) {
      return; // already visited
    }

    let parent_specifier = get_parent_path_specifier(&module.specifier);
    let leaf = specifier_graph.get_leaf(&parent_specifier);

    let mut specifiers = Vec::with_capacity(module.dependencies.len() * 2 + 1);
    let maybe_types = module.maybe_types_dependency.as_ref().map(|(_, r)| r);
    if let Some(Resolved::Ok { specifier, .. }) = &maybe_types {
      specifiers.push(specifier);
    }
    for dep in module.dependencies.values() {
      #[allow(clippy::manual_flatten)]
      for resolved in [&dep.maybe_code, &dep.maybe_type] {
        if let Resolved::Ok { specifier, .. } = resolved {
          specifiers.push(specifier);
        }
      }
    }
    // fill this leaf's information
    for specifier in &specifiers {
      if specifier.scheme() == "npm" {
        if let Ok(npm_ref) = NpmPackageReference::from_specifier(&specifier) {
          leaf.reqs.insert(npm_ref.req);
        }
      } else if !specifier.as_str().starts_with(&parent_specifier.as_str()) {
        leaf
          .dependencies
          .insert(get_parent_path_specifier(&specifier));
      }
    }
    drop(leaf);

    // now visit all the dependencies
    for specifier in &specifiers {
      let module = graph.get(specifier).unwrap();
      analyze_module(module, graph, specifier_graph, seen);
    }
  }

  let mut seen = HashSet::new();
  let mut specifier_graph = NpmSpecifierTree::default();
  for (root, _) in graph.roots.iter() {
    if let Some(module) = graph.get(root) {
      analyze_module(module, graph, &mut specifier_graph, &mut seen);
    }
  }

  let mut seen = HashSet::new();
  let mut pending_specifiers = VecDeque::new();
  let mut result = Vec::new();
  for (specifier, _) in &graph.roots {
    match NpmPackageReference::from_specifier(specifier) {
      Ok(npm_ref) => result.push(npm_ref.req),
      Err(_) => pending_specifiers.push_back(specifier.clone()),
    }
  }
  while let Some(specifier) = pending_specifiers.pop_front() {
    let leaf = specifier_graph.get_leaf(&specifier);
    if !seen.insert(leaf.specifier.clone()) {
      continue; // already seen
    }
    let reqs = std::mem::take(&mut leaf.reqs);
    let mut reqs = reqs.into_iter().collect::<Vec<_>>();
    // todo(THIS PR): sort also by version
    // The requirements for each batch should be sorted alphabetically
    // in order to help create determinism.
    reqs.sort_by(|a, b| a.name.cmp(&b.name));
    result.extend(reqs);
    let mut deps = std::mem::take(&mut leaf.dependencies)
      .into_iter()
      .collect::<Vec<_>>();
    deps.sort_by(|a, b| {
      fn order_folder_names_descending(
        path_a: &str,
        path_b: &str,
      ) -> Option<Ordering> {
        let path_a = path_a.trim_end_matches('/');
        let path_b = path_b.trim_end_matches('/');
        match path_a.rfind('/') {
          Some(a_index) => match path_b.rfind('/') {
            Some(b_index) => match path_a[a_index..].cmp(&path_b[b_index..]) {
              Ordering::Equal => None,
              ordering => Some(ordering),
            },
            None => None,
          },
          None => None,
        }
      }

      fn order_specifiers(
        a: &ModuleSpecifier,
        b: &ModuleSpecifier,
      ) -> Ordering {
        match order_folder_names_descending(a.path(), b.path()) {
          Some(ordering) => ordering,
          None => a.as_str().cmp(b.as_str()), // fallback to just comparing the entire url
        }
      }

      if a.scheme() == "file" {
        if b.scheme() == "file" {
          order_specifiers(a, b)
        } else {
          Ordering::Less
        }
      } else if b.scheme() == "file" {
        Ordering::Greater
      } else {
        order_specifiers(a, b)
      }
    });
    for dep in deps {
      pending_specifiers.push_back(dep);
    }
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_npm_package_ref() {
    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@1").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("1").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@^1.2").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("^1.2").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package")
        .err()
        .unwrap()
        .to_string(),
      "Not a valid package: @package"
    );
  }
}
