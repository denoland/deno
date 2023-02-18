// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_graph::npm::NpmPackageId;
use deno_graph::npm::NpmPackageReq;
use deno_graph::semver::VersionReq;
use log::debug;
use once_cell::sync::Lazy;

use crate::npm::cache::should_sync_download;
use crate::npm::registry::NpmDependencyEntry;
use crate::npm::registry::NpmDependencyEntryKind;
use crate::npm::registry::NpmPackageInfo;
use crate::npm::registry::NpmPackageVersionInfo;
use crate::npm::resolution::common::resolve_best_package_version_and_info;
use crate::npm::NpmRegistryApi;

use super::common::version_req_satisfies;
use super::snapshot::NpmResolutionSnapshot;
use super::snapshot::SnapshotPackageCopyIndexResolver;
use super::NpmPackageResolvedId;
use super::NpmResolutionPackage;

pub static LATEST_VERSION_REQ: Lazy<VersionReq> =
  Lazy::new(|| VersionReq::parse_from_specifier("latest").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
struct NodeId(u32);

#[derive(Clone)]
enum GraphPathNodeOrRoot {
  Node(Arc<GraphPath>),
  Root(NpmPackageId),
}

/// Path through the graph.
#[derive(Clone)]
struct GraphPath {
  previous_node: Option<GraphPathNodeOrRoot>,
  node_id: Arc<Mutex<NodeId>>,
}

impl GraphPath {
  pub fn for_root(node_id: NodeId, pkg_id: NpmPackageId) -> Arc<Self> {
    Arc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Root(pkg_id)),
      node_id: Arc::new(Mutex::new(node_id)),
    })
  }

  pub fn new(node_id: NodeId) -> Arc<Self> {
    Arc::new(Self {
      previous_node: None,
      node_id: Arc::new(Mutex::new(node_id)),
    })
  }

  pub fn node_id(&self) -> NodeId {
    *self.node_id.lock()
  }

  pub fn change_id(&self, node_id: NodeId) {
    *self.node_id.lock() = node_id;
  }

  pub fn with_id(self: &Arc<GraphPath>, node_id: NodeId) -> Option<Arc<Self>> {
    if self.has_visited(node_id) {
      None
    } else {
      Some(Arc::new(Self {
        previous_node: Some(GraphPathNodeOrRoot::Node(self.clone())),
        node_id: Arc::new(Mutex::new(node_id)),
      }))
    }
  }

  pub fn has_visited(self: &Arc<Self>, node_id: NodeId) -> bool {
    if self.node_id() == node_id {
      return true;
    }
    let mut maybe_next_node = self.previous_node.as_ref();
    while let Some(GraphPathNodeOrRoot::Node(next_node)) = maybe_next_node {
      // stop once we encounter the same id
      if next_node.node_id() == node_id {
        return true;
      }
      maybe_next_node = next_node.previous_node.as_ref();
    }
    false
  }

  pub fn ancestors(&self) -> GraphPathAncestorIterator {
    GraphPathAncestorIterator {
      next: self.previous_node.as_ref(),
    }
  }
}

struct GraphPathAncestorIterator<'a> {
  next: Option<&'a GraphPathNodeOrRoot>,
}

