// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_graph::ModuleGraph;
use deno_graph::Resolved;

use crate::semver::VersionReq;

use super::NpmPackageReference;
use super::NpmPackageReq;

pub struct GraphNpmInfo {
  /// The order of these package requirements is the order they
  /// should be resolved in.
  pub package_reqs: Vec<NpmPackageReq>,
  /// Gets if the graph had a built-in node specifier (ex. `node:fs`).
  pub has_node_builtin_specifier: bool,
}

/// Resolves npm specific information from the graph.
///
/// This function will analyze the module graph for parent-most folder
/// specifiers of all modules, then group npm specifiers together as found in
/// those descendant modules and return them in the order found spreading out
/// from the root of the graph.
///
/// For example, given the following module graph:
///
///   file:///dev/local_module_a/mod.ts
///   ├── npm:package-a@1
///   ├─┬ https://deno.land/x/module_d/mod.ts
///   │ └─┬ https://deno.land/x/module_d/other.ts
///   │   └── npm:package-a@3
///   ├─┬ file:///dev/local_module_a/other.ts
///   │ └── npm:package-b@2
///   ├─┬ file:///dev/local_module_b/mod.ts
///   │ └── npm:package-b@2
///   └─┬ https://deno.land/x/module_a/mod.ts
///     ├── npm:package-a@4
///     ├── npm:package-c@5
///     ├─┬ https://deno.land/x/module_c/sub_folder/mod.ts
///     │ ├── https://deno.land/x/module_c/mod.ts
///     │ ├─┬ https://deno.land/x/module_d/sub_folder/mod.ts
///     │ │ └── npm:package-other@2
///     │ └── npm:package-c@5
///     └── https://deno.land/x/module_b/mod.ts
///
/// The graph above would be grouped down to the topmost specifier folders like
/// so and npm specifiers under each path would be resolved for that group
/// prioritizing file specifiers and sorting by end folder name alphabetically:
///
///   file:///dev/local_module_a/
///   ├── file:///dev/local_module_b/
///   ├─┬ https://deno.land/x/module_a/
///   │ ├── https://deno.land/x/module_b/
///   │ └─┬ https://deno.land/x/module_c/
///   │   └── https://deno.land/x/module_d/
///   └── https://deno.land/x/module_d/
///
/// Then it would resolve the npm specifiers in each of those groups according
/// to that tree going by tree depth.
pub fn resolve_graph_npm_info(graph: &ModuleGraph) -> GraphNpmInfo {
  fn collect_specifiers<'a>(
    graph: &'a ModuleGraph,
    module: &'a deno_graph::Module,
  ) -> Vec<&'a ModuleSpecifier> {
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

    // flatten any data urls into this list of specifiers
    for i in (0..specifiers.len()).rev() {
      if specifiers[i].scheme() == "data" {
        let data_specifier = specifiers.swap_remove(i);
        if let Some(module) = graph.get(data_specifier) {
          specifiers.extend(collect_specifiers(graph, module));
        }
      }
    }

    specifiers
  }

  fn analyze_module(
    module: &deno_graph::Module,
    graph: &ModuleGraph,
    specifier_graph: &mut SpecifierTree,
    seen: &mut HashSet<ModuleSpecifier>,
    has_node_builtin_specifier: &mut bool,
  ) {
    if !seen.insert(module.specifier.clone()) {
      return; // already visited
    }

    let parent_specifier = get_folder_path_specifier(&module.specifier);
    let leaf = specifier_graph.get_leaf(&parent_specifier);

    let specifiers = collect_specifiers(graph, module);

    // fill this leaf's information
    for specifier in &specifiers {
      if let Ok(npm_ref) = NpmPackageReference::from_specifier(specifier) {
        leaf.reqs.insert(npm_ref.req);
      } else if !specifier.as_str().starts_with(parent_specifier.as_str()) {
        leaf
          .dependencies
          .insert(get_folder_path_specifier(specifier));
      }

      if !*has_node_builtin_specifier && specifier.scheme() == "node" {
        *has_node_builtin_specifier = true;
      }
    }

    // now visit all the dependencies
    for specifier in &specifiers {
      if let Some(module) = graph.get(specifier) {
        analyze_module(
          module,
          graph,
          specifier_graph,
          seen,
          has_node_builtin_specifier,
        );
      }
    }
  }

  let root_specifiers = graph
    .roots
    .iter()
    .map(|url| graph.resolve(url))
    .collect::<Vec<_>>();
  let mut seen = HashSet::new();
  let mut specifier_graph = SpecifierTree::default();
  let mut has_node_builtin_specifier = false;
  for root in &root_specifiers {
    if let Some(module) = graph.get(root) {
      analyze_module(
        module,
        graph,
        &mut specifier_graph,
        &mut seen,
        &mut has_node_builtin_specifier,
      );
    }
  }

  let mut seen = HashSet::new();
  let mut pending_specifiers = VecDeque::new();
  let mut result = Vec::new();

  for specifier in &root_specifiers {
    match NpmPackageReference::from_specifier(specifier) {
      Ok(npm_ref) => result.push(npm_ref.req),
      Err(_) => {
        pending_specifiers.push_back(get_folder_path_specifier(specifier))
      }
    }
  }

  while let Some(specifier) = pending_specifiers.pop_front() {
    let leaf = specifier_graph.get_leaf(&specifier);
    if !seen.insert(leaf.specifier.clone()) {
      continue; // already seen
    }

    let reqs = std::mem::take(&mut leaf.reqs);
    let mut reqs = reqs.into_iter().collect::<Vec<_>>();
    reqs.sort_by(cmp_package_req);
    result.extend(reqs);

    let mut deps = std::mem::take(&mut leaf.dependencies)
      .into_iter()
      .collect::<Vec<_>>();
    deps.sort_by(cmp_folder_specifiers);

    for dep in deps {
      pending_specifiers.push_back(dep);
    }
  }

  GraphNpmInfo {
    has_node_builtin_specifier,
    package_reqs: result,
  }
}

