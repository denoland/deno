// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_graph::npm::NpmPackageNv;
use deno_graph::npm::NpmPackageReq;
use deno_graph::semver::VersionReq;
use log::debug;

use crate::npm::registry::NpmDependencyEntry;
use crate::npm::registry::NpmDependencyEntryKind;
use crate::npm::registry::NpmPackageInfo;
use crate::npm::registry::NpmPackageVersionInfo;
use crate::npm::resolution::common::resolve_best_package_version_and_info;
use crate::npm::resolution::snapshot::SnapshotPackageCopyIndexResolver;
use crate::npm::NpmRegistryApi;

use super::common::version_req_satisfies;
use super::common::LATEST_VERSION_REQ;
use super::snapshot::NpmResolutionSnapshot;
use super::NpmPackageId;
use super::NpmResolutionPackage;

/// A unique identifier to a node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
struct NodeId(u32);

/// A resolved package in the resolution graph.
#[derive(Debug)]
struct Node {
  /// The specifier to child relationship in the graph. The specifier is
  /// the key in an npm package's dependencies map (ex. "express"). We
  /// use a BTreeMap for some determinism when creating the snapshot.
  ///
  /// Note: We don't want to store the children as a `NodeRef` because
  /// multiple paths might visit through the children and we don't want
  /// to share those references with those paths.
  pub children: BTreeMap<String, NodeId>,
  /// Whether the node has demonstrated to have no peer dependencies in its
  /// descendants. If this is true then we can skip analyzing this node
  /// again when we encounter it another time in the dependency tree, which
  /// is much faster.
  pub no_peers: bool,
}

#[derive(Clone)]
enum ResolvedIdPeerDep {
  /// This is a reference to the parent instead of the child because we only have a
  /// node reference to the parent, since we've traversed it, but the child node may
  /// change from under it.
  ParentReference {
    parent: GraphPathNodeOrRoot,
    child_pkg_nv: NpmPackageNv,
  },
  /// A node that was created during snapshotting and is not being used in any path.
  SnapshotNodeId(NodeId),
}

impl ResolvedIdPeerDep {
  pub fn current_state_hash(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    self.current_state_hash_with_hasher(&mut hasher);
    hasher.finish()
  }

  pub fn current_state_hash_with_hasher(&self, hasher: &mut DefaultHasher) {
    match self {
      ResolvedIdPeerDep::ParentReference {
        parent,
        child_pkg_nv,
      } => {
        match parent {
          GraphPathNodeOrRoot::Root(root) => root.hash(hasher),
          GraphPathNodeOrRoot::Node(node) => node.node_id().hash(hasher),
        }
        child_pkg_nv.hash(hasher);
      }
      ResolvedIdPeerDep::SnapshotNodeId(node_id) => {
        node_id.hash(hasher);
      }
    }
  }
}

/// A pending resolved identifier used in the graph. At the end of resolution, these
/// will become fully resolved to an `NpmPackageId`.
#[derive(Clone)]
struct ResolvedId {
  nv: NpmPackageNv,
  peer_dependencies: Vec<ResolvedIdPeerDep>,
}

impl ResolvedId {
  /// Gets a hash of the resolved identifier at this current moment in time.
  ///
  /// WARNING: A resolved identifier references a value that could change in
  /// the future, so this should be used with that in mind.
  pub fn current_state_hash(&self) -> u64 {
    let mut hasher = DefaultHasher::new();
    self.nv.hash(&mut hasher);
    for dep in &self.peer_dependencies {
      dep.current_state_hash_with_hasher(&mut hasher);
    }
    hasher.finish()
  }

  pub fn push_peer_dep(&mut self, peer_dep: ResolvedIdPeerDep) {
    let new_hash = peer_dep.current_state_hash();
    for dep in &self.peer_dependencies {
      if new_hash == dep.current_state_hash() {
        return;
      }
    }
    self.peer_dependencies.push(peer_dep);
  }
}

/// Mappings of node identifiers to resolved identifiers. Each node has exactly
/// one resolved identifier.
#[derive(Default)]
struct ResolvedNodeIds {
  node_to_resolved_id: HashMap<NodeId, (ResolvedId, u64)>,
  resolved_to_node_id: HashMap<u64, NodeId>,
}

impl ResolvedNodeIds {
  pub fn set(&mut self, node_id: NodeId, resolved_id: ResolvedId) {
    let resolved_id_hash = resolved_id.current_state_hash();
    if let Some((_, old_resolved_id_key)) = self
      .node_to_resolved_id
      .insert(node_id, (resolved_id, resolved_id_hash))
    {
      // ensure the old resolved id key is removed as it might be stale
      self.resolved_to_node_id.remove(&old_resolved_id_key);
    }
    self.resolved_to_node_id.insert(resolved_id_hash, node_id);
  }

  pub fn get(&self, node_id: NodeId) -> Option<&ResolvedId> {
    self.node_to_resolved_id.get(&node_id).map(|(id, _)| id)
  }

