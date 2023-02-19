// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_graph::npm::NpmPackageNv;
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
use crate::npm::resolution::snapshot::SnapshotPackageCopyIndexResolver;
use crate::npm::NpmRegistryApi;

use super::common::version_req_satisfies;
use super::snapshot::NpmResolutionSnapshot;
use super::NpmPackageResolvedId;
use super::NpmResolutionPackage;

pub static LATEST_VERSION_REQ: Lazy<VersionReq> =
  Lazy::new(|| VersionReq::parse_from_specifier("latest").unwrap());

// todo(THIS PR): find a better way to represent the difference between a node
// that is unique for peer dependencies and other nodes

// todo(THIS PR): make this private
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeId(u32);

#[derive(Clone)]
enum GraphPathNodeOrRoot {
  Node(Arc<GraphPath>),
  Root(NpmPackageNv),
}

#[derive(Clone, Debug)]
struct NodeIdRef(Arc<Mutex<NodeId>>);

impl NodeIdRef {
  pub fn new(node_id: NodeId) -> Self {
    NodeIdRef(Arc::new(Mutex::new(node_id)))
  }

  pub fn change(&self, node_id: NodeId) {
    *self.0.lock() = node_id;
  }

  pub fn get(&self) -> NodeId {
    *self.0.lock()
  }
}

/// Path through the graph.
#[derive(Clone)]
struct GraphPath {
  previous_node: Option<GraphPathNodeOrRoot>,
  node_id_ref: NodeIdRef,
}