impl<'a> Iterator for GraphPathAncestorIterator<'a> {
  type Item = &'a GraphPathNodeOrRoot;
  fn next(&mut self) -> Option<Self::Item> {
    if let Some(next) = self.next.take() {
      if let GraphPathNodeOrRoot::Node(node) = next {
        self.next = node.previous_node.as_ref();
      }
      Some(next)
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum NodeParent {
  /// This means the node is at the top of the graph and is specified
  /// in Deno code.
  Root(NpmPackageId),
  /// A reference to another node, which is a resolved package.
  Node(NodeId),
}

/// A resolved package in the resolution graph.
#[derive(Debug)]
struct Node {
  // The same parent can exist multiple times, so just maintain a count
  pub parents: HashMap<NodeParent, u16>,
  // Use BTreeMap in order to create determinism when going down
  // the tree.
  pub children: BTreeMap<String, NodeId>,
  /// Whether the node has demonstrated to have no peer dependencies in its
  /// descendants. If this is true then we can skip analyzing this node
  /// again when we encounter it another time in the dependency tree, which
  /// is much faster.
  pub no_peers: bool,
}

impl Node {
  pub fn has_one_parent(&self) -> bool {
    self.parents.len() == 1 && *self.parents.values().next().unwrap() == 1
  }

  pub fn add_parent(&mut self, parent: NodeParent) {
    let value = self.parents.entry(parent).or_default();
    *value += 1;
  }

  pub fn remove_parent(&mut self, parent: &NodeParent) {
    if let Some(value) = self.parents.get_mut(parent) {
      if *value <= 1 {
        self.parents.remove(parent);
      } else {
        *value -= 1;
      }
    }
  }
}

#[derive(Debug, Default)]
struct ResolvedNodeIds {
  from: HashMap<NpmPackageResolvedId, NodeId>,
  to: HashMap<NodeId, NpmPackageResolvedId>,
}

impl ResolvedNodeIds {
  pub fn set(&mut self, node_id: NodeId, resolved_id: NpmPackageResolvedId) {
    if let Some(old_resolved_id) = self.to.insert(node_id, resolved_id.clone())
    {
      if old_resolved_id.peer_dependencies.is_empty() {
        self.from.remove(&old_resolved_id);
      }
    }
    // todo(THIS PR): order here is important... add some unit tests
    if resolved_id.peer_dependencies.is_empty() {
      self.from.insert(resolved_id, node_id);
    }
  }

  pub fn remove(&mut self, node_id: &NodeId) -> Option<NpmPackageResolvedId> {
    if let Some(resolved_id) = self.to.remove(node_id) {
      self.from.remove(&resolved_id);
      Some(resolved_id)
    } else {
      None
    }
  }

  pub fn get_resolved_id(
    &self,
    node_id: &NodeId,
  ) -> Option<&NpmPackageResolvedId> {
    self.to.get(node_id)
  }

  pub fn get_node_id(&self, id: &NpmPackageResolvedId) -> Option<NodeId> {
    if id.peer_dependencies.is_empty() {
      self.from.get(id).copied()
    } else {
      None
    }
  }
}

// Basic principles:
// 1. Graphs nodes with no peer dependencies in their resolved ID have exactly one
//    representation in memory.
// 2. Once a graph node has any peer dependency in its resolved ID, then it will
//    have a unique representation.

#[derive(Debug, Default)]
pub struct Graph {
  // Need a running count and can't derive this because the
  // packages hashmap could be removed from.
  next_package_id: u32,
  /// Each requirement is mapped to a specific name and version.
  package_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  /// Then each name and version is mapped to an exact node id.
  root_packages: HashMap<NpmPackageId, NodeId>,
  packages_by_name: HashMap<String, Vec<NodeId>>,
  packages: HashMap<NodeId, Node>,
  resolved_node_ids: ResolvedNodeIds,
  // This will be set when creating from a snapshot, then
  // inform the final snapshot creation.
  packages_to_copy_index: HashMap<NpmPackageResolvedId, usize>,
  /// Packages that the resolver should resolve first.
  pending_unresolved_packages: Vec<NpmPackageId>,
}

impl Graph {
  pub fn from_snapshot(snapshot: NpmResolutionSnapshot) -> Self {
    fn fill_for_resolve_id(
      graph: &mut Graph,
      resolved_id: &NpmPackageResolvedId,
      resolution: &NpmResolutionPackage,
      packages: &HashMap<NpmPackageResolvedId, NpmResolutionPackage>,
    ) -> NodeId {
      let (created, node_id) = graph.get_or_create_for_id(resolved_id);
      if created {
        for (name, child_id) in &resolution.dependencies {
          let child_node_id = fill_for_resolve_id(
            graph,
            child_id,
            packages.get(child_id).unwrap(),
            packages,
          );
          graph.set_child_parent_node(name, child_node_id, node_id);
        }
      }
      node_id
    }

    let mut graph = Self {
      // Note: It might be more correct to store the copy index
      // from past resolutions with the node somehow, but maybe not.
      packages_to_copy_index: snapshot
        .packages
        .iter()
        .map(|(id, p)| (id.clone(), p.copy_index))
        .collect(),
      package_reqs: snapshot.package_reqs,
      pending_unresolved_packages: snapshot.pending_unresolved_packages,
      ..Default::default()
    };
    for (id, resolved_id) in snapshot.root_packages {
      let resolution = snapshot.packages.get(&resolved_id).unwrap();
      let node_id = fill_for_resolve_id(
        &mut graph,
        &resolved_id,
        resolution,
        &snapshot.packages,
      );
      graph
        .packages
        .get_mut(&node_id)
        .unwrap()
        .add_parent(NodeParent::Root(id.clone()));
      graph.root_packages.insert(id, node_id);
    }
    graph
  }

  pub fn take_pending_unresolved(&mut self) -> Vec<NpmPackageId> {
    std::mem::take(&mut self.pending_unresolved_packages)
  }

  pub fn has_root_package(&self, id: &NpmPackageId) -> bool {
    self.root_packages.contains_key(id)
  }

  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.package_reqs.contains_key(req)
  }

  fn get_or_create_for_id(
    &mut self,
    resolved_id: &NpmPackageResolvedId,
  ) -> (bool, NodeId) {
    if resolved_id.peer_dependencies.is_empty() {
      if let Some(node_id) = self.resolved_node_ids.get_node_id(resolved_id) {
        return (false, node_id);
      }
    }

    let node_id = NodeId(self.next_package_id);
    self.next_package_id += 1;
    let node = Node {
      parents: Default::default(),
      children: Default::default(),
      no_peers: false,
    };

    self
      .packages_by_name
      .entry(resolved_id.id.name.clone())
      .or_default()
      .push(node_id);
    self.packages.insert(node_id, node);
    self.resolved_node_ids.set(node_id, resolved_id.clone());
    (true, node_id)
  }

  fn borrow_node(&mut self, node_id: &NodeId) -> &mut Node {
    self.packages.get_mut(node_id).unwrap()
  }

  fn borrow_node_mut(&mut self, node_id: &NodeId) -> &mut Node {
    self.packages.get_mut(node_id).unwrap()
  }

  fn set_child_parent(
    &mut self,
    specifier: &str,
    child_id: NodeId,
    parent: &NodeParent,
  ) {
    match parent {
      NodeParent::Node(parent_id) => {
        self.set_child_parent_node(specifier, child_id, *parent_id);
      }
      NodeParent::Root(id) => {
        debug_assert_eq!(specifier, ""); // this should be a magic empty string
        let node = self.packages.get_mut(&child_id).unwrap();
        node.add_parent(NodeParent::Root(id.clone()));
        self.root_packages.insert(id.clone(), child_id);
      }
    }
  }

  fn set_child_parent_node(
    &mut self,
    specifier: &str,
    child_id: NodeId,
    parent_id: NodeId,
  ) {
    assert_ne!(child_id, parent_id);
    let parent = self.packages.get_mut(&parent_id).unwrap();
    parent.children.insert(specifier.to_string(), child_id);
    let child = self.packages.get_mut(&child_id).unwrap();
    child.add_parent(NodeParent::Node(parent_id));
  }

  fn remove_child_parent(&mut self, child_id: NodeId, parent: &NodeParent) {
    match parent {
      NodeParent::Node(parent_id) => {
        let node = self.borrow_node_mut(parent_id);
        node.children.retain(|_, child| *child != child_id);
      }
      NodeParent::Root(_) => {
        // ignore removing from the top level information because,
        // if this ever happens it means it's being replaced
      }
    }
    self.borrow_node_mut(&child_id).remove_parent(parent);
  }

  pub async fn into_snapshot(
    self,
    api: &NpmRegistryApi,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::from_map_with_capacity(
        self.packages_to_copy_index,
        self.packages.len(),
      );
    let resolved_packages_by_name = self
      .packages_by_name
      .into_iter()
      .map(|(name, packages)| {
        (
          name,
          packages
            .into_iter()
            .map(|node_id| {
              self
                .resolved_node_ids
                .get_resolved_id(&node_id)
                .unwrap()
                .clone()
            })
            .collect::<Vec<_>>(),
        )
      })
      .collect::<HashMap<_, _>>();

    // Iterate through the packages vector in each packages_by_name in order
    // to set the copy index as this will be deterministic rather than
    // iterating over the hashmap below.
    for packages in resolved_packages_by_name.values() {
      if packages.len() > 1 {
        for resolved_id in packages {
          copy_index_resolver.resolve(resolved_id);
        }
      }
    }

    let mut packages = HashMap::with_capacity(self.packages.len());
    for (node_id, node) in self.packages {
      let resolved_id =
        self.resolved_node_ids.get_resolved_id(&node_id).unwrap();
      let dist = api
        .package_version_info(&resolved_id.id)
        .await?
        .unwrap()
        .dist;
      packages.insert(
        resolved_id.clone(),
        NpmResolutionPackage {
          copy_index: copy_index_resolver.resolve(&resolved_id),
          pkg_id: resolved_id.clone(),
          dist,
          dependencies: node
            .children
            .iter()
            .map(|(key, value)| {
              (
                key.clone(),
                self
                  .resolved_node_ids
                  .get_resolved_id(&value)
                  .unwrap()
                  .clone(),
              )
            })
            .collect(),
        },
      );
    }

    debug_assert!(self.pending_unresolved_packages.is_empty());

    Ok(NpmResolutionSnapshot {
      package_reqs: self.package_reqs,
      root_packages: self
        .root_packages
        .into_iter()
        .map(|(id, node_id)| {
          (
            id,
            self
              .resolved_node_ids
              .get_resolved_id(&node_id)
              .unwrap()
              .clone(),
          )
        })
        .collect(),
      packages_by_name: resolved_packages_by_name,
      packages,
      pending_unresolved_packages: self.pending_unresolved_packages,
    })
  }

  // Debugging methods

  #[cfg(debug_assertions)]
  #[allow(unused)]
  fn output_path(&self, path: &Arc<GraphPath>) {
    eprintln!("-----------");
    self.output_node(path.node_id());
    for path in path.ancestors() {
      match path {
        GraphPathNodeOrRoot::Node(node) => self.output_node(node.node_id()),
        GraphPathNodeOrRoot::Root(pkg_id) => {
          let node_id = self.root_packages.get(pkg_id).unwrap();
          eprintln!(
            "Root: {} ({}: {})",
            pkg_id,
            node_id.0,
            self
              .resolved_node_ids
              .get_resolved_id(node_id)
              .unwrap()
              .as_serialized()
          )
        }
      }
    }
    eprintln!("-----------");
  }

  #[cfg(debug_assertions)]
  #[allow(unused)]
  fn output_node(&self, node_id: NodeId) {
    eprintln!(
      "{:>4}: {}",
      node_id.0,
      self
        .resolved_node_ids
        .get_resolved_id(&node_id)
        .unwrap()
        .as_serialized()
    );
  }
}

#[derive(Default)]
struct DepEntryCache(HashMap<NpmPackageId, Arc<Vec<NpmDependencyEntry>>>);

impl DepEntryCache {
  pub fn store(
    &mut self,
    id: NpmPackageId,
    version_info: &NpmPackageVersionInfo,
  ) -> Result<Arc<Vec<NpmDependencyEntry>>, AnyError> {
    debug_assert!(!self.0.contains_key(&id)); // we should not be re-inserting
    let mut deps = version_info
      .dependencies_as_entries()
      .with_context(|| format!("npm package: {}", id))?;
    // Ensure name alphabetical and then version descending
    // so these are resolved in that order
    deps.sort();
    let deps = Arc::new(deps);
    self.0.insert(id, deps.clone());
    Ok(deps)
  }

  pub fn get(
    &self,
    id: &NpmPackageId,
  ) -> Option<&Arc<Vec<NpmDependencyEntry>>> {
    self.0.get(id)
  }
}

pub struct GraphDependencyResolver<'a> {
  graph: &'a mut Graph,
  api: &'a NpmRegistryApi,
  pending_unresolved_nodes: VecDeque<Arc<GraphPath>>,
  dep_entry_cache: DepEntryCache,
}

impl<'a> GraphDependencyResolver<'a> {
  pub fn new(graph: &'a mut Graph, api: &'a NpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_unresolved_nodes: Default::default(),
      dep_entry_cache: Default::default(),
    }
  }