  pub fn get_node_id(&self, resolved_id: &ResolvedId) -> Option<NodeId> {
    self
      .resolved_to_node_id
      .get(&resolved_id.current_state_hash())
      .copied()
  }
}

/// A pointer to a specific node in a graph path. The underlying node id
/// may change as peer dependencies are created.
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

#[derive(Clone)]
enum GraphPathNodeOrRoot {
  Node(Arc<GraphPath>),
  Root(NpmPackageNv),
}

/// Path through the graph that represents a traversal through the graph doing
/// the dependency resolution. The graph tries to share duplicate package
/// information and we try to avoid traversing parts of the graph that we know
/// are resolved.
#[derive(Clone)]
struct GraphPath {
  previous_node: Option<GraphPathNodeOrRoot>,
  node_id_ref: NodeIdRef,
  // todo(dsherret): I think we might be able to get rid of specifier and
  // node version here, but I added them for extra protection for the time being.
  specifier: String,
  nv: NpmPackageNv,
}

impl GraphPath {
  pub fn for_root(node_id: NodeId, nv: NpmPackageNv) -> Arc<Self> {
    Arc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Root(nv.clone())),
      node_id_ref: NodeIdRef::new(node_id),
      // use an empty specifier
      specifier: "".to_string(),
      nv,
    })
  }

  pub fn node_id(&self) -> NodeId {
    self.node_id_ref.get()
  }

  pub fn specifier(&self) -> &str {
    &self.specifier
  }

  pub fn change_id(&self, node_id: NodeId) {
    self.node_id_ref.change(node_id)
  }

  pub fn with_id(
    self: &Arc<GraphPath>,
    node_id: NodeId,
    specifier: &str,
    nv: NpmPackageNv,
  ) -> Option<Arc<Self>> {
    if self.has_visited(&nv) {
      None
    } else {
      Some(Arc::new(Self {
        previous_node: Some(GraphPathNodeOrRoot::Node(self.clone())),
        node_id_ref: NodeIdRef::new(node_id),
        specifier: specifier.to_string(),
        nv,
      }))
    }
  }

  /// Each time an identifier is added, we do a check to ensure
  /// that we haven't previously visited this node. I suspect this
  /// might be a little slow since it has to go up through the ancestors,
  /// so some optimizations could be made here in the future.
  pub fn has_visited(self: &Arc<Self>, nv: &NpmPackageNv) -> bool {
    if self.nv == *nv {
      return true;
    }
    let mut maybe_next_node = self.previous_node.as_ref();
    while let Some(GraphPathNodeOrRoot::Node(next_node)) = maybe_next_node {
      // we've visited this before, so stop
      if next_node.nv == *nv {
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

#[derive(Default)]
pub struct Graph {
  /// Each requirement is mapped to a specific name and version.
  package_reqs: HashMap<NpmPackageReq, NpmPackageNv>,
  /// Then each name and version is mapped to an exact node id.
  /// Note: Uses a BTreeMap in order to create some determinism
  /// when creating the snapshot.
  root_packages: BTreeMap<NpmPackageNv, NodeId>,
  nodes_by_package_name: HashMap<String, Vec<NodeId>>,
  nodes: HashMap<NodeId, Node>,
  resolved_node_ids: ResolvedNodeIds,
  // This will be set when creating from a snapshot, then
  // inform the final snapshot creation.
  packages_to_copy_index: HashMap<NpmPackageId, usize>,
}

impl Graph {
  pub fn from_snapshot(
    snapshot: NpmResolutionSnapshot,
  ) -> Result<Self, AnyError> {
    fn get_or_create_graph_node(
      graph: &mut Graph,
      resolved_id: &NpmPackageId,
      packages: &HashMap<NpmPackageId, NpmResolutionPackage>,
      created_package_ids: &mut HashMap<NpmPackageId, NodeId>,
    ) -> Result<NodeId, AnyError> {
      if let Some(id) = created_package_ids.get(resolved_id) {
        return Ok(*id);
      }

      let node_id = graph.create_node(&resolved_id.nv);
      created_package_ids.insert(resolved_id.clone(), node_id);

      let peer_dep_ids = resolved_id
        .peer_dependencies
        .iter()
        .map(|peer_dep| {
          Ok(ResolvedIdPeerDep::SnapshotNodeId(get_or_create_graph_node(
            graph,
            peer_dep,
            packages,
            created_package_ids,
          )?))
        })
        .collect::<Result<Vec<_>, AnyError>>()?;
      let graph_resolved_id = ResolvedId {
        nv: resolved_id.nv.clone(),
        peer_dependencies: peer_dep_ids,
      };
      graph.resolved_node_ids.set(node_id, graph_resolved_id);
      let resolution = match packages.get(resolved_id) {
        Some(resolved_id) => resolved_id,
        // maybe the user messed around with the lockfile
        None => bail!("not found package: {}", resolved_id.as_serialized()),
      };
      for (name, child_id) in &resolution.dependencies {
        let child_node_id = get_or_create_graph_node(
          graph,
          child_id,
          packages,
          created_package_ids,
        )?;
        graph.set_child_parent_node(name, child_node_id, node_id);
      }
      Ok(node_id)
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
      )?;
      graph.root_packages.insert(id, node_id);
    }
    Ok(graph)
  }

  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.package_reqs.contains_key(req)
  }

  fn get_npm_pkg_id(&self, node_id: NodeId) -> NpmPackageId {
    let resolved_id = self.resolved_node_ids.get(node_id).unwrap();
    self.get_npm_pkg_id_from_resolved_id(resolved_id, HashSet::new())
  }

  fn get_npm_pkg_id_from_resolved_id(
    &self,
    resolved_id: &ResolvedId,
    seen: HashSet<NodeId>,
  ) -> NpmPackageId {
    if resolved_id.peer_dependencies.is_empty() {
      NpmPackageId {
        nv: resolved_id.nv.clone(),
        peer_dependencies: Vec::new(),
      }
    } else {
      let mut npm_pkg_id = NpmPackageId {
        nv: resolved_id.nv.clone(),
        peer_dependencies: Vec::with_capacity(
          resolved_id.peer_dependencies.len(),
        ),
      };
      let mut seen_children_resolved_ids =
        HashSet::with_capacity(resolved_id.peer_dependencies.len());
      for peer_dep in &resolved_id.peer_dependencies {
        let maybe_node_and_resolved_id = match peer_dep {
          ResolvedIdPeerDep::SnapshotNodeId(node_id) => self
            .resolved_node_ids
            .get(*node_id)
            .map(|resolved_id| (*node_id, resolved_id)),
          ResolvedIdPeerDep::ParentReference {
            parent,
            child_pkg_nv: child_nv,
          } => match &parent {
            GraphPathNodeOrRoot::Root(_) => {
              self.root_packages.get(child_nv).and_then(|node_id| {
                self
                  .resolved_node_ids
                  .get(*node_id)
                  .map(|resolved_id| (*node_id, resolved_id))
              })
            }
            GraphPathNodeOrRoot::Node(parent_path) => {
              self.nodes.get(&parent_path.node_id()).and_then(|parent| {
                parent
                  .children
                  .values()
                  .filter_map(|child_id| {
                    let child_id = *child_id;
                    self
                      .resolved_node_ids
                      .get(child_id)
                      .map(|resolved_id| (child_id, resolved_id))
                  })
                  .find(|(_, resolved_id)| resolved_id.nv == *child_nv)
              })
            }
          },
        };
        // this should always be set
        debug_assert!(maybe_node_and_resolved_id.is_some());
        if let Some((child_id, child_resolved_id)) = maybe_node_and_resolved_id
        {
          let mut new_seen = seen.clone();
          if new_seen.insert(child_id) {
            let child_peer = self.get_npm_pkg_id_from_resolved_id(
              child_resolved_id,
              new_seen.clone(),
            );

            // This condition prevents a name showing up in the peer_dependencies
            // list that matches the current name. Checking just the name and
            // version should be sufficient because the rest of the peer dependency
            // resolutions should be the same
            let is_pkg_same = child_peer.nv == npm_pkg_id.nv;
            if !is_pkg_same
              && seen_children_resolved_ids.insert(child_peer.clone())
            {
              npm_pkg_id.peer_dependencies.push(child_peer);
            }
          }
        }
      }
      npm_pkg_id
    }
  }

  fn get_or_create_for_id(
    &mut self,
    resolved_id: &ResolvedId,
  ) -> (bool, NodeId) {
    if let Some(node_id) = self.resolved_node_ids.get_node_id(resolved_id) {
      return (false, node_id);
    }

    let node_id = self.create_node(&resolved_id.nv);
    self.resolved_node_ids.set(node_id, resolved_id.clone());
    (true, node_id)
  }

  fn create_node(&mut self, pkg_nv: &NpmPackageNv) -> NodeId {
    let node_id = NodeId(self.nodes.len() as u32);
    let node = Node {
      children: Default::default(),
      no_peers: false,
    };

    self
      .nodes_by_package_name
      .entry(pkg_nv.name.clone())
      .or_default()
      .push(node_id);
    self.nodes.insert(node_id, node);

    node_id
  }

  fn borrow_node_mut(&mut self, node_id: NodeId) -> &mut Node {
    self.nodes.get_mut(&node_id).unwrap()
  }

  fn set_child_parent_node(
    &mut self,
    specifier: &str,
    child_id: NodeId,
    parent_id: NodeId,
  ) {
    assert_ne!(child_id, parent_id);
    let parent = self.borrow_node_mut(parent_id);
    parent.children.insert(specifier.to_string(), child_id);
  }

  pub async fn into_snapshot(
    self,
    api: &NpmRegistryApi,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    let packages_to_resolved_id = self
      .nodes
      .keys()
      .map(|node_id| (*node_id, self.get_npm_pkg_id(*node_id)))
      .collect::<HashMap<_, _>>();
    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::from_map_with_capacity(
        self.packages_to_copy_index,
        self.nodes.len(),
      );
    let mut packages = HashMap::with_capacity(self.nodes.len());
    let mut traversed_node_ids = HashSet::with_capacity(self.nodes.len());
    let mut pending = VecDeque::new();
    for root_id in self.root_packages.values() {
      if traversed_node_ids.insert(*root_id) {
        pending.push_back(*root_id);
      }
    }
    while let Some(node_id) = pending.pop_front() {
      let node = self.nodes.get(&node_id).unwrap();
      let resolved_id = packages_to_resolved_id.get(&node_id).unwrap();
      // todo(dsherret): grab this from the dep entry cache, which should have it
      let dist = api
        .package_version_info(&resolved_id.nv)
        .await?
        .unwrap_or_else(|| panic!("missing: {:?}", resolved_id.nv))
        .dist;
      packages.insert(
        (*resolved_id).clone(),
        NpmResolutionPackage {
          copy_index: copy_index_resolver.resolve(resolved_id),
          pkg_id: (*resolved_id).clone(),
          dist,
          dependencies: node
            .children
            .iter()
            .map(|(key, value)| {
              (
                key.clone(),
                packages_to_resolved_id
                  .get(value)
                  .unwrap_or_else(|| {
                    panic!("{node_id:?} -- missing child: {value:?}")
                  })
                  .clone(),
              )
            })
            .collect(),
        },
      );
      for child_id in node.children.values() {
        if traversed_node_ids.insert(*child_id) {
          pending.push_back(*child_id);
        }
      }
    }

    Ok(NpmResolutionSnapshot {
      root_packages: self
        .root_packages
        .into_iter()
        .map(|(id, node_id)| {
          (id, packages_to_resolved_id.get(&node_id).unwrap().clone())
        })
        .collect(),
      packages_by_name: self
        .nodes_by_package_name
        .into_iter()
        .map(|(name, ids)| {
          let mut ids = ids
            .into_iter()
            .filter(|id| traversed_node_ids.contains(id))
            .map(|id| packages_to_resolved_id.get(&id).unwrap().clone())
            .collect::<Vec<_>>();
          ids.sort();
          ids.dedup();
          (name, ids)
        })
        .collect(),
      packages,
      package_reqs: self.package_reqs,
    })
  }

  // Debugging methods

  #[cfg(debug_assertions)]
  #[allow(unused)]
  fn output_path(&self, path: &Arc<GraphPath>) {
    eprintln!("-----------");
    self.output_node(path.node_id(), false);
    for path in path.ancestors() {
      match path {
        GraphPathNodeOrRoot::Node(node) => {
          self.output_node(node.node_id(), false)
        }
        GraphPathNodeOrRoot::Root(pkg_id) => {
          let node_id = self.root_packages.get(pkg_id).unwrap();
          eprintln!(
            "Root: {} ({}: {})",
            pkg_id,
            node_id.0,
            self.get_npm_pkg_id(*node_id).as_serialized()
          )
        }
      }
    }
    eprintln!("-----------");
  }

  #[cfg(debug_assertions)]
  #[allow(unused)]
  fn output_node(&self, node_id: NodeId, show_children: bool) {
    eprintln!(
      "{:>4}: {}",
      node_id.0,
      self.get_npm_pkg_id(node_id).as_serialized()
    );

    if show_children {
      let node = self.nodes.get(&node_id).unwrap();
      eprintln!("       Children:");
      for (specifier, child_id) in &node.children {
        eprintln!("         {}: {}", specifier, child_id.0);
      }
    }
  }

  #[cfg(debug_assertions)]
  #[allow(unused)]
  pub fn output_nodes(&self) {
    eprintln!("~~~");
    let mut node_ids = self
      .resolved_node_ids
      .node_to_resolved_id
      .keys()
      .copied()
      .collect::<Vec<_>>();
    node_ids.sort_by(|a, b| a.0.cmp(&b.0));
    for node_id in node_ids {
      self.output_node(node_id, true);
    }
    eprintln!("~~~");
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
      .with_context(|| format!("npm package: {nv}"))?;
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
  unresolved_optional_peers: HashMap<NpmPackageNv, Vec<UnresolvedOptionalPeer>>,
  dep_entry_cache: DepEntryCache,
}

impl<'a> GraphDependencyResolver<'a> {
  pub fn new(graph: &'a mut Graph, api: &'a NpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_unresolved_nodes: Default::default(),
      unresolved_optional_peers: Default::default(),
      dep_entry_cache: Default::default(),
    }
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
    self.graph.root_packages.insert(pkg_id.clone(), node_id);
    self
      .pending_unresolved_nodes
      .push_back(GraphPath::for_root(node_id, pkg_id));
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: &NpmPackageInfo,
    graph_path: &Arc<GraphPath>,
  ) -> Result<NodeId, AnyError> {
    debug_assert_eq!(entry.kind, NpmDependencyEntryKind::Dep);
    let parent_id = graph_path.node_id();
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
      self.graph.set_child_parent_node(
        &entry.bare_specifier,
        node_id,
        parent_id,
      );
      self.try_add_pending_unresolved_node(
        graph_path,
        node_id,
        &entry.bare_specifier,
      );
    }
    Ok(node_id)
  }

  fn try_add_pending_unresolved_node(
    &mut self,
    path: &Arc<GraphPath>,
    node_id: NodeId,
    specifier: &str,
  ) {
    if self.graph.nodes.get(&node_id).unwrap().no_peers {
      return; // skip, no need to analyze this again
    }
    let node_nv = self
      .graph
      .resolved_node_ids
      .get(node_id)
      .unwrap()
      .nv
      .clone();
    let new_path = match path.with_id(node_id, specifier, node_nv) {
      Some(visited_versions) => visited_versions,
      None => return, // circular, don't visit this node
    };
    self.pending_unresolved_nodes.push_back(new_path);
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
        .nodes_by_package_name
        .entry(package_info.name.clone())
        .or_default()
        .iter()
        .map(|node_id| {
          &self
            .graph
            .resolved_node_ids
            .get(*node_id)
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
    let (_, node_id) = self.graph.get_or_create_for_id(&resolved_id);
    let pkg_id = resolved_id.nv;

    let has_deps = if let Some(deps) = self.dep_entry_cache.get(&pkg_id) {
      !deps.is_empty()
    } else {
      let deps = self
        .dep_entry_cache
        .store(pkg_id.clone(), version_and_info.info)?;
      !deps.is_empty()
    };

    if !has_deps {
      // ensure this is set if not, as it's an optimization
      let mut node = self.graph.borrow_node_mut(node_id);
      node.no_peers = true;
    }

    debug!(
      "{} - Resolved {}@{} to {}",
      match parent_id {
        Some(parent_id) => self.graph.get_npm_pkg_id(parent_id).as_serialized(),
        None => "<package-req>".to_string(),
      },
      pkg_req_name,
      version_req.version_text(),
      pkg_id.to_string(),
    );

    Ok((pkg_id, node_id))
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some(graph_path) = self.pending_unresolved_nodes.pop_front() {
        let (pkg_id, deps) = {
          let node_id = graph_path.node_id();
          if self.graph.nodes.get(&node_id).unwrap().no_peers {
            // We can skip as there's no reason to analyze this graph segment further
            // Note that we don't need to count parent references here because that's
            // only necessary for graph segments that could potentially have peer
            // dependencies within them.
            continue;
          }

          let pkg_nv = self
            .graph
            .resolved_node_ids
            .get(node_id)
            .unwrap()
            .nv
            .clone();
          let deps = if let Some(deps) = self.dep_entry_cache.get(&pkg_nv) {
            deps.clone()
          } else {
            // the api should have this in the cache at this point, so no need to parallelize
            match self.api.package_version_info(&pkg_nv).await? {
              Some(version_info) => {
                self.dep_entry_cache.store(pkg_nv.clone(), &version_info)?
              }
              None => {
                bail!("Could not find version information for {}", pkg_nv)
              }
            }
          };

          (pkg_nv, deps)
        };

        // cache all the dependencies' registry infos in parallel if should
        self
          .api
          .cache_in_parallel({
            deps.iter().map(|dep| dep.name.clone()).collect()
          })
          .await?;

        // resolve the dependencies
        let mut found_peer = false;

        for dep in deps.iter() {
          let package_info = self.api.package_info(&dep.name).await?;

          match dep.kind {
            NpmDependencyEntryKind::Dep => {
              // todo(dsherret): look into skipping dependency analysis if
              // it was done previously again
              let child_id =
                self.analyze_dependency(dep, &package_info, &graph_path)?;

              if !found_peer {
                found_peer = !self.graph.borrow_node_mut(child_id).no_peers;
              }
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              found_peer = true;
              // we need to re-evaluate peer dependencies every time and can't
              // skip over them because they might be evaluated differently based
              // on the current path
              let maybe_new_id = self.resolve_peer_dep(
                &dep.bare_specifier,
                dep,
                &package_info,
                &graph_path,
              )?;

              // For optional dependencies, we want to resolve them if any future
              // same parent version resolves them. So when not resolved, store them to be
              // potentially resolved later.
              //
              // Note: This is not a good solution, but will probably work ok in most
              // scenarios. We can work on improving this in the future. We probably
              // want to resolve future optional peers to the same dependency for example.
              if dep.kind == NpmDependencyEntryKind::OptionalPeer {
                match maybe_new_id {
                  Some(new_id) => {
                    if let Some(unresolved_optional_peers) =
                      self.unresolved_optional_peers.remove(&pkg_id)
                    {
                      for optional_peer in unresolved_optional_peers {
                        let peer_parent = GraphPathNodeOrRoot::Node(
                          optional_peer.graph_path.clone(),
                        );
                        self.set_new_peer_dep(
                          vec![&optional_peer.graph_path],
                          peer_parent,
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
          self.graph.borrow_node_mut(graph_path.node_id()).no_peers = true;
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

    let mut path = vec![ancestor_path];

    // the current dependency might have had the peer dependency
    // in another bare specifier slot... if so resolve it to that
    {
      let maybe_peer_dep = self.find_peer_dep_in_node(
        ancestor_path,
        peer_dep,
        peer_package_info,
      )?;

      if let Some((peer_parent, peer_dep_id)) = maybe_peer_dep {
        // this will always have an ancestor because we're not at the root
        self.set_new_peer_dep(path, peer_parent, specifier, peer_dep_id);
        return Ok(Some(peer_dep_id));
      }
    }

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    for ancestor_node in ancestor_path.ancestors() {
      match ancestor_node {
        GraphPathNodeOrRoot::Node(ancestor_graph_path_node) => {
          path.push(ancestor_graph_path_node);
          let maybe_peer_dep = self.find_peer_dep_in_node(
            ancestor_graph_path_node,
            peer_dep,
            peer_package_info,
          )?;
          if let Some((parent, peer_dep_id)) = maybe_peer_dep {
            // this will always have an ancestor because we're not at the root
            self.set_new_peer_dep(path, parent, specifier, peer_dep_id);
            return Ok(Some(peer_dep_id));
          }
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
            let peer_parent = GraphPathNodeOrRoot::Root(root_pkg_id.clone());
            self.set_new_peer_dep(path, peer_parent, specifier, child_id);
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
      let peer_parent = GraphPathNodeOrRoot::Node(ancestor_path.clone());
      self.set_new_peer_dep(
        vec![ancestor_path],
        peer_parent,
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
  ) -> Result<Option<(GraphPathNodeOrRoot, NodeId)>, AnyError> {
    let node_id = path.node_id();
    let resolved_node_id = self.graph.resolved_node_ids.get(node_id).unwrap();
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
      let parent = path.previous_node.as_ref().unwrap().clone();
      Ok(Some((parent, node_id)))
    } else {
      let node = self.graph.nodes.get(&node_id).unwrap();
      let children = node.children.values().map(|child_node_id| {
        let child_node_id = *child_node_id;
        (
          child_node_id,
          &self.graph.resolved_node_ids.get(child_node_id).unwrap().nv,
        )
      });
      find_matching_child(peer_dep, peer_package_info, children).map(
        |maybe_child_id| {
          maybe_child_id.map(|child_id| {
            let parent = GraphPathNodeOrRoot::Node(path.clone());
            (parent, child_id)
          })
        },
      )
    }
  }

  fn set_new_peer_dep(
    &mut self,
    // path from the node above the resolved dep to just above the peer dep
    path: Vec<&Arc<GraphPath>>,
    peer_dep_parent: GraphPathNodeOrRoot,
    peer_dep_specifier: &str,
    mut peer_dep_id: NodeId,
  ) {
    debug_assert!(!path.is_empty());
    let peer_dep_pkg_id = self
      .graph
      .resolved_node_ids
      .get(peer_dep_id)
      .unwrap()
      .nv
      .clone();

    let peer_dep = ResolvedIdPeerDep::ParentReference {
      parent: peer_dep_parent,
      child_pkg_nv: peer_dep_pkg_id,
    };
    for graph_path_node in path.iter().rev() {
      let old_node_id = graph_path_node.node_id();
      let old_resolved_id = self
        .graph
        .resolved_node_ids
        .get(old_node_id)
        .unwrap()
        .clone();

      let mut new_resolved_id = old_resolved_id.clone();
      new_resolved_id.push_peer_dep(peer_dep.clone());
      let (created, new_node_id) =
        self.graph.get_or_create_for_id(&new_resolved_id);

      // this will occur when the peer dependency is in an ancestor
      if old_node_id == peer_dep_id {
        peer_dep_id = new_node_id;
      }

      if created {
        let old_children =
          self.graph.borrow_node_mut(old_node_id).children.clone();
        // copy over the old children to this new one
        for (specifier, child_id) in &old_children {
          self
            .graph
            .set_child_parent_node(specifier, *child_id, new_node_id);
        }
      }

      debug_assert_eq!(graph_path_node.node_id(), old_node_id);
      graph_path_node.change_id(new_node_id);

      // update the previous parent to have this as its child
      match graph_path_node.previous_node.as_ref().unwrap() {
        GraphPathNodeOrRoot::Root(pkg_id) => {
          self.graph.root_packages.insert(pkg_id.clone(), new_node_id);
        }
        GraphPathNodeOrRoot::Node(parent_node_path) => {
          let parent_node_id = parent_node_path.node_id();
          let parent_node = self.graph.borrow_node_mut(parent_node_id);
          parent_node
            .children
            .insert(graph_path_node.specifier().to_string(), new_node_id);
        }
      }
    }

    // now set the peer dependency
    let bottom_node = path.first().unwrap();
    let parent_node_id = bottom_node.node_id();
    self.graph.set_child_parent_node(
      peer_dep_specifier,
      peer_dep_id,
      parent_node_id,
    );

    // mark the peer dependency to be analyzed
    self.try_add_pending_unresolved_node(
      bottom_node,
      peer_dep_id,
      peer_dep_specifier,
    );

    debug!(
      "Resolved peer dependency for {} in {} to {}",
      peer_dep_specifier,
      &self.graph.get_npm_pkg_id(parent_node_id).as_serialized(),
      &self.graph.get_npm_pkg_id(peer_dep_id).as_serialized(),
    );
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

  #[test]
  fn resolved_id_tests() {
    let mut ids = ResolvedNodeIds::default();
    let node_id = NodeId(0);
    let resolved_id = ResolvedId {
      nv: NpmPackageNv::from_str("package@1.1.1").unwrap(),
      peer_dependencies: Vec::new(),
    };
    ids.set(node_id, resolved_id.clone());
    assert!(ids.get(node_id).is_some());
    assert!(ids.get(NodeId(1)).is_none());
    assert_eq!(ids.get_node_id(&resolved_id), Some(node_id));

    let resolved_id_new = ResolvedId {
      nv: NpmPackageNv::from_str("package@1.1.2").unwrap(),
      peer_dependencies: Vec::new(),
    };
    ids.set(node_id, resolved_id_new.clone());
    assert_eq!(ids.get_node_id(&resolved_id), None); // stale entry should have been removed
    assert!(ids.get(node_id).is_some());
    assert_eq!(ids.get_node_id(&resolved_id_new), Some(node_id));
  }

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
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized("package-c@0.1.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-c@0.1.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageId::from_serialized("package-d@3.2.1").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-d@3.2.1").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
  async fn peer_deps_simple_top_tree() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1.0", "npm:package-peer@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-a@1.0".to_string(),
          "package-a@1.0.0_package-peer@1.0.0".to_string()
        ),
        (
          "package-peer@1.0".to_string(),
          "package-peer@1.0.0".to_string()
        )
      ]
    );
  }

  #[tokio::test]
  async fn peer_deps_simple_root_pkg_children() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-0", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-0@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized(
                "package-a@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![(
        "package-0@1.0".to_string(),
        "package-0@1.0.0_package-peer@1.0.0".to_string()
      ),]
    );
  }

  #[tokio::test]
  async fn peer_deps_simple_deeper() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-1", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-1", "1"));
    api.add_dependency(("package-1", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-1", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-1".to_string(),
            NpmPackageId::from_serialized("package-1@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-1@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized(
                "package-a@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.0".to_string(), "package-0@1.0.0".to_string()),]
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized("package-0@1.1.1").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-peer@4.0.0")
              .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer@4.1.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer@4.1.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer@4.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer@4.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-c@3.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized(
                "package-a@1.0.0_package-peer@2.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
            )
          ]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 1,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized(
                "package-a@1.0.0_package-peer@2.0.0"
              )
              .unwrap(),
            ),
          ]),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
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
  async fn resolve_peer_dep_other_specifier_slot() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");
    // bit of an edge case... probably nobody has ever done this
    api.add_dependency(
      ("package-a", "1.0.0"),
      ("package-peer2", "npm:package-peer@2"),
    );
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "2"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@2.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
            ),
            (
              "package-peer2".to_string(),
              NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![(
        "package-a@1".to_string(),
        "package-a@1.0.0_package-peer@2.0.0".to_string()
      ),]
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
          pkg_id: NpmPackageId::from_serialized(
            "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::from_serialized(
              "package-peer-a@2.0.0_package-peer-b@3.0.0"
            )
            .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::from_serialized("package-peer-b@3.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer-b@3.0.0")
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
          pkg_id: NpmPackageId::from_serialized(
            "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageId::from_serialized(
                "package-peer-a@2.0.0_package-peer-b@3.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer-b".to_string(),
              NpmPackageId::from_serialized("package-peer-b@3.0.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::from_serialized("package-peer-b@3.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer-b@3.0.0")
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
          pkg_id: NpmPackageId::from_serialized("package-0@1.1.1")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized(
              "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
              )
              .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageId::from_serialized("package-d@3.5.0").unwrap(),
            ),
            (
              "package-peer-a".to_string(),
              NpmPackageId::from_serialized(
                "package-peer-a@4.0.0_package-peer-b@5.4.1"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
                .unwrap(),
            ),
            (
              "package-peer-c".to_string(),
              NpmPackageId::from_serialized("package-peer-c@6.2.0")
                .unwrap(),
            )
          ])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-d@3.5.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-e@3.6.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer-a@4.0.0_package-peer-b@5.4.1")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::from_serialized("package-peer-b@5.4.1")
              .unwrap(),
          )])
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer-b@5.4.1")
            .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer-c@6.2.0")
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
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0_package-a@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-a@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
            pkg_id: NpmPackageId::from_serialized(
              "package-a@1.0.0_package-peer@4.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([
              (
                "package-dep".to_string(),
                NpmPackageId::from_serialized(
                  "package-dep@3.0.0_package-peer@4.0.0"
                )
                .unwrap(),
              ),
              (
                "package-peer".to_string(),
                NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
              ),
            ]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageId::from_serialized(
              "package-b@2.0.0_package-peer@5.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([
              (
                "package-dep".to_string(),
                NpmPackageId::from_serialized(
                  "package-dep@3.0.0_package-peer@5.0.0"
                )
                .unwrap(),
              ),
              (
                "package-peer".to_string(),
                NpmPackageId::from_serialized("package-peer@5.0.0").unwrap(),
              ),
            ]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageId::from_serialized(
              "package-dep@3.0.0_package-peer@4.0.0"
            )
            .unwrap(),
            copy_index: 0,
            dependencies: HashMap::from([(
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
            )]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageId::from_serialized(
              "package-dep@3.0.0_package-peer@5.0.0"
            )
            .unwrap(),
            copy_index: 1,
            dependencies: HashMap::from([(
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@5.0.0").unwrap(),
            )]),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageId::from_serialized("package-peer@4.0.0")
              .unwrap(),
            copy_index: 0,
            dependencies: HashMap::new(),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            pkg_id: NpmPackageId::from_serialized("package-peer@5.0.0")
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 1,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.1.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.2.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.2.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@1.0.0_package-peer@1.1.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.1.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-peer@1.1.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.1.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.2.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-d@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized("package-c@1.0.0_package-d@1.0.0")
                .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageId::from_serialized("package-d@1.0.0").unwrap(),
            ),
            (
              "package-e".to_string(),
              NpmPackageId::from_serialized("package-e@1.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@1.0.0_package-d@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageId::from_serialized("package-d@1.0.0").unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-d@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-e@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
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
        pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
          pkg_id: NpmPackageId::from_serialized("package-a@0.5.0").unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@0.5.0").unwrap(),
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
  async fn grand_child_package_has_self_as_peer_dependency_root() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "2"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0_package-a@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-a@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn grand_child_package_has_self_as_peer_dependency_under_root() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "*"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "2"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0_package-a@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-a@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.0".to_string(), "package-0@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn nested_deps_same_peer_dep_ancestor() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-1", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-0", "1.0.0"), ("package-1", "1"));
    api.add_dependency(("package-1", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-a", "*"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-a", "*"));
    api.add_peer_dependency(("package-d", "1.0.0"), ("package-a", "*"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-0", "*"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-0", "*"));
    api.add_peer_dependency(("package-d", "1.0.0"), ("package-0", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
          ), (
            "package-1".to_string(),
            NpmPackageId::from_serialized("package-1@1.0.0_package-0@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-1@1.0.0_package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-0".to_string(),
              NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized("package-c@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-c@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-0".to_string(),
              NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageId::from_serialized("package-d@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0")
                .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-d@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-0".to_string(),
              NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized("package-a@1.0.0_package-0@1.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.0".to_string(), "package-0@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn peer_dep_resolved_then_resolved_deeper() {
    let api = TestNpmRegistryApiInner::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-1", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-0", "1.0.0"), ("package-1", "1"));
    api.add_dependency(("package-1", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-0@1.0", "npm:package-peer@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-0@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-1".to_string(),
              NpmPackageId::from_serialized(
                "package-1@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
            ),
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized(
                "package-a@1.0.0_package-peer@1.0.0"
              )
              .unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-1@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0_package-peer@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-peer@1.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          pkg_id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: Default::default(),
          dist: Default::default(),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-0@1.0".to_string(),
          "package-0@1.0.0_package-peer@1.0.0".to_string()
        ),
        (
          "package-peer@1.0".to_string(),
          "package-peer@1.0.0".to_string()
        )
      ]
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
        .unwrap()
        .into_snapshot(&api)
        .await
        .unwrap();
      assert_eq!(
        snapshot, new_snapshot,
        "recreated snapshot should be the same"
      );
      // create one again from the new snapshot
      let new_snapshot2 = Graph::from_snapshot(new_snapshot.clone())
        .unwrap()
        .into_snapshot(&api)
        .await
        .unwrap();
      assert_eq!(
        snapshot, new_snapshot2,
        "second recreated snapshot should be the same"
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