fn get_folder_path_specifier(specifier: &ModuleSpecifier) -> ModuleSpecifier {
  let mut specifier = specifier.clone();
  specifier.set_query(None);
  specifier.set_fragment(None);
  if !specifier.path().ends_with('/') {
    // remove the last path part, but keep the trailing slash
    let mut path_parts = specifier.path().split('/').collect::<Vec<_>>();
    let path_parts_len = path_parts.len(); // make borrow checker happy for some reason
    if path_parts_len > 0 {
      path_parts[path_parts_len - 1] = "";
    }
    specifier.set_path(&path_parts.join("/"));
  }
  specifier
}

#[derive(Debug)]
enum SpecifierTreeNode {
  Parent(SpecifierTreeParentNode),
  Leaf(SpecifierTreeLeafNode),
}

impl SpecifierTreeNode {
  pub fn mut_to_leaf(&mut self) {
    if let SpecifierTreeNode::Parent(node) = self {
      let node = std::mem::replace(
        node,
        SpecifierTreeParentNode {
          specifier: node.specifier.clone(),
          dependencies: Default::default(),
        },
      );
      *self = SpecifierTreeNode::Leaf(node.into_leaf());
    }
  }
}

#[derive(Debug)]
struct SpecifierTreeParentNode {
  specifier: ModuleSpecifier,
  dependencies: HashMap<String, SpecifierTreeNode>,
}

