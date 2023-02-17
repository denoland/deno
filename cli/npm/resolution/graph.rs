// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

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
struct NodeId(usize);

/// Path of visited node identifiers.
#[derive(Clone)]
struct VisitedNodeIds {
  previous_node: Option<Arc<VisitedNodeIds>>,
  node_id: Arc<Mutex<NodeId>>,
}

impl VisitedNodeIds {
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

  pub fn with_parent(
    self: &Arc<VisitedNodeIds>,
    parent: &NodeParent,
  ) -> Option<Arc<Self>> {
    match parent {
      NodeParent::Node(id) => self.with_id(*id),
      NodeParent::Root(_) => Some(self.clone()),
    }
  }

  pub fn with_id(
    self: &Arc<VisitedNodeIds>,
    node_id: NodeId,
  ) -> Option<Arc<Self>> {
    if self.has_visited(node_id) {
      None
    } else {
      Some(Arc::new(Self {
        previous_node: Some(self.clone()),
        node_id: Arc::new(Mutex::new(node_id)),
      }))
    }
  }

  pub fn has_visited(self: &Arc<Self>, node_id: NodeId) -> bool {
    let mut maybe_next_node = Some(self);
    while let Some(next_node) = maybe_next_node {
      // stop once we encounter the same id
      if next_node.node_id() == node_id {
        return true;
      }
      maybe_next_node = next_node.previous_node.as_ref();
    }
    false
  }
}

/// A memory efficient path of the visited specifiers in the tree.
#[derive(Default, Clone)]
struct GraphSpecifierPath {
  previous_node: Option<Arc<GraphSpecifierPath>>,
  specifier: String,
}

impl GraphSpecifierPath {
  pub fn new(specifier: String) -> Arc<Self> {
    Arc::new(Self {
      previous_node: None,
      specifier,
    })
  }

  pub fn with_specifier(self: &Arc<Self>, specifier: String) -> Arc<Self> {
    Arc::new(Self {
      previous_node: Some(self.clone()),
      specifier,
    })
  }

  pub fn pop(&self) -> Option<&Arc<Self>> {
    self.previous_node.as_ref()
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
  // Use BTreeMap in order to create determinism
  // when going up and down the tree
  pub parents: BTreeMap<String, BTreeSet<NodeParent>>,
  pub children: BTreeMap<String, NodeId>,
  pub deps: Arc<Vec<NpmDependencyEntry>>,
  /// Whether the node has demonstrated to have no peer dependencies in its
  /// descendants. If this is true then we can skip analyzing this node
  /// again when we encounter it another time in the dependency tree, which
  /// is much faster.
  pub no_peers: bool,
}

impl Node {
  pub fn add_parent(&mut self, specifier: String, parent: NodeParent) {
    self.parents.entry(specifier).or_default().insert(parent);
  }

  pub fn remove_parent(&mut self, specifier: &str, parent: &NodeParent) {
    if let Some(parents) = self.parents.get_mut(specifier) {
      parents.remove(parent);
      if parents.is_empty() {
        self.parents.remove(specifier);
      }
    }
  }
}

#[derive(Debug, Default)]
struct ResolvedNodeIds {
  // bidirectional map
  from: HashMap<NpmPackageResolvedId, NodeId>,
  to: HashMap<NodeId, NpmPackageResolvedId>,
}

impl ResolvedNodeIds {
  pub fn insert(&mut self, resolved_id: NpmPackageResolvedId, node_id: NodeId) {
    self.from.insert(resolved_id.clone(), node_id.clone());
    self.to.insert(node_id.clone(), resolved_id.clone());
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
    self.from.get(id).copied()
  }
}

#[derive(Debug, Default)]
pub struct Graph {
  // Need a running count and can't derive this because the
  // packages hashmap could be removed from.
  next_package_id: usize,
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
      root_packages: HashMap::with_capacity(snapshot.root_packages.len()),
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
        // insert at an empty magic specifier
        .add_parent("".to_string(), NodeParent::Root(id.clone()));
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
    if let Some(node_id) = self.resolved_node_ids.get_node_id(resolved_id) {
      (false, node_id)
    } else {
      let node_id = NodeId(self.next_package_id);
      self.next_package_id += 1;
      let node = Node {
        parents: Default::default(),
        children: Default::default(),
        deps: Default::default(),
        no_peers: false,
      };
      self
        .packages_by_name
        .entry(resolved_id.id.name.clone())
        .or_default()
        .push(node_id);
      self.packages.insert(node_id, node);
      self.resolved_node_ids.insert(resolved_id.clone(), node_id);
      (true, node_id)
    }
  }

  fn borrow_node(&mut self, node_id: NodeId) -> &mut Node {
    self.packages.get_mut(&node_id).unwrap()
  }