  pub fn add_root_package(
    &mut self,
    package_id: &NpmPackageId,
    package_info: &NpmPackageInfo,
  ) -> Result<(), AnyError> {
    if self.graph.root_packages.contains_key(package_id) {
      // it already exists, so ignore
      return Ok(());
    }

    // todo(dsherret): using a version requirement here is a temporary hack
    // to reuse code in a large refactor. We should not be using a
    // version requirement here.
    let version_req =
      VersionReq::parse_from_specifier(&format!("{}", package_id.version))
        .unwrap();
    let (pkg_id, node_id) = self.resolve_node_from_info(
      &package_id.name,
      &version_req,
      package_info,
      None,
    )?;
    self.graph.set_child_parent(
      "",
      node_id,
      &NodeParent::Root(pkg_id.id.clone()),
    );
    self
      .pending_unresolved_nodes
      .push_back(GraphPath::for_root(node_id, pkg_id.id));
    Ok(())
  }

  pub fn add_package_req(
    &mut self,
    package_req: &NpmPackageReq,
    package_info: &NpmPackageInfo,
  ) -> Result<(), AnyError> {
    let (pkg_id, node_id) = self.resolve_node_from_info(
      &package_req.name,
      package_req
        .version_req
        .as_ref()
        .unwrap_or(&*LATEST_VERSION_REQ),
      package_info,
      None,
    )?;
    self
      .graph
      .package_reqs
      .insert(package_req.clone(), pkg_id.id.clone());
    self.graph.set_child_parent(
      "",
      node_id,
      &NodeParent::Root(pkg_id.id.clone()),
    );
    self
      .pending_unresolved_nodes
      .push_back(GraphPath::for_root(node_id, pkg_id.id));
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: &NpmPackageInfo,
    visited_versions: &Arc<GraphPath>,
  ) -> Result<NodeId, AnyError> {
    debug_assert_eq!(entry.kind, NpmDependencyEntryKind::Dep);
    let parent_id = visited_versions.node_id();
    let (_, node_id) = self.resolve_node_from_info(
      &entry.name,
      &entry.version_req,
      package_info,
      Some(parent_id),
    )?;
    // Some packages may resolves to themselves as a dependency. If this occurs,
    // just ignore adding these as dependencies because this is likely a mistake
    // in the package.
    if node_id != parent_id {
      self.graph.set_child_parent(
        &entry.bare_specifier,
        node_id,
        &NodeParent::Node(parent_id),
      );
      self.try_add_pending_unresolved_node(visited_versions, node_id);
    }
    Ok(node_id)
  }