impl GraphPath {
  pub fn for_root(node_id_ref: NodeIdRef, pkg_nv: NpmPackageNv) -> Arc<Self> {
    Arc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Root(pkg_nv)),
      node_id_ref,
    })
  }

  pub fn new(node_id_ref: NodeIdRef) -> Arc<Self> {
    Arc::new(Self {
      previous_node: None,
      node_id_ref,
    })
  }

  // todo(this pr): maybe remove this?
  pub fn node_id_ref(&self) -> &NodeIdRef {
    &self.node_id_ref
  }

  pub fn node_id(&self) -> NodeId {
    self.node_id_ref.get()
  }

  pub fn change_id(&self, node_id: NodeId) {
    self.node_id_ref.change(node_id)
  }

  pub fn with_id(
    self: &Arc<GraphPath>,
    node_id_ref: NodeIdRef,
  ) -> Option<Arc<Self>> {
    if self.has_visited(node_id_ref.get()) {
      None
    } else {
      Some(Arc::new(Self {
        previous_node: Some(GraphPathNodeOrRoot::Node(self.clone())),
        node_id_ref,
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
  Root(NpmPackageNv),
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
  //
  // Note: We don't want to store the children as a `NodeRef` because
  // multiple paths might visit through the children and we don't want
  // to share those references with those paths.
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

#[derive(Debug, Clone)]
enum ResolvedIdPeerDep {
  // A node that was created during snapshotting. We can hold a direct reference
  // to it because full resolution of this node has already occurred.
  SnapshotNodeId(NodeId),
  /// This is a reference to the parent instead of the child because the parent
  /// node id will not change since it's been resolved as having a peer dependency,
  /// but the child node could.
  ParentReference {
    /// This parent will be unique in the graph and never change, so we
    /// can hold a direct reference to it.
    parent: NodeParent,
    child_pkg_nv: NpmPackageNv,
  },
}

// todo(THIS PR): make this private
#[derive(Debug, Clone)]
pub struct ResolvedId {
  nv: NpmPackageNv,
  peer_dependencies: Vec<ResolvedIdPeerDep>,
}

#[derive(Debug, Default)]
struct ResolvedNodeIds {
  // cache of node identifiers that don't have peer dependencies
  no_peer_deps: HashMap<NpmPackageNv, NodeId>,
  to: HashMap<NodeId, ResolvedId>,
}

impl ResolvedNodeIds {
  pub fn set(&mut self, node_id: NodeId, resolved_id: ResolvedId) {
    if let Some(old_resolved_id) = self.to.insert(node_id, resolved_id.clone())
    {
      if old_resolved_id.peer_dependencies.is_empty() {
        self.no_peer_deps.remove(&old_resolved_id.nv);
      }
    }
    // todo(THIS PR): order here is important... add some unit tests
    if resolved_id.peer_dependencies.is_empty() {
      self.no_peer_deps.insert(resolved_id.nv, node_id);
    }
  }

  pub fn get_resolved_id(&self, node_id: &NodeId) -> Option<&ResolvedId> {
    self.to.get(node_id)
  }

  pub fn get_no_peer_deps_node_id(&self, id: &NpmPackageNv) -> Option<NodeId> {
    self.no_peer_deps.get(&id).copied()
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
  package_reqs: HashMap<NpmPackageReq, NpmPackageNv>,
  /// Then each name and version is mapped to an exact node id.
  root_packages: HashMap<NpmPackageNv, NodeId>,
  packages_by_name: HashMap<String, Vec<NodeId>>,
  packages: HashMap<NodeId, Node>,
  resolved_node_ids: ResolvedNodeIds,
  // This will be set when creating from a snapshot, then
  // inform the final snapshot creation.
  packages_to_copy_index: HashMap<NpmPackageResolvedId, usize>,
  /// Packages that the resolver should resolve first.
  pending_unresolved_packages: Vec<NpmPackageNv>,
}

impl Graph {
  pub fn from_snapshot(snapshot: NpmResolutionSnapshot) -> Self {
    fn get_or_create_graph_node(
      graph: &mut Graph,
      resolved_id: &NpmPackageResolvedId,
      packages: &HashMap<NpmPackageResolvedId, NpmResolutionPackage>,
      created_package_ids: &mut HashMap<NpmPackageResolvedId, NodeId>,
    ) -> NodeId {
      if let Some(id) = created_package_ids.get(resolved_id) {
        return *id; // already created
      }

      let node_id = graph.create_node(&resolved_id.nv);
      created_package_ids.insert(resolved_id.clone(), node_id);

      let peer_dep_ids = resolved_id
        .peer_dependencies
        .iter()
        .map(|peer_dep| {
          ResolvedIdPeerDep::SnapshotNodeId(get_or_create_graph_node(
            graph,
            peer_dep,
            packages,
            created_package_ids,
          ))
        })
        .collect::<Vec<_>>();
      let graph_resolved_id = ResolvedId {
        nv: resolved_id.nv.clone(),
        peer_dependencies: peer_dep_ids,
      };
      graph.resolved_node_ids.set(node_id, graph_resolved_id);
      let resolution = packages.get(&resolved_id).unwrap();
      for (name, child_id) in &resolution.dependencies {
        let child_node_id = get_or_create_graph_node(
          graph,
          child_id,
          packages,
          created_package_ids,
        );
        graph.set_child_parent_node(name, child_node_id, node_id);
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
    let mut created_package_ids =
      HashMap::with_capacity(snapshot.packages.len());
    for (id, resolved_id) in snapshot.root_packages {
      let node_id = get_or_create_graph_node(
        &mut graph,
        &resolved_id,
        &snapshot.packages,
        &mut created_package_ids,
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

  pub fn take_pending_unresolved(&mut self) -> Vec<NpmPackageNv> {
    std::mem::take(&mut self.pending_unresolved_packages)
  }

  pub fn has_root_package(&self, id: &NpmPackageNv) -> bool {
    self.root_packages.contains_key(id)
  }

  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.package_reqs.contains_key(req)
  }

  // todo (THIS PR): move this out so it can't be used in log messages
  pub fn get_npm_pkg_resolved_id(
    &self,
    node_id: NodeId,
  ) -> NpmPackageResolvedId {
    let resolved_id = self.resolved_node_ids.get_resolved_id(&node_id).unwrap();
    self.get_finalized_npm_pkg_resolved_id_from_resolved_id(
      resolved_id,
      HashSet::new(),
    )
  }

  fn get_finalized_npm_pkg_resolved_id_from_resolved_id(
    &self,
    resolved_id: &ResolvedId,
    seen: HashSet<NodeId>,
  ) -> NpmPackageResolvedId {
    let no_peer_deps_id = NpmPackageResolvedId {
      nv: resolved_id.nv.clone(),
      peer_dependencies: Vec::new(),
    };
    if resolved_id.peer_dependencies.is_empty() {
      no_peer_deps_id
    } else {
      let mut seen_children_resolved_ids =
        HashSet::with_capacity(resolved_id.peer_dependencies.len() + 1);

      // prevent a name showing up in the peer_dependencies list that matches the current name
      seen_children_resolved_ids.insert(no_peer_deps_id);

      NpmPackageResolvedId {
        nv: resolved_id.nv.clone(),
        peer_dependencies: resolved_id
          .peer_dependencies
          .iter()
          .filter_map(|peer_dep| {
            let (child_id, child_resolved_id) = match peer_dep {
              ResolvedIdPeerDep::SnapshotNodeId(node_id) => (
                node_id,
                self.resolved_node_ids.get_resolved_id(node_id).unwrap(),
              ),
              ResolvedIdPeerDep::ParentReference {
                parent,
                child_pkg_nv: child_nv,
              } => match &parent {
                NodeParent::Root(_) => {
                  let node_id = self.root_packages.get(&child_nv).unwrap();
                  (
                    node_id,
                    self.resolved_node_ids.get_resolved_id(&node_id).unwrap(),
                  )
                }
                NodeParent::Node(parent_id) => {
                  let parent = self.packages.get(&parent_id).unwrap();
                  parent
                    .children
                    .values()
                    .filter_map(|child_id| {
                      self
                        .resolved_node_ids
                        .get_resolved_id(child_id)
                        .map(|resolved_id| (child_id, resolved_id))
                    })
                    .filter(|(_, resolved_id)| resolved_id.nv == *child_nv)
                    .next()
                    .unwrap()
                }
              },
            };
            let mut new_seen = seen.clone();
            if new_seen.insert(*child_id) {
              let id = self.get_finalized_npm_pkg_resolved_id_from_resolved_id(
                child_resolved_id,
                new_seen.clone(),
              );
              if seen_children_resolved_ids.insert(id.clone()) {
                Some(id)
              } else {
                None
              }
            } else {
              None
            }
          })
          .collect(),
      }
    }
  }

  fn get_or_create_for_id(
    &mut self,
    resolved_id: &ResolvedId,
  ) -> (bool, NodeId) {
    // A node is reusable if it has no peer dependencies, but once
    // it has peer dependencies then we create a fresh node each time
    if resolved_id.peer_dependencies.is_empty() {
      if let Some(node_id) = self
        .resolved_node_ids
        .get_no_peer_deps_node_id(&resolved_id.nv)
      {
        return (false, node_id);
      }
    }

    let node_id = self.create_node(&resolved_id.nv);
    self.resolved_node_ids.set(node_id, resolved_id.clone());
    (true, node_id)
  }

  fn create_node(&mut self, pkg_nv: &NpmPackageNv) -> NodeId {
    let node_id = NodeId(self.next_package_id);
    self.next_package_id += 1;
    let node = Node {
      parents: Default::default(),
      children: Default::default(),
      no_peers: false,
    };

    self
      .packages_by_name
      .entry(pkg_nv.name.clone())
      .or_default()
      .push(node_id);
    self.packages.insert(node_id, node);
    node_id
  }

  fn borrow_node(&mut self, node_id: &NodeId) -> &mut Node {
    self.packages.get_mut(node_id).unwrap()
  }

  // todo(this pr): standardize to NodeId instead of reference
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

  pub async fn into_snapshot(
    mut self,
    api: &NpmRegistryApi,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    let packages_to_resolved_id = self
      .packages
      .keys()
      .map(|node_id| (*node_id, self.get_npm_pkg_resolved_id(*node_id)))
      .collect::<HashMap<_, _>>();
    let resolved_packages_by_name = self
      .packages_by_name
      .into_iter()
      .map(|(name, packages)| {
        (name, {
          let mut packages = packages
            .into_iter()
            .map(|node_id| {
              (
                node_id,
                packages_to_resolved_id.get(&node_id).unwrap().clone(),
              )
            })
            .collect::<Vec<_>>();
          // sort in order to get a deterministic copy index
          packages.sort_by(|a, b| a.1.cmp(&b.1));
          // now that we're sorted, filter out duplicate graph segment resolutions
          packages.dedup_by(|a, b| a.1 == b.1);
          packages
        })
      })
      .collect::<HashMap<_, _>>();

    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::from_map_with_capacity(
        self.packages_to_copy_index,
        self.packages.len(),
      );
    let mut packages = HashMap::with_capacity(self.packages.len());
    for (_, ids) in &resolved_packages_by_name {
      for (node_id, resolved_id) in ids {
        let node = self.packages.remove(node_id).unwrap();
        let dist = api
          .package_version_info(&resolved_id.nv)
          .await?
          .unwrap()
          .dist;
        packages.insert(
          (*resolved_id).clone(),
          NpmResolutionPackage {
            copy_index: copy_index_resolver.resolve(resolved_id),
            pkg_id: (*resolved_id).clone(),
            dist,
            dependencies: node
              .children
              .into_iter()
              .map(|(key, value)| {
                (key, packages_to_resolved_id.get(&value).unwrap().clone())
              })
              .collect(),
          },
        );
      }
    }

    debug_assert!(self.pending_unresolved_packages.is_empty());

    Ok(NpmResolutionSnapshot {
      root_packages: self
        .root_packages
        .into_iter()
        .map(|(id, node_id)| {
          (
            id.clone(),
            packages_to_resolved_id.get(&node_id).unwrap().clone(),
          )
        })
        .collect(),
      packages_by_name: resolved_packages_by_name
        .into_iter()
        .map(|(name, ids)| (name, ids.into_iter().map(|(_, id)| id).collect()))
        .collect(),
      packages,
      package_reqs: self.package_reqs,
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
            self.get_npm_pkg_resolved_id(*node_id).as_serialized()
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
      self.get_npm_pkg_resolved_id(node_id).as_serialized()
    );
  }
}

#[derive(Default)]
struct DepEntryCache(HashMap<NpmPackageNv, Arc<Vec<NpmDependencyEntry>>>);

impl DepEntryCache {
  pub fn store(
    &mut self,
    nv: NpmPackageNv,
    version_info: &NpmPackageVersionInfo,
  ) -> Result<Arc<Vec<NpmDependencyEntry>>, AnyError> {
    debug_assert!(!self.0.contains_key(&nv)); // we should not be re-inserting
    let mut deps = version_info
      .dependencies_as_entries()
      .with_context(|| format!("npm package: {}", nv))?;
    // Ensure name alphabetical and then version descending
    // so these are resolved in that order
    deps.sort();
    let deps = Arc::new(deps);
    self.0.insert(nv, deps.clone());
    Ok(deps)
  }

  pub fn get(
    &self,
    id: &NpmPackageNv,
  ) -> Option<&Arc<Vec<NpmDependencyEntry>>> {
    self.0.get(id)
  }
}

struct UnresolvedOptionalPeer {
  specifier: String,
  graph_path: Arc<GraphPath>,
}

pub struct GraphDependencyResolver<'a> {
  graph: &'a mut Graph,
  api: &'a NpmRegistryApi,
  pending_unresolved_nodes: VecDeque<Arc<GraphPath>>,
  // todo(THIS PR): consider this, but probably not
  optional_peer_versions: HashMap<NpmPackageNv, NpmPackageNv>,
  unresolved_optional_peers: HashMap<NpmPackageNv, Vec<UnresolvedOptionalPeer>>,
  dep_entry_cache: DepEntryCache,
}

impl<'a> GraphDependencyResolver<'a> {
  pub fn new(graph: &'a mut Graph, api: &'a NpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_unresolved_nodes: Default::default(),
      optional_peer_versions: Default::default(),
      unresolved_optional_peers: Default::default(),
      dep_entry_cache: Default::default(),
    }
  }

  pub fn add_root_package(
    &mut self,
    package_id: &NpmPackageNv,
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
      .set_child_parent("", node_id, &NodeParent::Root(pkg_id.clone()));
    self
      .pending_unresolved_nodes
      .push_back(GraphPath::for_root(NodeIdRef::new(node_id), pkg_id));
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
      .insert(package_req.clone(), pkg_id.clone());
    self
      .graph
      .set_child_parent("", node_id, &NodeParent::Root(pkg_id.clone()));
    self
      .pending_unresolved_nodes
      .push_back(GraphPath::for_root(NodeIdRef::new(node_id), pkg_id));
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
      self.try_add_pending_unresolved_node(
        visited_versions,
        NodeIdRef::new(node_id),
      );
    }
    Ok(node_id)
  }

  fn try_add_pending_unresolved_node(
    &mut self,
    path: &Arc<GraphPath>,
    node_id_ref: NodeIdRef,
  ) {
    if self
      .graph
      .packages
      .get(&node_id_ref.get())
      .unwrap()
      .no_peers
    {
      return; // skip, no need to analyze this again
    }
    let visited_versions = match path.with_id(node_id_ref) {
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
  ) -> Result<(NpmPackageNv, NodeId), AnyError> {
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
            .nv
            .version
        }),
    )?;
    let resolved_id = ResolvedId {
      nv: NpmPackageNv {
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
          .get_npm_pkg_resolved_id(parent_id)
          .as_serialized(),
        None => "<package-req>".to_string(),
      },
      pkg_req_name,
      version_req.version_text(),
      resolved_id.nv.to_string(),
    );
    let (_, node_id) = self.graph.get_or_create_for_id(&resolved_id);
    let pkg_id = resolved_id.nv;

    let has_deps = if let Some(deps) = self.dep_entry_cache.get(&pkg_id) {
      !deps.is_empty()
    } else {
      let deps = self
        .dep_entry_cache
        .store(pkg_id.clone(), &version_and_info.info)?;
      !deps.is_empty()
    };

    if !has_deps {
      // ensure this is set if not, as its an optimization
      let mut node = self.graph.borrow_node_mut(&node_id);
      node.no_peers = true;
    }

    Ok((pkg_id, node_id))
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some(graph_path) = self.pending_unresolved_nodes.pop_front() {
        let (pkg_id, deps) = {
          let node_id = graph_path.node_id();
          match self.graph.packages.get(&node_id) {
            Some(node) if node.no_peers => {
              continue; // skip, no need to analyze
            }
            Some(_) => {}
            None => {
              // todo(dsherret): I don't believe this should occur anymore
              // todo(THIS PR): add a debug assert
              continue;
            }
          };

          let pkg_id = self
            .graph
            .resolved_node_ids
            .get_resolved_id(&node_id)
            .unwrap()
            .nv
            .clone();
          let deps = if let Some(deps) = self.dep_entry_cache.get(&pkg_id) {
            deps.clone()
          } else {
            // the api should have this in the cache at this point, so no need to parallelize
            match self.api.package_version_info(&pkg_id).await? {
              Some(version_info) => {
                self.dep_entry_cache.store(pkg_id.clone(), &version_info)?
              }
              None => {
                bail!("Could not find version information for {}", pkg_id)
              }
            }
          };

          (pkg_id, deps)
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
              let maybe_child_id = self
                .graph
                .packages
                .get(&graph_path.node_id())
                .unwrap()
                .children
                .get(&dep.bare_specifier)
                .copied();
              let child_id = if let Some(child_id) = maybe_child_id {
                // we already resolved this dependency before, and we can skip over it
                self.try_add_pending_unresolved_node(
                  &graph_path,
                  NodeIdRef::new(child_id.clone()),
                );
                child_id
              } else {
                self.analyze_dependency(dep, &package_info, &graph_path)?
              };

              if !found_peer {
                found_peer = !self.graph.borrow_node_mut(&child_id).no_peers;
              }
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              found_peer = true;
              // we need to re-evaluate peer dependencies every time and can't
              // skip over them because they might be evaluated differently
              let maybe_new_id = self.resolve_peer_dep(
                &dep.bare_specifier,
                dep,
                &package_info,
                &graph_path,
              )?;

              // For optional dependencies, we want to resolve them if any future
              // same version resolves them, so when not resolve, store them to be
              // potentially resolved later and if resolved, drain the previous ones.
              //
              // Note: This is not a good solution, but will probably work ok in most
              // scenarios. We can work on improving this in the future.
              if dep.kind == NpmDependencyEntryKind::OptionalPeer {
                match maybe_new_id {
                  Some(new_id) => {
                    if let Some(unresolved_optional_peers) =
                      self.unresolved_optional_peers.remove(&pkg_id)
                    {
                      for optional_peer in unresolved_optional_peers {
                        self.set_new_peer_dep(
                          NodeParent::Node(optional_peer.graph_path.node_id()),
                          vec![&optional_peer.graph_path],
                          &optional_peer.specifier,
                          new_id,
                        );
                      }
                    }
                  }
                  None => {
                    // store this for later if it's resolved for this version
                    self
                      .unresolved_optional_peers
                      .entry(pkg_id.clone())
                      .or_default()
                      .push(UnresolvedOptionalPeer {
                        specifier: dep.bare_specifier.clone(),
                        graph_path: graph_path.clone(),
                      });
                  }
                }
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
  ) -> Result<Option<NodeId>, AnyError> {
    debug_assert!(matches!(
      peer_dep.kind,
      NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer
    ));

    // use this to detect cycles... this is just in case and probably won't happen
    let mut up_path = GraphPath::new(ancestor_path.node_id_ref().clone());
    let mut path = vec![ancestor_path];

    // todo(THIS PR): add a test for this
    // the current dependency might have had the peer dependency
    // in another bare specifier slot... if so resolve it to that
    {
      let maybe_peer_dep = self.find_peer_dep_in_node(
        ancestor_path,
        peer_dep,
        peer_package_info,
      )?;

      if let Some((peer_parent, peer_dep_id)) = maybe_peer_dep {
        //self.try_add_pending_unresolved_node(ancestor_path, peer_dep_id);
        // this will always have an ancestor because we're not at the root
        self.set_new_peer_dep(peer_parent, path, specifier, peer_dep_id);
        return Ok(Some(peer_dep_id));
      }
    }

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut ancestor_iterator = ancestor_path.ancestors().peekable();
    while let Some(ancestor_node) = ancestor_iterator.next() {
      match ancestor_node {
        GraphPathNodeOrRoot::Node(ancestor_graph_path_node) => {
          path.push(ancestor_graph_path_node);
          let maybe_peer_dep = self.find_peer_dep_in_node(
            ancestor_graph_path_node,
            peer_dep,
            peer_package_info,
          )?;
          if let Some((parent, peer_dep_id)) = maybe_peer_dep {
            //self.try_add_pending_unresolved_node(ancestor_path, peer_dep_id);
            // this will always have an ancestor because we're not at the root
            self.set_new_peer_dep(parent, path, specifier, peer_dep_id);
            return Ok(Some(peer_dep_id));
          }

          up_path = match up_path
            .with_id(ancestor_graph_path_node.node_id_ref().clone())
          {
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
            // if peer_dep.kind.is_optional() {
            //   self.set_previously_unresolved_optional_dependency(
            //     child_id,
            //     peer_dep,
            //     ancestor_path,
            //   );
            //   return Ok(());
            // }

            //self.try_add_pending_unresolved_node(ancestor_path, child_id);
            self.set_new_peer_dep(
              NodeParent::Root(root_pkg_id.clone()),
              path,
              specifier,
              child_id,
            );
            return Ok(Some(child_id));
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
      let peer_parent = NodeParent::Node(ancestor_path.node_id());
      self.set_new_peer_dep(
        peer_parent,
        vec![ancestor_path],
        specifier,
        node_id,
      );
      Ok(Some(node_id))
    } else {
      Ok(None)
    }
  }

  fn find_peer_dep_in_node(
    &self,
    path: &Arc<GraphPath>,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: &NpmPackageInfo,
  ) -> Result<Option<(NodeParent, NodeId)>, AnyError> {
    let node_id = path.node_id();
    let resolved_node_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&node_id)
      .unwrap();
    // check if this node itself is a match for
    // the peer dependency and if so use that
    if resolved_node_id.nv.name == peer_dep.name
      && version_req_satisfies(
        &peer_dep.version_req,
        &resolved_node_id.nv.version,
        peer_package_info,
        None,
      )?
    {
      let parent = match path.previous_node.as_ref().unwrap() {
        GraphPathNodeOrRoot::Node(node) => NodeParent::Node(node.node_id()),
        GraphPathNodeOrRoot::Root(pkg_id) => NodeParent::Root(pkg_id.clone()),
      };
      Ok(Some((parent, node_id)))
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
                .nv,
            )
          }),
      )
      .map(|maybe_child_id| {
        maybe_child_id.map(|child_id| (NodeParent::Node(node_id), child_id))
      })
    }
  }

  fn set_new_peer_dep(
    &mut self,
    peer_dep_parent: NodeParent,
    // path from the node above the resolved dep to just above the peer dep
    path: Vec<&Arc<GraphPath>>,
    peer_dep_specifier: &str,
    peer_dep_id: NodeId,
  ) {
    debug_assert!(!path.is_empty());
    let peer_dep_pkg_id = self
      .graph
      .resolved_node_ids
      .get_resolved_id(&peer_dep_id)
      .unwrap()
      .nv
      .clone();

    let mut peer_dep = match &peer_dep_parent {
      NodeParent::Root(id) => Some(ResolvedIdPeerDep::ParentReference {
        parent: NodeParent::Root(id.clone()),
        child_pkg_nv: peer_dep_pkg_id.clone(),
      }),
      // we need to create it later once we have a new node id
      _ => None,
    };
    let mut added_to_pending_nodes = false;
    let mut node_parent =
      match path.last().unwrap().previous_node.as_ref().unwrap() {
        GraphPathNodeOrRoot::Node(path) => NodeParent::Node(path.node_id()),
        GraphPathNodeOrRoot::Root(pkg_id) => NodeParent::Root(pkg_id.clone()),
      };

    for graph_path_node in path.iter().rev() {
      let node_id = graph_path_node.node_id();
      let old_resolved_id = self
        .graph
        .resolved_node_ids
        .get_resolved_id(&node_id)
        .unwrap()
        .clone();

      let current_node = self.graph.packages.get(&node_id).unwrap();
      if current_node.has_one_parent() {
        // In this case, we can take control of this node identifier and
        // update the current node in place. We only need to update the
        // collection of resolved node identifiers.

        // todo(THIS PR): extract out?
        let peer_dep = match peer_dep.as_ref() {
          Some(peer_dep) => peer_dep.clone(),
          None => {
            // use the current node id since we're not changing it
            let new_peer_dep = ResolvedIdPeerDep::ParentReference {
              parent: NodeParent::Node(node_id),
              child_pkg_nv: peer_dep_pkg_id.clone(),
            };
            debug_assert_ne!(peer_dep_pkg_id, old_resolved_id.nv);
            peer_dep = Some(new_peer_dep.clone());
            new_peer_dep
          }
        };
        let mut new_resolved_id = old_resolved_id.clone();
        new_resolved_id.peer_dependencies.push(peer_dep.clone());
        self.graph.resolved_node_ids.set(node_id, new_resolved_id);

        node_parent = NodeParent::Node(node_id);
      } else {
        let old_node_id = node_id;
        let new_node_id = self.graph.create_node(&old_resolved_id.nv);

        // update the resolved id
        let peer_dep = match peer_dep.as_ref() {
          Some(peer_dep) => peer_dep.clone(),
          None => {
            let new_peer_dep = ResolvedIdPeerDep::ParentReference {
              parent: NodeParent::Node(new_node_id),
              child_pkg_nv: peer_dep_pkg_id.clone(),
            };
            debug_assert_ne!(peer_dep_pkg_id, old_resolved_id.nv);
            peer_dep = Some(new_peer_dep.clone());
            new_peer_dep
          }
        };
        let mut new_resolved_id = old_resolved_id.clone();
        new_resolved_id.peer_dependencies.push(peer_dep.clone());
        self
          .graph
          .resolved_node_ids
          .set(new_node_id, new_resolved_id);

        debug_assert_eq!(graph_path_node.node_id(), old_node_id);
        graph_path_node.change_id(new_node_id);

        if !added_to_pending_nodes {
          // add the top node changed to the list of pending unresolved nodes
          self
            .pending_unresolved_nodes
            .push_back((**graph_path_node).clone());
          added_to_pending_nodes = true;
        }

        // update the current node to not have the parent
        {
          let node = self.graph.borrow_node_mut(&old_node_id);
          node.remove_parent(&node_parent);
        }

        // update the parent to point to this new node
        match &node_parent {
          NodeParent::Root(pkg_id) => {
            self.graph.root_packages.insert(pkg_id.clone(), new_node_id);
          }
          NodeParent::Node(parent_node_id) => {
            let parent_node = self.graph.borrow_node_mut(parent_node_id);
            let mut found_match = false;
            for child_id in parent_node.children.values_mut() {
              if *child_id == old_node_id {
                *child_id = new_node_id;
                found_match = true;
              }
            }
            debug_assert!(found_match);
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
      NodeParent::Node(parent_node_id) => {
        self.graph.set_child_parent_node(
          peer_dep_specifier,
          peer_dep_id,
          parent_node_id,
        );

        // add the peer dependency to be analyzed when none of its ancestors will be
        if !added_to_pending_nodes {
          let bottom_node = path.first().unwrap();
          debug_assert_eq!(bottom_node.node_id(), parent_node_id);
          self.try_add_pending_unresolved_node(
            bottom_node,
            NodeIdRef::new(peer_dep_id),
          );
        }

        debug!(
          "Resolved peer dependency for {} in {} to {}",
          peer_dep_specifier,
          &self
            .graph
            .get_npm_pkg_resolved_id(parent_node_id)
            .as_serialized(),
          &self
            .graph
            .get_npm_pkg_resolved_id(peer_dep_id)
            .as_serialized(),
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
  children: impl Iterator<Item = (NodeId, &'a NpmPackageNv)>,
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
              NpmPackageResolvedId::from_serialized(
                "package-b@2.0.0_package-peer@4.1.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@3.0.0_package-peer@4.1.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@2.0.0_package-peer@4.1.0"
          )
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-c@3.0.0_package-peer@4.1.0"
          )
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-b@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
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
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-peer@1.0.0".to_string()
        ),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-peer@1.0.0".to_string()
        )
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@2.0.0"
          )
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-a@1.0.0_package-peer@2.0.0"
              )
              .unwrap(),
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
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-peer@2.0.0".to_string()
        ),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-peer@2.0.0".to_string()
        )
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
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
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-peer@1.0.0".to_string()
        ),
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
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
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-peer@1.0.0".to_string()
        ),
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-peer-a@2.0.0_package-peer-b@3.0.0"
            )
            .unwrap(),
          )]),
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
      vec![(
        "package-0@1.0".to_string(),
        "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
          .to_string()
      )]
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
            "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0"
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
          "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0"
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
              "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
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
            "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
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
            "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
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
          pkg_id: NpmPackageResolvedId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
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
            "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0"
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
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
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
            "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageResolvedId::from_serialized(
              "package-b@1.0.0_package-peer@1.0.0"
            )
            .unwrap(),
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
          "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0".to_string()
        ),
        (
          "package-b@1.0".to_string(),
          "package-b@1.0.0_package-peer@1.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_then_other_dep_with_different_peer() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.1.0");
    api.ensure_package_version("package-peer", "1.2.0");
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "*")); // should select 1.2.0, then 1.1.0
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "=1.1.0"));
    api.add_dependency(("package-c", "1.0.0"), ("package-a", "1"));

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
            "package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.1.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-a@1.0.0_package-peer@1.2.0"
          )
          .unwrap(),
          copy_index: 1,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageResolvedId::from_serialized("package-peer@1.2.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageResolvedId::from_serialized(
            "package-b@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageResolvedId::from_serialized(
                "package-c@1.0.0_package-peer@1.1.0"
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
            "package-c@1.0.0_package-peer@1.1.0"
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
        (
          "package-a@1.0".to_string(),
          "package-a@1.0.0_package-peer@1.2.0".to_string()
        ),
        (
          "package-b@1.0".to_string(),
          "package-b@1.0.0_package-peer@1.1.0".to_string()
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

    {
      let new_snapshot = Graph::from_snapshot(snapshot.clone())
        .into_snapshot(&api)
        .await
        .unwrap();
      assert_eq!(
        snapshot, new_snapshot,
        "recreated snapshot should be the same"
      );
    }

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