impl SpecifierTreeParentNode {
  pub fn into_leaf(self) -> SpecifierTreeLeafNode {
    fn fill_new_leaf(
      deps: HashMap<String, SpecifierTreeNode>,
      new_leaf: &mut SpecifierTreeLeafNode,
    ) {
      for node in deps.into_values() {
        match node {
          SpecifierTreeNode::Parent(node) => {
            fill_new_leaf(node.dependencies, new_leaf)
          }
          SpecifierTreeNode::Leaf(leaf) => {
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

    let mut new_leaf = SpecifierTreeLeafNode {
      specifier: self.specifier,
      reqs: Default::default(),
      dependencies: Default::default(),
    };
    fill_new_leaf(self.dependencies, &mut new_leaf);
    new_leaf
  }
}

#[derive(Debug)]
struct SpecifierTreeLeafNode {
  specifier: ModuleSpecifier,
  reqs: HashSet<NpmPackageReq>,
  dependencies: HashSet<ModuleSpecifier>,
}

#[derive(Default)]
struct SpecifierTree {
  root_nodes: HashMap<ModuleSpecifier, SpecifierTreeNode>,
}

impl SpecifierTree {
  pub fn get_leaf(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> &mut SpecifierTreeLeafNode {
    let root_specifier = {
      let mut specifier = specifier.clone();
      specifier.set_path("");
      specifier
    };
    let root_node = self
      .root_nodes
      .entry(root_specifier.clone())
      .or_insert_with(|| {
        SpecifierTreeNode::Parent(SpecifierTreeParentNode {
          specifier: root_specifier.clone(),
          dependencies: Default::default(),
        })
      });
    let mut current_node = root_node;
    if !matches!(specifier.path(), "" | "/") {
      let mut current_parts = Vec::new();
      let path = specifier.path();
      for part in path[1..path.len() - 1].split('/') {
        current_parts.push(part);
        match current_node {
          SpecifierTreeNode::Leaf(leaf) => return leaf,
          SpecifierTreeNode::Parent(node) => {
            current_node = node
              .dependencies
              .entry(part.to_string())
              .or_insert_with(|| {
                SpecifierTreeNode::Parent(SpecifierTreeParentNode {
                  specifier: {
                    let mut specifier = root_specifier.clone();
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
      SpecifierTreeNode::Leaf(leaf) => leaf,
      _ => unreachable!(),
    }
  }
}

// prefer file: specifiers, then sort by folder name, then by specifier
fn cmp_folder_specifiers(a: &ModuleSpecifier, b: &ModuleSpecifier) -> Ordering {
  fn order_folder_name(path_a: &str, path_b: &str) -> Option<Ordering> {
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

  fn order_specifiers(a: &ModuleSpecifier, b: &ModuleSpecifier) -> Ordering {
    match order_folder_name(a.path(), b.path()) {
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
}

// Sort the package requirements alphabetically then the version
// requirement in a way that will lead to the least number of
// duplicate packages (so sort None last since it's `*`), but
// mostly to create some determinism around how these are resolved.
fn cmp_package_req(a: &NpmPackageReq, b: &NpmPackageReq) -> Ordering {
  fn cmp_specifier_version_req(a: &VersionReq, b: &VersionReq) -> Ordering {
    match a.tag() {
      Some(a_tag) => match b.tag() {
        Some(b_tag) => b_tag.cmp(a_tag), // sort descending
        None => Ordering::Less,          // prefer a since tag
      },
      None => {
        match b.tag() {
          Some(_) => Ordering::Greater, // prefer b since tag
          None => {
            // At this point, just sort by text descending.
            // We could maybe be a bit smarter here in the future.
            b.to_string().cmp(&a.to_string())
          }
        }
      }
    }
  }

  match a.name.cmp(&b.name) {
    Ordering::Equal => {
      match &b.version_req {
        Some(b_req) => {
          match &a.version_req {
            Some(a_req) => cmp_specifier_version_req(a_req, b_req),
            None => Ordering::Greater, // prefer b, since a is *
          }
        }
        None => Ordering::Less, // prefer a, since b is *
      }
    }
    ordering => ordering,
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn sorting_folder_specifiers() {
    fn cmp(a: &str, b: &str) -> Ordering {
      let a = ModuleSpecifier::parse(a).unwrap();
      let b = ModuleSpecifier::parse(b).unwrap();
      cmp_folder_specifiers(&a, &b)
    }

    // prefer file urls
    assert_eq!(
      cmp("file:///test/", "https://deno.land/x/module/"),
      Ordering::Less
    );
    assert_eq!(
      cmp("https://deno.land/x/module/", "file:///test/"),
      Ordering::Greater
    );

    // sort by folder name
    assert_eq!(
      cmp(
        "https://deno.land/x/module_a/",
        "https://deno.land/x/module_b/"
      ),
      Ordering::Less
    );
    assert_eq!(
      cmp(
        "https://deno.land/x/module_b/",
        "https://deno.land/x/module_a/"
      ),
      Ordering::Greater
    );
    assert_eq!(
      cmp(
        "https://deno.land/x/module_a/",
        "https://deno.land/std/module_b/"
      ),
      Ordering::Less
    );
    assert_eq!(
      cmp(
        "https://deno.land/std/module_b/",
        "https://deno.land/x/module_a/"
      ),
      Ordering::Greater
    );

    // by specifier, since folder names match
    assert_eq!(
      cmp(
        "https://deno.land/std/module_a/",
        "https://deno.land/x/module_a/"
      ),
      Ordering::Less
    );
  }

  #[test]
  fn sorting_package_reqs() {
    fn cmp_req(a: &str, b: &str) -> Ordering {
      let a = NpmPackageReq::from_str(a).unwrap();
      let b = NpmPackageReq::from_str(b).unwrap();
      cmp_package_req(&a, &b)
    }

    // sort by name
    assert_eq!(cmp_req("a", "b@1"), Ordering::Less);
    assert_eq!(cmp_req("b@1", "a"), Ordering::Greater);
    // prefer non-wildcard
    assert_eq!(cmp_req("a", "a@1"), Ordering::Greater);
    assert_eq!(cmp_req("a@1", "a"), Ordering::Less);
    // prefer tag
    assert_eq!(cmp_req("a@tag", "a"), Ordering::Less);
    assert_eq!(cmp_req("a", "a@tag"), Ordering::Greater);
    // sort tag descending
    assert_eq!(cmp_req("a@latest-v1", "a@latest-v2"), Ordering::Greater);
    assert_eq!(cmp_req("a@latest-v2", "a@latest-v1"), Ordering::Less);
    // sort version req descending
    assert_eq!(cmp_req("a@1", "a@2"), Ordering::Greater);
    assert_eq!(cmp_req("a@2", "a@1"), Ordering::Less);
  }

  #[test]
  fn test_get_folder_path_specifier() {
    fn get(a: &str) -> String {
      get_folder_path_specifier(&ModuleSpecifier::parse(a).unwrap()).to_string()
    }

    assert_eq!(get("https://deno.land/"), "https://deno.land/");
    assert_eq!(get("https://deno.land"), "https://deno.land/");
    assert_eq!(get("https://deno.land/test"), "https://deno.land/");
    assert_eq!(get("https://deno.land/test/"), "https://deno.land/test/");
    assert_eq!(
      get("https://deno.land/test/other"),
      "https://deno.land/test/"
    );
    assert_eq!(
      get("https://deno.land/test/other/"),
      "https://deno.land/test/other/"
    );
    assert_eq!(
      get("https://deno.land/test/other/test?test#other"),
      "https://deno.land/test/other/"
    );
  }

  #[tokio::test]
  async fn test_resolve_npm_package_reqs() {
    let mut loader = deno_graph::source::MemoryLoader::new(
      vec![
        (
          "file:///dev/local_module_a/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "file:///dev/local_module_a/mod.ts".to_string(),
            content: concat!(
              "import 'https://deno.land/x/module_d/mod.ts';",
              "import 'file:///dev/local_module_a/other.ts';",
              "import 'file:///dev/local_module_b/mod.ts';",
              "import 'https://deno.land/x/module_a/mod.ts';",
              "import 'npm:package-a@local_module_a';",
              "import 'https://deno.land/x/module_e/';",
            )
            .to_string(),
            maybe_headers: None,
          },
        ),
        (
          "file:///dev/local_module_a/other.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "file:///dev/local_module_a/other.ts".to_string(),
            content: "import 'npm:package-b@local_module_a';".to_string(),
            maybe_headers: None,
          },
        ),
        (
          "file:///dev/local_module_b/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "file:///dev/local_module_b/mod.ts".to_string(),
            content: concat!(
              "export * from 'npm:package-b@local_module_b';",
              "import * as test from 'data:application/typescript,export%20*%20from%20%22npm:package-data%40local_module_b%22;';",
            ).to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_d/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_d/mod.ts".to_string(),
            content: concat!(
              "import './other.ts';",
              "import 'npm:package-a@module_d';",
            )
            .to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_d/other.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_d/other.ts".to_string(),
            content: "import 'npm:package-c@module_d'".to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_a/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_a/mod.ts".to_string(),
            content: concat!(
              "import 'npm:package-a@module_a';",
              "import 'npm:package-b@module_a';",
              "import '../module_c/sub/sub/mod.ts';",
              "import '../module_b/mod.ts';",
            )
            .to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_b/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_b/mod.ts".to_string(),
            content: "import 'npm:package-a@module_b'".to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_c/sub/sub/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_c/sub/sub/mod.ts"
              .to_string(),
            content: concat!(
              "import 'npm:package-a@module_c';",
              "import '../../mod.ts';",
            )
            .to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_c/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_c/mod.ts".to_string(),
            content: concat!(
              "import 'npm:package-b@module_c';",
              "import '../module_d/sub_folder/mod.ts';",
            )
            .to_string(),
            maybe_headers: None,
          },
        ),
        (
          "https://deno.land/x/module_d/sub_folder/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_d/sub_folder/mod.ts"
              .to_string(),
            content: "import 'npm:package-b@module_d';".to_string(),
            maybe_headers: None,
          },
        ),
        (
          // ensure a module at a directory is treated as being at a directory
          "https://deno.land/x/module_e/".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_e/"
              .to_string(),
            content: "import 'npm:package-a@module_e';".to_string(),
            maybe_headers: Some(vec![(
              "content-type".to_string(),
              "application/javascript".to_string(),
            )]),
          },
        ),
        // redirect module
        (
          "https://deno.land/x/module_redirect/mod.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_redirect@0.0.1/mod.ts".to_string(),
            content: concat!(
              "import 'npm:package-a@module_redirect';",
              // try another redirect here
              "import 'https://deno.land/x/module_redirect/other.ts';",
            ).to_string(),
            maybe_headers: None,
          }
        ),
        (
          "https://deno.land/x/module_redirect/other.ts".to_string(),
          deno_graph::source::Source::Module {
            specifier: "https://deno.land/x/module_redirect@0.0.1/other.ts".to_string(),
            content: "import 'npm:package-b@module_redirect';".to_string(),
            maybe_headers: None,
          }
        ),
      ],
      Vec::new(),
    );
    let analyzer = deno_graph::CapturingModuleAnalyzer::default();
    let graph = deno_graph::create_graph(
      vec![
        ModuleSpecifier::parse("file:///dev/local_module_a/mod.ts").unwrap(),
        // test redirect at root
        ModuleSpecifier::parse("https://deno.land/x/module_redirect/mod.ts")
          .unwrap(),
      ],
      &mut loader,
      deno_graph::GraphOptions {
        is_dynamic: false,
        imports: None,
        resolver: None,
        module_analyzer: Some(&analyzer),
        reporter: None,
      },
    )
    .await;
    let reqs = resolve_graph_npm_info(&graph)
      .package_reqs
      .into_iter()
      .map(|r| r.to_string())
      .collect::<Vec<_>>();

    assert_eq!(
      reqs,
      vec![
        "package-a@local_module_a",
        "package-b@local_module_a",
        "package-a@module_redirect",
        "package-b@module_redirect",
        "package-b@local_module_b",
        "package-data@local_module_b",
        "package-a@module_a",
        "package-b@module_a",
        "package-a@module_d",
        "package-b@module_d",
        "package-c@module_d",
        "package-a@module_e",
        "package-a@module_b",
        "package-a@module_c",
        "package-b@module_c",
      ]
    );
  }
}