  fn try_add_pending_unresolved_node(
    &mut self,
    path: &Arc<GraphPath>,
    node_id: NodeId,
  ) {
    if self.graph.packages.get(&node_id).unwrap().no_peers {
      return; // skip, no need to analyze this again
    }
    let visited_versions = match path.with_id(node_id) {
      Some(visited_versions) => visited_versions,
      None => return, // circular, don't visit this node
    };
    self.pending_unresolved_nodes.push_back(visited_versions);
  }

  fn resolve_node_from_info(
    &mut self,
    pkg_req_name: &str,
    version_req: &VersionReq,
    package_info: &NpmPackageInfo,
    parent_id: Option<NodeId>,
  ) -> Result<(NpmPackageResolvedId, NodeId), AnyError> {
    let version_and_info = resolve_best_package_version_and_info(
      version_req,
      package_info,
      self
        .graph
        .packages_by_name
        .entry(package_info.name.clone())
        .or_default()
        .iter()
        .map(|node_id| {
          &self
            .graph
            .resolved_node_ids
            .get_resolved_id(node_id)
            .unwrap()
            .id
            .version
        }),
    )?;
    let resolved_id = NpmPackageResolvedId {
      id: NpmPackageId {
        name: package_info.name.to_string(),
        version: version_and_info.version.clone(),
      },
      peer_dependencies: Vec::new(),
    };
    // todo(THIS PR): revert to debug
    eprintln!(
      "{} - Resolved {}@{} to {}",
      match parent_id {
        Some(parent_id) => self
          .graph
          .resolved_node_ids
          .get_resolved_id(&parent_id)
          .unwrap()
          .as_serialized(),
        None => "<package-req>".to_string(),
      },
      pkg_req_name,
      version_req.version_text(),
      resolved_id.as_serialized(),
    );
    let (_, node_id) = self.graph.get_or_create_for_id(&resolved_id);

    let has_deps = if let Some(deps) = self.dep_entry_cache.get(&resolved_id.id)
    {
      !deps.is_empty()
    } else {
      let deps = self
        .dep_entry_cache
        .store(resolved_id.id.clone(), &version_and_info.info)?;
      !deps.is_empty()
    };

    if !has_deps {
      // ensure this is set if not, as its an optimization
      let mut node = self.graph.borrow_node_mut(&node_id);
      node.no_peers = true;
    }

    Ok((resolved_id, node_id))
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some(graph_path) = self.pending_unresolved_nodes.pop_front() {
        let deps = {
          let node_id = graph_path.node_id();
          let parent_node = match self.graph.packages.get(&node_id) {
            Some(node) if node.no_peers => {
              continue; // skip, no need to analyze
            }
            Some(node) => node,
            None => {
              // todo(dsherret): I don't believe this should occur anymore
              // todo(THIS PR): add a debug assert
              continue;
            }
          };

          let resolved_node_id = self
            .graph
            .resolved_node_ids
            .get_resolved_id(&node_id)
            .unwrap();
          let deps = if let Some(deps) =
            self.dep_entry_cache.get(&resolved_node_id.id)
          {
            deps.clone()
          } else {
            // the api should have this in the cache at this point, so no need to parallelize
            match self.api.package_version_info(&resolved_node_id.id).await? {
              Some(version_info) => self
                .dep_entry_cache
                .store(resolved_node_id.id.clone(), &version_info)?,
              None => bail!(
                "Could not find version information for {}",
                resolved_node_id.id
              ),
            }
          };

          deps
        };

        // cache all the dependencies' registry infos in parallel if should
        if !should_sync_download() {
          let handles = deps
            .iter()
            .map(|dep| {
              let name = dep.name.clone();
              let api = self.api.clone();
              tokio::task::spawn(async move {
                // it's ok to call this without storing the result, because
                // NpmRegistryApi will cache the package info in memory
                api.package_info(&name).await
              })
            })
            .collect::<Vec<_>>();
          let results = futures::future::join_all(handles).await;
          for result in results {
            result??; // surface the first error
          }
        }

        // resolve the dependencies
        let mut found_peer = false;

        eprintln!("DEPS");
        eprintln!("----");
        for dep in deps.iter() {
          eprintln!("{}", dep.bare_specifier);
        }
        eprintln!("----");

        for dep in deps.iter() {
          let package_info = self.api.package_info(&dep.name).await?;

          let maybe_known_child_id = if let Some(child_id) = self
            .graph
            .packages
            .get(&graph_path.node_id())
            .unwrap()
            .children
            .get(&dep.bare_specifier)
            .copied()
          {
            // we already resolved this dependency before, and we can skip it
            self.try_add_pending_unresolved_node(&graph_path, child_id);
            Some(child_id)
          } else {
            None
          };

          match dep.kind {
            NpmDependencyEntryKind::Dep => {
              let node_id = if let Some(child_id) = maybe_known_child_id {
                child_id
              } else {
                self.analyze_dependency(dep, &package_info, &graph_path)?
              };

              if !found_peer {
                found_peer = !self.graph.borrow_node_mut(&node_id).no_peers;
              }
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              found_peer = true;
              if maybe_known_child_id.is_none() {
                eprintln!("Parent id: {:?}", graph_path.node_id());
                self.resolve_peer_dep(
                  &dep.bare_specifier,
                  dep,
                  &package_info,
                  &graph_path,
                )?;
              }
            }
          }
        }

        if !found_peer {
          self.graph.borrow_node_mut(&graph_path.node_id()).no_peers = true;
        }
      }
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    specifier: &str,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: &NpmPackageInfo,
    ancestor_path: &Arc<GraphPath>,
  ) -> Result<(), AnyError> {
    debug_assert!(matches!(
      peer_dep.kind,
      NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer
    ));

    eprintln!("HERE");
    self.graph.output_path(ancestor_path);

    // use this to detect cycles... this is just in case and probably won't happen
    let mut up_path = GraphPath::new(ancestor_path.node_id());
    let mut path = vec![ancestor_path];

    // todo(THIS PR): add a test for this
    // the current dependency might have had the peer dependency
    // in another bare specifier slot... if so resolve it to that
    let maybe_peer_dep_id = self.find_peer_dep_in_node(
      ancestor_path.node_id(),
      peer_dep,
      peer_package_info,
    )?;

    if let Some(peer_dep_id) = maybe_peer_dep_id {
      // handle optional dependency that's never been set
      if peer_dep.kind.is_optional() {
        self.set_previously_unresolved_optional_dependency(
          peer_dep_id,
          peer_dep,
          ancestor_path,
        );
        return Ok(());
      }

      //self.try_add_pending_unresolved_node(ancestor_path, peer_dep_id);
      // this will always have an ancestor because we're not at the root
      let parent = match ancestor_path.ancestors().next().unwrap() {
        GraphPathNodeOrRoot::Node(node) => NodeParent::Node(node.node_id()),
        GraphPathNodeOrRoot::Root(pkg_id) => NodeParent::Root(pkg_id.clone()),
      };
      self.set_new_peer_dep(parent, path, specifier, peer_dep_id);
      return Ok(());
    }

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut ancestor_iterator = ancestor_path.ancestors().peekable();
    while let Some(ancestor_node) = ancestor_iterator.next() {
      match ancestor_node {
        GraphPathNodeOrRoot::Node(ancestor_graph_path_node) => {
          path.push(ancestor_graph_path_node);
          let ancestor_node_id = ancestor_graph_path_node.node_id();
          let maybe_peer_dep_id = self.find_peer_dep_in_node(
            ancestor_node_id,
            peer_dep,
            peer_package_info,
          )?;
          if let Some(peer_dep_id) = maybe_peer_dep_id {
            // handle optional dependency that's never been set
            if peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                peer_dep_id,
                peer_dep,
                ancestor_path,
              );
              return Ok(());
            }

            //self.try_add_pending_unresolved_node(ancestor_path, peer_dep_id);
            // this will always have an ancestor because we're not at the root
            let parent = match ancestor_iterator.peek().unwrap() {
              GraphPathNodeOrRoot::Node(node) => {
                NodeParent::Node(node.node_id())
              }
              GraphPathNodeOrRoot::Root(pkg_id) => {
                NodeParent::Root(pkg_id.clone())
              }
            };
            self.set_new_peer_dep(parent, path, specifier, peer_dep_id);
            return Ok(());
          }

          up_path = match up_path.with_id(ancestor_node_id) {
            Some(up_path) => up_path,
            None => {
              // circular, shouldn't happen
              debug_assert!(false, "should not be circular");
              break;
            }
          };
        }
        GraphPathNodeOrRoot::Root(root_pkg_id) => {
          // in this case, the parent is the root so the children are all the package requirements
          if let Some(child_id) = find_matching_child(
            peer_dep,
            peer_package_info,
            self
              .graph
              .root_packages
              .iter()
              .map(|(pkg_id, id)| (*id, pkg_id)),
          )? {
            // handle optional dependency that's never been set
            if peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                child_id,
                peer_dep,
                ancestor_path,
              );
              return Ok(());
            }

            //self.try_add_pending_unresolved_node(ancestor_path, child_id);
            self.set_new_peer_dep(
              NodeParent::Root(root_pkg_id.clone()),
              path,
              specifier,
              child_id,
            );
            return Ok(());
          }
        }
      }
    }

    // We didn't find anything by searching the ancestor siblings, so we need
    // to resolve based on the package info
    if !peer_dep.kind.is_optional() {
      let parent_id = ancestor_path.node_id();
      let (_, node_id) = self.resolve_node_from_info(
        &peer_dep.name,
        peer_dep
          .peer_dep_version_req
          .as_ref()
          .unwrap_or(&peer_dep.version_req),
        peer_package_info,
        Some(parent_id),
      )?;
      let parent = match ancestor_path.previous_node.as_ref().unwrap() {
        GraphPathNodeOrRoot::Node(node) => NodeParent::Node(node.node_id()),
        GraphPathNodeOrRoot::Root(req) => NodeParent::Root(req.clone()),
      };
      self.set_new_peer_dep(parent, vec![ancestor_path], specifier, node_id);
    }

    Ok(())
  }

  fn find_peer_dep_in_node(
    &self,
    node_id: NodeId,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: &NpmPackageInfo,
  ) -> Result<Option<NodeId>, AnyError> {
    let node_id = node_id;
    let resolved_node_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&node_id)
      .unwrap();
    // check if this node itself is a match for
    // the peer dependency and if so use that
    if resolved_node_id.id.name == peer_dep.name
      && version_req_satisfies(
        &peer_dep.version_req,
        &resolved_node_id.id.version,
        peer_package_info,
        None,
      )?
    {
      Ok(Some(node_id))
    } else {
      // todo(THIS PR): improve this
      find_matching_child(
        peer_dep,
        peer_package_info,
        self
          .graph
          .packages
          .get(&node_id)
          .unwrap()
          .children
          .values()
          .map(|child_node_id| {
            (
              *child_node_id,
              &self
                .graph
                .resolved_node_ids
                .get_resolved_id(child_node_id)
                .unwrap()
                .id,
            )
          }),
      )
    }
  }

  /// Optional peer dependencies that have never been set before are
  /// simply added to the existing peer dependency instead of affecting
  /// the entire sub tree.
  fn set_previously_unresolved_optional_dependency(
    &mut self,
    peer_dep_id: NodeId,
    peer_dep: &NpmDependencyEntry,
    visited_ancestor_versions: &Arc<GraphPath>,
  ) {
    self.graph.set_child_parent(
      &peer_dep.bare_specifier,
      peer_dep_id,
      &NodeParent::Node(visited_ancestor_versions.node_id()),
    );
    self
      .try_add_pending_unresolved_node(visited_ancestor_versions, peer_dep_id);
  }

  fn set_new_peer_dep(
    &mut self,
    mut node_parent: NodeParent,
    // path from the node above the resolved dep to just above the peer dep
    path: Vec<&Arc<GraphPath>>,
    peer_dep_specifier: &str,
    peer_dep_id: NodeId,
  ) {
    let peer_dep_id = peer_dep_id;
    let peer_dep_resolved_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&peer_dep_id)
      .unwrap()
      .clone();

    let mut had_created_node = false;

    for graph_path_node in path.iter().rev() {
      let node_id = graph_path_node.node_id();
      let old_resolved_id = self
        .graph
        .resolved_node_ids
        .get_resolved_id(&node_id)
        .unwrap()
        .clone();
      if old_resolved_id
        .peer_dependencies
        .contains(&peer_dep_resolved_id)
      {
        // some other path already resolved the same peer dependency for this node
        node_parent = NodeParent::Node(node_id);
        continue;
      }

      let mut new_resolved_id = old_resolved_id.clone();
      new_resolved_id
        .peer_dependencies
        .push(peer_dep_resolved_id.clone());

      let current_node = self.graph.packages.get(&node_id).unwrap();
      if current_node.has_one_parent() {
        // in this case, we can just update the current node in place and only
        // update the collection of resolved node identifiers
        self.graph.resolved_node_ids.set(node_id, new_resolved_id);
        node_parent = NodeParent::Node(node_id);
      } else {
        let old_node_id = node_id;
        let (created, new_node_id) =
          self.graph.get_or_create_for_id(&new_resolved_id);
        debug_assert!(created); // these should always be created

        debug_assert_eq!(graph_path_node.node_id(), old_node_id);
        graph_path_node.change_id(new_node_id);

        if !had_created_node {
          // add the top node to the list of pending unresolved nodes
          self
            .pending_unresolved_nodes
            .push_back((**graph_path_node).clone());
          had_created_node = true;
        }

        // update the current node to not have the parent
        {
          let node = self.graph.borrow_node_mut(&node_id);
          node.remove_parent(&node_parent);
        }

        // update the parent to point to this new node
        match &node_parent {
          NodeParent::Root(pkg_id) => {
            self.graph.root_packages.insert(pkg_id.clone(), new_node_id);
          }
          NodeParent::Node(parent_node_id) => {
            let parent_node = self.graph.borrow_node_mut(parent_node_id);
            for child_id in parent_node.children.values_mut() {
              if *child_id == old_node_id {
                *child_id = new_node_id;
              }
            }
          }
        }

        // update the new node to have the parent
        {
          let new_node = self.graph.borrow_node_mut(&new_node_id);
          new_node.add_parent(node_parent.clone());
        }

        node_parent = NodeParent::Node(new_node_id);
      }
    }

    match node_parent {
      NodeParent::Node(node_id) => {
        // handle this node having a previous child due to another peer dependency
        let node = self.graph.borrow_node(&node_id);
        if let Some(child_id) = node.children.remove(peer_dep_specifier) {
          eprintln!("SPECIFIER: {}", peer_dep_specifier);
          eprintln!("NODE ID: {:?}", node_id);

          self.graph.output_path(&path[0]);

          eprintln!(
            "{} {} {}",
            self
              .graph
              .resolved_node_ids
              .get_resolved_id(&node_id)
              .unwrap()
              .as_serialized(),
            self
              .graph
              .resolved_node_ids
              .get_resolved_id(&child_id)
              .unwrap()
              .as_serialized(),
            self
              .graph
              .resolved_node_ids
              .get_resolved_id(&peer_dep_id)
              .unwrap()
              .as_serialized()
          );
          debug_assert!(false, "what, why would this happen?"); // todo: update
        }

        eprintln!(
          "RESOLVING: {} {}",
          self
            .graph
            .resolved_node_ids
            .get_resolved_id(&node_id)
            .unwrap()
            .as_serialized(),
          self
            .graph
            .resolved_node_ids
            .get_resolved_id(&peer_dep_id)
            .unwrap()
            .as_serialized()
        );

        // todo(THIS PR): revert to debug
        eprintln!(
          "Resolved peer dependency for {} in {} to {}",
          peer_dep_specifier,
          &self
            .graph
            .resolved_node_ids
            .get_resolved_id(&node_id)
            .unwrap()
            .as_serialized(),
          &self
            .graph
            .resolved_node_ids
            .get_resolved_id(&peer_dep_id)
            .unwrap()
            .as_serialized(),
        );

        self.graph.set_child_parent_node(
          peer_dep_specifier,
          peer_dep_id,
          node_id,
        );
      }
      NodeParent::Root(_) => {
        unreachable!();
      }
    }
  }
}