  fn forget_orphan(&mut self, node_id: NodeId) {
    if let Some(node) = self.packages.remove(&node_id) {
      assert_eq!(node.parents.len(), 0);

      // Remove the id from the list of packages by name.
      let resolved_id = self.resolved_node_ids.remove(&node_id).unwrap();
      let packages_with_name =
        self.packages_by_name.get_mut(&resolved_id.id.name).unwrap();
      let remove_index = packages_with_name
        .iter()
        .position(|id| id == &node_id)
        .unwrap();
      packages_with_name.remove(remove_index);

      let parent = NodeParent::Node(node_id.clone());
      for (specifier, child_id) in &node.children {
        let child = self.borrow_node(*child_id);
        child.remove_parent(specifier, &parent);
        if child.parents.is_empty() {
          drop(child); // stop borrowing from self
          self.forget_orphan(*child_id);
        }
      }
    }
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
        node.add_parent(specifier.to_string(), NodeParent::Root(id.clone()));
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
    child.add_parent(specifier.to_string(), NodeParent::Node(parent_id));
  }

  fn remove_child_parent(
    &mut self,
    specifier: &str,
    child_id: NodeId,
    parent: &NodeParent,
  ) {
    match parent {
      NodeParent::Node(parent_id) => {
        let node = self.borrow_node(*parent_id);
        if let Some(removed_child_id) = node.children.remove(specifier) {
          assert_eq!(removed_child_id, child_id);
        }
      }
      NodeParent::Root(_) => {
        // ignore removing from the top level information because,
        // if this ever happens it means it's being replaced
      }
    }
    self.borrow_node(child_id).remove_parent(specifier, parent);
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
}

pub struct GraphDependencyResolver<'a> {
  graph: &'a mut Graph,
  api: &'a NpmRegistryApi,
  pending_unresolved_nodes: VecDeque<Arc<VisitedNodeIds>>,
}

impl<'a> GraphDependencyResolver<'a> {
  pub fn new(graph: &'a mut Graph, api: &'a NpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_unresolved_nodes: Default::default(),
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
    self
      .graph
      .set_child_parent("", node_id, &NodeParent::Root(pkg_id.id));
    self.try_add_pending_unresolved_node(None, node_id);
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
    self
      .graph
      .set_child_parent("", node_id, &NodeParent::Root(pkg_id.id));
    self.try_add_pending_unresolved_node(None, node_id);
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: &NpmPackageInfo,
    parent_id: NodeId,
    visited_versions: &Arc<VisitedNodeIds>,
  ) -> Result<NodeId, AnyError> {
    let (_, node_id) = self.resolve_node_from_info(
      &entry.name,
      match entry.kind {
        NpmDependencyEntryKind::Dep => &entry.version_req,
        // when resolving a peer dependency as a dependency, it should
        // use the "dependencies" entry version requirement if it exists
        NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer => {
          entry
            .peer_dep_version_req
            .as_ref()
            .unwrap_or(&entry.version_req)
        }
      },
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
      self.try_add_pending_unresolved_node(Some(visited_versions), node_id);
    }
    Ok(node_id)
  }

  fn try_add_pending_unresolved_node(
    &mut self,
    maybe_previous_visited_versions: Option<&Arc<VisitedNodeIds>>,
    node_id: NodeId,
  ) {
    if self.graph.packages.get(&node_id).unwrap().no_peers {
      return; // skip, no need to analyze this again
    }
    let visited_versions = match maybe_previous_visited_versions {
      Some(previous_visited_versions) => {
        match previous_visited_versions.with_id(node_id) {
          Some(visited_versions) => visited_versions,
          None => return, // circular, don't visit this node
        }
      }
      None => VisitedNodeIds::new(node_id),
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
    debug!(
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
    let (created, node_id) = self.graph.get_or_create_for_id(&resolved_id);
    if created {
      let mut node = self.graph.borrow_node(node_id);
      let mut deps = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| format!("npm package: {}", resolved_id.id))?;
      // Ensure name alphabetical and then version descending
      // so these are resolved in that order
      deps.sort();
      node.deps = Arc::new(deps);
      node.no_peers = node.deps.is_empty();
    }

    Ok((resolved_id, node_id))
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some(visited_versions) =
        self.pending_unresolved_nodes.pop_front()
      {
        let mut parent_id = visited_versions.node_id();
        let (deps, existing_children) = {
          let parent_node = match self.graph.packages.get(&parent_id) {
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

          (parent_node.deps.clone(), parent_node.children.clone())
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
        for dep in deps.iter() {
          let package_info = self.api.package_info(&dep.name).await?;

          match dep.kind {
            NpmDependencyEntryKind::Dep => {
              let node_id = self.analyze_dependency(
                dep,
                &package_info,
                parent_id,
                &visited_versions,
              )?;
              if !found_peer {
                found_peer = !self.graph.borrow_node(node_id).no_peers;
              }
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              found_peer = true;
              let maybe_new_parent_id = self.resolve_peer_dep(
                &dep.bare_specifier,
                parent_id,
                dep,
                &package_info,
                &visited_versions,
                existing_children.get(&dep.bare_specifier).copied(),
              )?;
              if let Some(new_parent_id) = maybe_new_parent_id {
                visited_versions.change_id(new_parent_id);
                parent_id = new_parent_id;
              }
            }
          }
        }

        if !found_peer {
          self.graph.borrow_node(parent_id).no_peers = true;
        }
      }
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    specifier: &str,
    parent_id: NodeId,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: &NpmPackageInfo,
    visited_ancestor_versions: &Arc<VisitedNodeIds>,
    existing_dep_id: Option<NodeId>,
  ) -> Result<Option<NodeId>, AnyError> {
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

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut pending_ancestors = VecDeque::new(); // go up the tree by depth
    let path = GraphSpecifierPath::new(specifier.to_string());
    let visited_versions = VisitedNodeIds::new(parent_id);

    // skip over the current node
    for (specifier, grand_parents) in
      self.graph.borrow_node(parent_id).parents.clone()
    {
      let path = path.with_specifier(specifier);
      for grand_parent in grand_parents {
        if let Some(visited_versions) =
          visited_versions.with_parent(&grand_parent)
        {
          pending_ancestors.push_back((
            grand_parent,
            path.clone(),
            visited_versions,
          ));
        }
      }
    }

    while let Some((ancestor, path, visited_versions)) =
      pending_ancestors.pop_front()
    {
      match &ancestor {
        NodeParent::Node(ancestor_node_id) => {
          let resolved_ancestor_id = self
            .graph
            .resolved_node_ids
            .get_resolved_id(ancestor_node_id)
            .unwrap();
          let maybe_peer_dep_id = if resolved_ancestor_id.id.name
            == peer_dep.name
            && version_req_satisfies(
              &peer_dep.version_req,
              &resolved_ancestor_id.id.version,
              peer_package_info,
              None,
            )? {
            Some(ancestor_node_id.clone())
          } else {
            let ancestor = self.graph.packages.get(ancestor_node_id).unwrap();
            for (specifier, parents) in &ancestor.parents {
              let new_path = path.with_specifier(specifier.clone());
              for parent in parents {
                if let Some(visited_versions) =
                  visited_versions.with_parent(parent)
                {
                  pending_ancestors.push_back((
                    parent.clone(),
                    new_path.clone(),
                    visited_versions,
                  ));
                }
              }
            }
            find_matching_child(
              peer_dep,
              peer_package_info,
              ancestor.children.values().map(|node_id| {
                (
                  *node_id,
                  &self
                    .graph
                    .resolved_node_ids
                    .get_resolved_id(node_id)
                    .unwrap()
                    .id,
                )
              }),
            )?
          };
          if let Some(peer_dep_id) = maybe_peer_dep_id {
            if existing_dep_id == Some(peer_dep_id) {
              return Ok(None); // do nothing, there's already an existing child dep id for this
            }

            // handle optional dependency that's never been set
            if existing_dep_id.is_none() && peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                peer_dep_id,
                parent_id,
                peer_dep,
                visited_ancestor_versions,
              );
              return Ok(None);
            }

            let parents =
              self.graph.borrow_node(*ancestor_node_id).parents.clone();
            return Ok(Some(self.set_new_peer_dep(
              parents,
              *ancestor_node_id,
              peer_dep_id,
              &path,
              visited_ancestor_versions,
            )));
          }
        }
        NodeParent::Root(root_pkg_id) => {
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
            if existing_dep_id == Some(child_id) {
              return Ok(None); // do nothing, there's already an existing child dep id for this
            }

            // handle optional dependency that's never been set
            if existing_dep_id.is_none() && peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                child_id,
                parent_id,
                peer_dep,
                visited_ancestor_versions,
              );
              return Ok(None);
            }

            let specifier = path.specifier.to_string();
            let path = path.pop().unwrap(); // go back down one level from the package requirement
            let old_id =
              self.graph.root_packages.get(&root_pkg_id).unwrap().clone();
            return Ok(Some(self.set_new_peer_dep(
              BTreeMap::from([(
                specifier,
                BTreeSet::from([NodeParent::Root(root_pkg_id.clone())]),
              )]),
              old_id,
              child_id,
              path,
              visited_ancestor_versions,
            )));
          }
        }
      }
    }

    // We didn't find anything by searching the ancestor siblings, so we need
    // to resolve based on the package info and will treat this just like any
    // other dependency when not optional
    if !peer_dep.kind.is_optional()
      // prefer the existing dep id if it exists
      && existing_dep_id.is_none()
    {
      self.analyze_dependency(
        peer_dep,
        peer_package_info,
        parent_id,
        visited_ancestor_versions,
      )?;
    }

    Ok(None)
  }

  /// Optional peer dependencies that have never been set before are
  /// simply added to the existing peer dependency instead of affecting
  /// the entire sub tree.
  fn set_previously_unresolved_optional_dependency(
    &mut self,
    peer_dep_id: NodeId,
    parent_id: NodeId,
    peer_dep: &NpmDependencyEntry,
    visited_ancestor_versions: &Arc<VisitedNodeIds>,
  ) {
    self.graph.set_child_parent(
      &peer_dep.bare_specifier,
      peer_dep_id,
      &NodeParent::Node(parent_id),
    );
    self.try_add_pending_unresolved_node(
      Some(visited_ancestor_versions),
      peer_dep_id,
    );
  }

  fn set_new_peer_dep(
    &mut self,
    previous_parents: BTreeMap<String, BTreeSet<NodeParent>>,
    node_id: NodeId,
    peer_dep_id: NodeId,
    path: &Arc<GraphSpecifierPath>,
    visited_ancestor_versions: &Arc<VisitedNodeIds>,
  ) -> NodeId {
    let peer_dep_id = peer_dep_id;
    let peer_dep_resolved_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&peer_dep_id)
      .unwrap();
    let old_id = node_id;
    let old_resolved_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&old_id)
      .unwrap();
    let (new_id, mut old_node_children) = if old_resolved_id
      .peer_dependencies
      .contains(&peer_dep_resolved_id)
      || old_id == peer_dep_id
    {
      // the parent has already resolved to using this peer dependency
      // via some other path or the parent is the peer dependency,
      // so we don't need to update its ids, but instead only make a link to it
      (
        old_id.clone(),
        self.graph.borrow_node(old_id).children.clone(),
      )
    } else {
      let mut new_id = old_resolved_id.clone();
      new_id.peer_dependencies.push(peer_dep_resolved_id.clone());

      // remove the previous parents from the old node
      let old_node_children = {
        for (specifier, parents) in &previous_parents {
          for parent in parents {
            self.graph.remove_child_parent(specifier, old_id, parent);
          }
        }
        let old_node = self.graph.borrow_node(old_id);
        old_node.children.clone()
      };

      let (_, new_node_id) = self.graph.get_or_create_for_id(&new_id);

      // update the previous parent to point to the new node
      // and this node to point at those parents
      for (specifier, parents) in previous_parents {
        for parent in parents {
          self
            .graph
            .set_child_parent(&specifier, new_node_id, &parent);
        }
      }

      // now add the previous children to this node
      let new_id_as_parent = NodeParent::Node(new_node_id.clone());
      for (specifier, child_id) in &old_node_children {
        self
          .graph
          .set_child_parent(specifier, *child_id, &new_id_as_parent);
      }
      (new_node_id, old_node_children)
    };

    // this is the parent id found at the bottom of the path
    let mut bottom_parent_id = new_id.clone();

    // continue going down the path
    let next_specifier = &path.specifier;
    if let Some(path) = path.pop() {
      let next_node_id = old_node_children.get(next_specifier).unwrap();
      bottom_parent_id = self.set_new_peer_dep(
        BTreeMap::from([(
          next_specifier.to_string(),
          BTreeSet::from([NodeParent::Node(new_id.clone())]),
        )]),
        *next_node_id,
        peer_dep_id,
        path,
        visited_ancestor_versions,
      );
    } else {
      // this means we're at the peer dependency now
      debug!(
        "Resolved peer dependency for {} in {} to {}",
        next_specifier,
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

      // handle this node having a previous child due to another peer dependency
      if let Some(child_id) = old_node_children.remove(next_specifier) {
        if let Some(node) = self.graph.packages.get_mut(&child_id) {
          let is_orphan = {
            node
              .remove_parent(next_specifier, &NodeParent::Node(new_id.clone()));
            node.parents.is_empty()
          };
          if is_orphan {
            self.graph.forget_orphan(child_id);
          }
        }
      }

      self.try_add_pending_unresolved_node(
        Some(visited_ancestor_versions),
        peer_dep_id,
      );
      self
        .graph
        .set_child_parent_node(next_specifier, peer_dep_id, new_id);
    }

    // forget the old node at this point if it has no parents
    if new_id != old_id {
      let old_node = self.graph.borrow_node(old_id);
      if old_node.parents.is_empty() {
        drop(old_node); // stop borrowing
        self.graph.forget_orphan(old_id);
      }
    }

    bottom_parent_id
  }
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
      ("package-peer-c", "=6.2.0"),
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
                "package-b@2.0.0_package-peer-a@4.0.0"
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
              NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0")
                .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-peer-a@4.0.0"
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