fn find_matching_child<'a>(
  peer_dep: &NpmDependencyEntry,
  peer_package_info: &NpmPackageInfo,
  children: impl Iterator<Item = (NodeId, &'a NpmPackageId)>,
) -> Result<Option<NodeId>, AnyError> {
  for (child_id, pkg_id) in children {
    if pkg_id.name == peer_dep.name
      && version_req_satisfies(
        &peer_dep.version_req,
        &pkg_id.version,
        peer_package_info,
        None,
      )?
    {
      return Ok(Some(child_id));
    }
  }
  Ok(None)
}

#[cfg(test)]
mod test {
  use deno_graph::npm::NpmPackageReqReference;
  use pretty_assertions::assert_eq;

  use crate::npm::registry::TestNpmRegistryApiInner;

  use super::*;

  #[tokio::test]
  async fn resolve_deps_no_peer() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "0.1.0");
    api.ensure_package_version("package-c", "0.0.10");
    api.ensure_package_version("package-d", "3.2.1");
    api.ensure_package_version("package-d", "3.2.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^0.1"));
    api.add_dependency(("package-c", "0.1.0"), ("package-d", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized("package-c@0.1.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-c@0.1.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageResolvedId::from_serialized("package-d@3.2.1").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-d@3.2.1")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_deps_circular() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_dependency(("package-b", "2.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-a@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_with_peer_deps_top_tree() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-peer", "4.0.0");
    api.ensure_package_version("package-peer", "4.1.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-peer", "4"));
    api.add_peer_dependency(("package-c", "3.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      // the peer dependency is specified here at the top of the tree
      // so it should resolve to 4.0.0 instead of 4.1.0
      vec!["npm:package-a@1", "npm:package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-peer@4.0.0".to_string()
        ),
        (
          "package-peer@4.0.0".to_string(),
          "package-peer@4.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_with_peer_deps_ancestor_sibling_not_top_tree() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.1.1");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-peer", "4.0.0");
    api.ensure_package_version("package-peer", "4.1.0");
    api.add_dependency(("package-0", "1.1.1"), ("package-a", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    // the peer dependency is specified here as a sibling of "a" and "b"
    // so it should resolve to 4.0.0 instead of 4.1.0
    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "4.0.0"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-peer", "4"));
    api.add_peer_dependency(("package-c", "3.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.1.1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-0@1.1.1")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-a@1.0.0_package-peer@4.0.0"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
                .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.1.1".to_string(), "package-0@1.1.1".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_with_peer_deps_auto_resolved() {
    // in this case, the peer dependency is not found in the tree
    // so it's auto-resolved based on the registry
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-peer", "4.0.0");
    api.ensure_package_version("package-peer", "4.1.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-peer", "4"));
    api.add_peer_dependency(("package-c", "3.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.1.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-c@3.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.1.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@4.1.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_with_optional_peer_dep_not_resolved() {
    // in this case, the peer dependency is not found in the tree
    // so it's auto-resolved based on the registry
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-peer", "4.0.0");
    api.ensure_package_version("package-peer", "4.1.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    api.add_optional_peer_dependency(
      ("package-b", "2.0.0"),
      ("package-peer", "4"),
    );
    api.add_optional_peer_dependency(
      ("package-c", "3.0.0"),
      ("package-peer", "*"),
    );

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-c@3.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_with_optional_peer_found() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-peer", "4.0.0");
    api.ensure_package_version("package-peer", "4.1.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    api.add_optional_peer_dependency(
      ("package-b", "2.0.0"),
      ("package-peer", "4"),
    );
    api.add_optional_peer_dependency(
      ("package-c", "3.0.0"),
      ("package-peer", "*"),
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1", "npm:package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-c@3.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-peer@4.0.0".to_string(),
          "package-peer@4.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_optional_peer_first_not_resolved_second_resolved_scenario1()
  {
    // When resolving a dependency a second time and it has an optional
    // peer dependency that wasn't previously resolved, it should resolve all the
    // previous versions to the new one
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "^1"));
    api.add_optional_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-peer", "*"),
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1", "npm:package-b@1"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@1.0.0").unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
                .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@1.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
              .unwrap(),
          )]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        ("package-b@1".to_string(), "package-b@1.0.0".to_string())
      ]
    );
  }

  #[tokio::test]
  async fn resolve_optional_peer_first_not_resolved_second_resolved_scenario2()
  {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");
    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "*"),
    );
    api.add_dependency(("package-b", "1.0.0"), ("package-a", "1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "2.0.0"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1", "npm:package-b@1"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@1.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageResolvedId::from_serialized("package-a@1.0.0").unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
                .unwrap(),
            )
          ]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        ("package-b@1".to_string(), "package-b@1.0.0".to_string())
      ]
    );
  }

  #[tokio::test]
  async fn resolve_optional_dep_npm_req_top() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "*"),
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1", "npm:package-peer@1"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-peer@1".to_string(),
          "package-peer@1.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_optional_dep_different_resolution_second_time() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");
    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "*"),
    );
    api.add_dependency(("package-b", "1.0.0"), ("package-a", "1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "2.0.0"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec![
        "npm:package-a@1",
        "npm:package-b@1",
        "npm:package-peer@1.0.0",
      ],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 1,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
                .unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-a@1.0.0_package-peer@2.0.0"
              )
              .unwrap(),
            ),
          ]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@2.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-peer@2.0.0".to_string()
        ),
        (
          "package-peer@1.0.0".to_string(),
          "package-peer@1.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_nested_peer_deps_auto_resolved() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-peer-a", "2.0.0");
    api.ensure_package_version("package-peer-b", "3.0.0");
    api.add_peer_dependency(("package-0", "1.0.0"), ("package-peer-a", "2"));
    api.add_peer_dependency(
      ("package-peer-a", "2.0.0"),
      ("package-peer-b", "3"),
    );

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-0@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-a@2.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-a@2.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-b@3.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-b@3.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.0".to_string(), "package-0@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_nested_peer_deps_ancestor_sibling_deps() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-peer-a", "2.0.0");
    api.ensure_package_version("package-peer-b", "3.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-peer-b", "*"));
    api.add_peer_dependency(("package-0", "1.0.0"), ("package-peer-a", "2"));
    api.add_peer_dependency(
      ("package-peer-a", "2.0.0"),
      ("package-peer-b", "3"),
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec![
        "npm:package-0@1.0",
        "npm:package-peer-a@2",
        "npm:package-peer-b@3",
      ],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-0@1.0.0_package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-peer-a@2.0.0_package-peer-b@3.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer-b@3.0.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-b@3.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-b@3.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-0@1.0".to_string(),
          "package-0@1.0.0_package-peer-a@2.0.0_package-peer-b@3.0.0"
            .to_string()
        ),
        (
          "package-peer-a@2".to_string(),
          "package-peer-a@2.0.0_package-peer-b@3.0.0".to_string()
        ),
        (
          "package-peer-b@3".to_string(),
          "package-peer-b@3.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_with_peer_deps_multiple() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.1.1");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-c", "3.0.0");
    api.ensure_package_version("package-d", "3.5.0");
    api.ensure_package_version("package-e", "3.6.0");
    api.ensure_package_version("package-peer-a", "4.0.0");
    api.ensure_package_version("package-peer-a", "4.1.0");
    api.ensure_package_version("package-peer-b", "5.3.0");
    api.ensure_package_version("package-peer-b", "5.4.1");
    api.ensure_package_version("package-peer-c", "6.2.0");
    api.add_dependency(("package-0", "1.1.1"), ("package-a", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^2"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "^3"));
    api.add_dependency(("package-a", "1.0.0"), ("package-d", "^3"));
    api.add_dependency(("package-a", "1.0.0"), ("package-peer-a", "4.0.0"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-peer-a", "4"));
    api.add_peer_dependency(
      ("package-b", "2.0.0"),
      ("package-peer-c", "=6.2.0"), // will be auto-resolved
    );
    api.add_peer_dependency(("package-c", "3.0.0"), ("package-peer-a", "*"));
    api.add_peer_dependency(
      ("package-peer-a", "4.0.0"),
      ("package-peer-b", "^5.4"), // will be auto-resolved
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-0@1.1.1", "npm:package-e@3"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-0@1.1.1")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-a@1.0.0_package-peer-a@4.0.0"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-b@2.0.0_package-peer-a@4.0.0_package-peer-c@6.2.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@3.0.0_package-peer-a@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageResolvedId::from_serialized("package-d@3.5.0").unwrap(),
            ),
            (
              "package-peer-a".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-peer-a@4.0.0_package-peer-b@5.4.1"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-peer-a@4.0.0_package-peer-c@6.2.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0")
                .unwrap(),
            ),
            (
              "package-peer-c".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer-c@6.2.0")
                .unwrap(),
            )
          ])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@3.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-d@3.5.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-e@3.6.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-b@5.4.1")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-b@5.4.1")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-c@6.2.0")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-0@1.1.1".to_string(), "package-0@1.1.1".to_string()),
        ("package-e@3".to_string(), "package-e@3.6.0".to_string()),
      ]
    );
  }

  #[tokio::test]
  async fn resolve_peer_deps_circular() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-b@2.0.0_package-a@1.0.0"
            )
            .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-a@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-a@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_peer_deps_multiple_copies() {
    // repeat this a few times to have a higher probability of surfacing indeterminism
    for _ in 0..3 {
      let api = TestNpmRegistryApiInner::default();
      api.ensure_package_version("package-a", "1.0.0");
      api.ensure_package_version("package-b", "2.0.0");
      api.ensure_package_version("package-dep", "3.0.0");
      api.ensure_package_version("package-peer", "4.0.0");
      api.ensure_package_version("package-peer", "5.0.0");
      api.add_dependency(("package-a", "1.0.0"), ("package-dep", "*"));
      api.add_dependency(("package-a", "1.0.0"), ("package-peer", "4"));
      api.add_dependency(("package-b", "2.0.0"), ("package-dep", "*"));
      api.add_dependency(("package-b", "2.0.0"), ("package-peer", "5"));
      api.add_peer_dependency(("package-dep", "3.0.0"), ("package-peer", "*"));

      let (packages, package_reqs) = run_resolver_and_get_output(
        api,
        vec!["npm:package-a@1", "npm:package-b@2"],
      )
      .await;
      assert_eq!(
        packages,
        vec![
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized(
              "package-a@1.0.0_package-peer@4.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([
              (
                "package-dep".to_string(),
                NpmPackageResolvedId::from_serialized(
                  "package-dep@3.0.0_package-peer@4.0.0"
                )
                .unwrap(),
              ),
              (
                "package-peer".to_string(),
                NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
                  .unwrap(),
              ),
            ]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized(
              "package-b@2.0.0_package-peer@5.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([
              (
                "package-dep".to_string(),
                NpmPackageResolvedId::from_serialized(
                  "package-dep@3.0.0_package-peer@5.0.0"
                )
                .unwrap(),
              ),
              (
                "package-peer".to_string(),
                NpmPackageResolvedId::from_serialized("package-peer@5.0.0")
                  .unwrap(),
              ),
            ]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized(
              "package-dep@3.0.0_package-peer@4.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([(
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
                .unwrap(),
            )]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized(
              "package-dep@3.0.0_package-peer@5.0.0"
            )
            .unwrap(),
            copy_index: 1,
            dependencies: HashMap::from([(
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@5.0.0")
                .unwrap(),
            )]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized("package-peer@4.0.0")
              .unwrap(),
            copy_index: 0,
            dependencies: HashMap::new(),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageResolvedId::from_serialized("package-peer@5.0.0")
              .unwrap(),
            copy_index: 0,
            dependencies: HashMap::new(),
            dist: Default::default(),
          },
        ]
      );
      assert_eq!(
        package_reqs,
        vec![
          (
            "package-a@1".to_string(),
            "package-a@1.0.0_package-peer@4.0.0".to_string()
          ),
          (
            "package-b@2".to_string(),
            "package-b@2.0.0_package-peer@5.0.0".to_string()
          )
        ]
      );
    }
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_dep_then_peer() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1.0", "npm:package-b@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-b@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@1.0.0_package-b@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@1.0.0_package-b@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-b@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-a@1.0".to_string(),
          "package-a@1.0.0_package-b@1.0.0".to_string()
        ),
        ("package-b@1.0".to_string(), "package-b@1.0.0".to_string())
      ]
    );
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_dep_then_different_peer() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.1.0");
    api.ensure_package_version("package-peer", "1.2.0");
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "*")); // should select 1.2.0
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "=1.1.0"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-a", "1"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1.0", "npm:package-b@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.2.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 1,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.1.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@1.0.0_package-a@1.0.0_package-peer@1.1.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer@1.1.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@1.0.0_package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-a@1.0.0_package-peer@1.1.0"
            )
            .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.1.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer@1.2.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1.0".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-b@1.0".to_string(),
          "package-b@1.0.0_package-a@1.0.0_package-peer@1.1.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_dep_and_peer_dist_tag() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.ensure_package_version("package-b", "3.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "some-tag"));
    api.add_dependency(("package-a", "1.0.0"), ("package-d", "1.0.0"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1.0.0"));
    api.add_dependency(("package-a", "1.0.0"), ("package-e", "1.0.0"));
    api.add_dependency(("package-e", "1.0.0"), ("package-b", "some-tag"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-d", "other-tag"));
    api.add_dist_tag("package-b", "some-tag", "2.0.0");
    api.add_dist_tag("package-d", "other-tag", "1.0.0");

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-d@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@1.0.0_package-d@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageResolvedId::from_serialized("package-d@1.0.0").unwrap(),
            ),
            (
              "package-e".to_string(),
              NpmPackageResolvedId::from_serialized("package-e@1.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-b@2.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@1.0.0_package-d@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageResolvedId::from_serialized("package-d@1.0.0").unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-d@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-e@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageResolvedId::from_serialized("package-b@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![(
        "package-a@1.0".to_string(),
        "package-a@1.0.0_package-d@1.0.0".to_string()
      ),]
    );
  }

  #[tokio::test]
  async fn package_has_self_as_dependency() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![NpmResolutionPackage {
        pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
          .unwrap(),
        copy_index: 0,
        // in this case, we just ignore that the package did this
        dependencies: Default::default(),
        dist: Default::default(),
      }]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn package_has_self_but_different_version_as_dependency() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-a", "0.5.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-a", "^0.5"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@0.5.0")
            .unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized("package-a@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-a@0.5.0").unwrap(),
          )]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  async fn run_resolver_and_get_output(
    api: TestNpmRegistryApiInner,
    reqs: Vec<&str>,
  ) -> (Vec<NpmResolutionPackage>, Vec<(String, String)>) {
    let mut graph = Graph::default();
    let api = NpmRegistryApi::new_for_test(api);
    let mut resolver = GraphDependencyResolver::new(&mut graph, &api);

    for req in reqs {
      let req = NpmPackageReqReference::from_str(req).unwrap().req;
      resolver
        .add_package_req(&req, &api.package_info(&req.name).await.unwrap())
        .unwrap();
    }

    resolver.resolve_pending().await.unwrap();
    let snapshot = graph.into_snapshot(&api).await.unwrap();
    let mut packages = snapshot.all_packages();
    packages.sort_by(|a, b| a.pkg_id.cmp(&b.pkg_id));
    let mut package_reqs = snapshot
      .package_reqs
      .into_iter()
      .map(|(a, b)| {
        (
          a.to_string(),
          snapshot.root_packages.get(&b).unwrap().as_serialized(),
        )
      })
      .collect::<Vec<_>>();
    package_reqs.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    (packages, package_reqs)
  }
}
