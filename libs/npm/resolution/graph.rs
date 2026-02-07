// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::rc::Rc;

use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::debug;
use thiserror::Error;

use super::common::NpmPackageVersionResolutionError;
use super::common::NpmPackageVersionResolver;
use super::common::NpmVersionResolver;
use super::snapshot::NpmResolutionSnapshot;
use crate::NpmPackageId;
use crate::NpmResolutionPackage;
use crate::NpmResolutionPackageSystemInfo;
use crate::registry::NpmDependencyEntry;
use crate::registry::NpmDependencyEntryError;
use crate::registry::NpmDependencyEntryKind;
use crate::registry::NpmPackageInfo;
use crate::registry::NpmPackageVersionInfo;
use crate::registry::NpmRegistryApi;
use crate::registry::NpmRegistryPackageInfoLoadError;
use crate::resolution::collections::OneDirectionalLinkedList;
use crate::resolution::snapshot::SnapshotPackageCopyIndexResolver;

pub trait Reporter: std::fmt::Debug + Send + Sync {
  #[allow(unused_variables)]
  fn on_resolved(&self, package_req: &PackageReq, nv: &PackageNv) {}
}

// todo(dsherret): for perf we should use an arena/bump allocator for
// creating the nodes and paths since this is done in a phase

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnmetPeerDepDiagnostic {
  /// Ancestor nodes going up the graph to the root.
  pub ancestors: Vec<PackageNv>,
  pub dependency: PackageReq,
  pub resolved: Version,
}

#[derive(Debug, Clone, Error, deno_error::JsError)]
pub enum NpmResolutionError {
  #[class(inherit)]
  #[error(transparent)]
  Registry(#[from] NpmRegistryPackageInfoLoadError),
  #[class(inherit)]
  #[error(transparent)]
  Resolution(#[from] NpmPackageVersionResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  DependencyEntry(#[from] Box<NpmDependencyEntryError>),
}

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
  pub children: BTreeMap<StackString, NodeId>,
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
    child_pkg_nv: Rc<PackageNv>,
  },
  /// A node that was created during snapshotting and is not being used in any path.
  SnapshotNodeId(NodeId),
}

impl ResolvedIdPeerDep {
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
  nv: Rc<PackageNv>,
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
}

/// Mappings of node identifiers to resolved identifiers. Each node has exactly
/// one resolved identifier.
///
/// The mapping from resolved to node_ids is imprecise and will do a best attempt
/// at sharing nodes.
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

  pub fn clear_peer_deps(&mut self) {
    self.resolved_to_node_id.clear();
    for (node_id, (resolved_id, resolved_id_hash)) in
      &mut self.node_to_resolved_id
    {
      resolved_id.peer_dependencies.clear();
      *resolved_id_hash = resolved_id.current_state_hash();
      self.resolved_to_node_id.insert(*resolved_id_hash, *node_id);
    }
  }
}

/// A pointer to a specific node in a graph path. The underlying node id
/// may change as peer dependencies are created.
#[derive(Debug)]
struct NodeIdRef(Cell<NodeId>);

impl NodeIdRef {
  pub fn new(node_id: NodeId) -> Self {
    NodeIdRef(Cell::new(node_id))
  }

  pub fn change(&self, node_id: NodeId) {
    self.0.set(node_id);
  }

  pub fn get(&self) -> NodeId {
    self.0.get()
  }
}

#[derive(Clone)]
enum GraphPathNodeOrRoot {
  Node(Rc<GraphPath>),
  Root(Rc<PackageNv>),
}

#[derive(Debug, Copy, Clone)]
enum GraphPathResolutionMode {
  All,
  OptionalPeers,
}

/// Path through the graph that represents a traversal through the graph doing
/// the dependency resolution. The graph tries to share duplicate package
/// information and we try to avoid traversing parts of the graph that we know
/// are resolved.
struct GraphPath {
  previous_node: Option<GraphPathNodeOrRoot>,
  node_id_ref: NodeIdRef,
  specifier: StackString,
  // we could consider not storing this here and instead reference the resolved
  // nodes, but we should performance profile this code first
  nv: Rc<PackageNv>,
  /// Descendants in the path that circularly link to an ancestor in a child. These
  /// descendants should be kept up to date and always point to this node.
  linked_circular_descendants: RefCell<Vec<Rc<GraphPath>>>,
  mode: GraphPathResolutionMode,
}

impl GraphPath {
  pub fn for_root(
    node_id: NodeId,
    nv: Rc<PackageNv>,
    mode: GraphPathResolutionMode,
  ) -> Rc<Self> {
    Rc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Root(nv.clone())),
      node_id_ref: NodeIdRef::new(node_id),
      // use an empty specifier
      specifier: "".into(),
      nv,
      linked_circular_descendants: Default::default(),
      mode,
    })
  }

  pub fn node_id(&self) -> NodeId {
    self.node_id_ref.get()
  }

  pub fn specifier(&self) -> &StackString {
    &self.specifier
  }

  pub fn change_id(&self, node_id: NodeId) {
    self.node_id_ref.change(node_id)
  }

  pub fn with_id(
    self: &Rc<GraphPath>,
    node_id: NodeId,
    specifier: StackString,
    nv: Rc<PackageNv>,
    mode: GraphPathResolutionMode,
  ) -> Rc<Self> {
    Rc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Node(self.clone())),
      node_id_ref: NodeIdRef::new(node_id),
      specifier,
      nv,
      linked_circular_descendants: Default::default(),
      mode,
    })
  }

  /// Gets if there is an ancestor with the same name & version along this path.
  pub fn find_ancestor(&self, nv: &PackageNv) -> Option<Rc<GraphPath>> {
    let mut maybe_next_node = self.previous_node.as_ref();
    while let Some(GraphPathNodeOrRoot::Node(next_node)) = maybe_next_node {
      // we've visited this before, so stop
      if *next_node.nv == *nv {
        return Some(next_node.clone());
      }
      maybe_next_node = next_node.previous_node.as_ref();
    }
    None
  }

  /// Gets the bottom-up path to the ancestor not including the current or ancestor node.
  pub fn get_path_to_ancestor_exclusive(
    &self,
    ancestor_node_id: NodeId,
  ) -> Vec<&Rc<GraphPath>> {
    let mut path = Vec::new();
    let mut maybe_next_node = self.previous_node.as_ref();
    while let Some(GraphPathNodeOrRoot::Node(next_node)) = maybe_next_node {
      if next_node.node_id() == ancestor_node_id {
        break;
      }
      path.push(next_node);
      maybe_next_node = next_node.previous_node.as_ref();
    }
    debug_assert!(maybe_next_node.is_some());
    path
  }

  pub fn ancestors(&self) -> GraphPathAncestorIterator<'_> {
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

struct PackagesForSnapshot<'a> {
  packages: Vec<NpmResolutionPackage>,
  packages_by_name: HashMap<PackageName, Vec<NpmPackageId>>,
  traversed_ids: HashSet<&'a NpmPackageId>,
}

pub struct Graph {
  /// Each requirement is mapped to a specific name and version.
  package_reqs: HashMap<PackageReq, Rc<PackageNv>>,
  /// Then each name and version is mapped to an exact node id.
  /// Note: Uses a BTreeMap in order to create some determinism
  /// when creating the snapshot.
  root_packages: BTreeMap<Rc<PackageNv>, NodeId>,
  package_name_versions: HashMap<StackString, HashSet<Version>>,
  nodes: HashMap<NodeId, Node>,
  resolved_node_ids: ResolvedNodeIds,
  // This will be set when creating from a snapshot, then
  // inform the final snapshot creation.
  packages_to_copy_index: HashMap<NpmPackageId, u8>,
  moved_package_ids: IndexMap<NodeId, (ResolvedId, ResolvedId)>,
  unresolved_optional_peers: UnresolvedOptionalPeers,
  #[cfg(feature = "tracing")]
  traces: Vec<super::tracing::TraceGraphSnapshot>,
}

impl Graph {
  pub fn from_snapshot(snapshot: NpmResolutionSnapshot) -> Self {
    fn get_or_create_graph_node<'a>(
      graph: &mut Graph,
      pkg_id: &NpmPackageId,
      packages: &HashMap<NpmPackageId, NpmResolutionPackage>,
      created_package_ids: &mut HashMap<NpmPackageId, NodeId>,
      ancestor_ids: &'a OneDirectionalLinkedList<'a, NpmPackageId>,
    ) -> NodeId {
      if let Some(id) = created_package_ids.get(pkg_id) {
        return *id;
      }

      let node_id = graph.create_node(&pkg_id.nv);
      created_package_ids.insert(pkg_id.clone(), node_id);
      let ancestor_ids_with_current = ancestor_ids.push(pkg_id);

      let peer_dep_ids = pkg_id
        .peer_dependencies
        .iter()
        .map(|peer_dep| {
          ResolvedIdPeerDep::SnapshotNodeId(get_or_create_graph_node(
            graph,
            peer_dep,
            packages,
            created_package_ids,
            &ancestor_ids_with_current,
          ))
        })
        .collect::<Vec<_>>();
      let graph_resolved_id = ResolvedId {
        nv: Rc::new(pkg_id.nv.clone()),
        peer_dependencies: peer_dep_ids,
      };
      let resolution = match packages.get(pkg_id) {
        Some(package) => package,
        None => {
          // when this occurs, it means that the pkg_id is circular. For example:
          //   package-b@1.0.0_package-c@1.0.0__package-b@1.0.0
          //                                    ^ attempting to resolve this
          // In this case, we go up the ancestors to see if we can find a matching nv.
          let id = ancestor_ids
            .iter()
            .find(|id| id.nv == pkg_id.nv && packages.contains_key(id))
            .unwrap_or_else(|| {
              // If a matching nv is not found in the ancestors, then we fall
              // back to searching the entire collection of packages for a matching
              // nv and just select the first one even though that might not be exactly
              // correct. I suspect this scenario will be super rare to occur.
              let mut packages_with_same_nv = packages
                .keys()
                .filter(|id| id.nv == pkg_id.nv)
                .collect::<Vec<_>>();
              packages_with_same_nv.sort();
              if packages_with_same_nv.is_empty() {
                // we verify in other places that references in a snapshot
                // should be valid (ex. when creating a snapshot), so we should
                // never get here and if so that indicates a bug elsewhere
                panic!("not found package id: {}", pkg_id.as_serialized());
              } else {
                packages_with_same_nv.remove(0)
              }
            });
          packages.get(id).unwrap()
        }
      };
      for (name, child_id) in &resolution.dependencies {
        let child_node_id = get_or_create_graph_node(
          graph,
          child_id,
          packages,
          created_package_ids,
          &ancestor_ids_with_current,
        );
        // this condition is only for past graphs that have been incorrectly created
        // with a child that points to itself (see the graph_from_snapshot_dep_on_self
        // test)
        if node_id != child_node_id {
          graph.set_child_of_parent_node(node_id, name, child_node_id);
        }
      }
      for key in &resolution.optional_peer_dependencies {
        if resolution.dependencies.contains_key(key) {
          graph
            .unresolved_optional_peers
            .mark_seen(graph_resolved_id.nv.clone(), key);
        }
      }
      graph.resolved_node_ids.set(node_id, graph_resolved_id);
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
      package_reqs: snapshot
        .package_reqs
        .into_iter()
        .map(|(k, v)| (k, Rc::new(v)))
        .collect(),
      nodes: Default::default(),
      package_name_versions: Default::default(),
      resolved_node_ids: Default::default(),
      root_packages: Default::default(),
      unresolved_optional_peers: Default::default(),
      moved_package_ids: Default::default(),
      #[cfg(feature = "tracing")]
      traces: Default::default(),
    };
    let mut created_package_ids =
      HashMap::with_capacity(snapshot.packages.len());
    for (id, resolved_id) in snapshot.root_packages {
      let node_id = get_or_create_graph_node(
        &mut graph,
        &resolved_id,
        &snapshot.packages,
        &mut created_package_ids,
        &Default::default(),
      );
      graph.root_packages.insert(Rc::new(id), node_id);
    }
    graph
  }

  pub fn get_req_nv(&self, req: &PackageReq) -> Option<&Rc<PackageNv>> {
    self.package_reqs.get(req)
  }

  fn get_npm_pkg_id(&self, node_id: NodeId) -> NpmPackageId {
    let resolved_id = self.resolved_node_ids.get(node_id).unwrap();
    self.get_npm_pkg_id_from_resolved_id(resolved_id)
  }

  #[inline(always)]
  fn get_npm_pkg_id_from_resolved_id(
    &self,
    resolved_id: &ResolvedId,
  ) -> NpmPackageId {
    self.get_npm_pkg_id_from_resolved_id_with_seen(
      resolved_id,
      HashSet::from([resolved_id.nv.clone()]),
    )
  }

  fn get_npm_pkg_id_from_resolved_id_with_seen(
    &self,
    resolved_id: &ResolvedId,
    seen: HashSet<Rc<PackageNv>>,
  ) -> NpmPackageId {
    if resolved_id.peer_dependencies.is_empty() {
      NpmPackageId {
        nv: (*resolved_id.nv).clone(),
        peer_dependencies: Default::default(),
      }
    } else {
      let mut npm_pkg_id = NpmPackageId {
        nv: (*resolved_id.nv).clone(),
        peer_dependencies: crate::NpmPackageIdPeerDependencies::with_capacity(
          resolved_id.peer_dependencies.len(),
        ),
      };
      let mut seen_children_resolved_ids =
        HashSet::with_capacity(resolved_id.peer_dependencies.len());
      for peer_dep in &resolved_id.peer_dependencies {
        let maybe_node_and_resolved_id =
          self.peer_dep_to_maybe_node_id_and_resolved_id(peer_dep);
        // this should always be set
        debug_assert!(maybe_node_and_resolved_id.is_some());
        if let Some((_child_id, child_resolved_id)) = maybe_node_and_resolved_id
        {
          let mut new_seen = seen.clone();
          if new_seen.insert(child_resolved_id.nv.clone()) {
            let child_peer = self.get_npm_pkg_id_from_resolved_id_with_seen(
              child_resolved_id,
              new_seen.clone(),
            );

            if seen_children_resolved_ids.insert(child_peer.clone()) {
              npm_pkg_id.peer_dependencies.push(child_peer);
            }
          } else {
            npm_pkg_id.peer_dependencies.push(NpmPackageId {
              nv: (*child_resolved_id.nv).clone(),
              peer_dependencies: Default::default(),
            });
          }
        }
      }
      npm_pkg_id
    }
  }

  fn peer_dep_to_maybe_node_id_and_resolved_id(
    &self,
    peer_dep: &ResolvedIdPeerDep,
  ) -> Option<(NodeId, &ResolvedId)> {
    match peer_dep {
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

  fn create_node(&mut self, pkg_nv: &PackageNv) -> NodeId {
    let node_id = NodeId(self.nodes.len() as u32);
    let node = Node {
      children: Default::default(),
      no_peers: false,
    };

    self
      .package_name_versions
      .entry(pkg_nv.name.clone())
      .or_default()
      .insert(pkg_nv.version.clone());
    self.nodes.insert(node_id, node);

    node_id
  }

  fn borrow_node_mut(&mut self, node_id: NodeId) -> &mut Node {
    self.nodes.get_mut(&node_id).unwrap()
  }

  fn set_child_of_parent_node(
    &mut self,
    parent_id: NodeId,
    specifier: &StackString,
    child_id: NodeId,
  ) {
    assert_ne!(child_id, parent_id);
    let parent = self.borrow_node_mut(parent_id);
    parent.children.insert(specifier.clone(), child_id);
  }

  pub async fn into_snapshot<TNpmRegistryApi: NpmRegistryApi>(
    self,
    api: &TNpmRegistryApi,
    link_packages: &HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
  ) -> Result<NpmResolutionSnapshot, NpmResolutionError> {
    #[cfg(feature = "tracing")]
    if !self.traces.is_empty() {
      super::tracing::output(&self.traces);
    }

    let packages_to_pkg_ids = self
      .nodes
      .keys()
      .map(|node_id| (*node_id, self.get_npm_pkg_id(*node_id)))
      .collect::<HashMap<_, _>>();

    let pkgs = match self
      .resolve_packages_for_snapshot_with_maybe_restart(
        api,
        link_packages,
        &packages_to_pkg_ids,
      )
      .await?
    {
      Some(pkgs) => pkgs,
      None => self
        .resolve_packages_for_snapshot_with_maybe_restart(
          api,
          link_packages,
          &packages_to_pkg_ids,
        )
        .await?
        // a panic here means the api is doing multiple reloads of data
        // from the npm registry, which it shouldn't be doing and is considered
        // a bug in that implementation
        .unwrap(),
    };

    self.into_snapshot_from_packages(&packages_to_pkg_ids, pkgs)
  }

  async fn resolve_packages_for_snapshot_with_maybe_restart<
    'ids,
    TNpmRegistryApi: NpmRegistryApi,
  >(
    &self,
    api: &TNpmRegistryApi,
    link_packages: &HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
    packages_to_pkg_ids: &'ids HashMap<NodeId, NpmPackageId>,
  ) -> Result<Option<PackagesForSnapshot<'ids>>, NpmResolutionError> {
    let mut traversed_ids = HashSet::with_capacity(self.nodes.len());
    let mut pending = VecDeque::with_capacity(self.nodes.len());
    for root_id in self.root_packages.values().copied() {
      let pkg_id = packages_to_pkg_ids.get(&root_id).unwrap();
      if traversed_ids.insert(pkg_id) {
        pending.push_back((root_id, pkg_id));
      }
    }

    let mut pending_futures = futures::stream::FuturesOrdered::new();
    while let Some((node_id, pkg_id)) = pending.pop_front() {
      let node = self.nodes.get(&node_id).unwrap();
      let mut dependencies = HashMap::with_capacity(node.children.len());
      for (specifier, child_id) in &node.children {
        let child_id = *child_id;
        let child_pkg_id = packages_to_pkg_ids.get(&child_id).unwrap();
        if traversed_ids.insert(child_pkg_id) {
          pending.push_back((child_id, child_pkg_id));
        }
        dependencies.insert(specifier.clone(), child_pkg_id.clone());
      }

      pending_futures.push_back(async move {
        let package_info = api.package_info(&pkg_id.nv.name).await?;
        Ok::<_, NpmRegistryPackageInfoLoadError>((
          pkg_id,
          dependencies,
          package_info,
        ))
      });
    }

    let mut packages_by_name: HashMap<PackageName, Vec<_>> =
      HashMap::with_capacity(self.nodes.len());
    let mut packages: Vec<NpmResolutionPackage> =
      Vec::with_capacity(self.nodes.len());
    while let Some(result) = pending_futures.next().await {
      let (pkg_id, dependencies, package_info) = result?;
      let version_info =
        match package_info.version_info(&pkg_id.nv, link_packages) {
          Ok(info) => info,
          Err(err) => {
            if api.mark_force_reload() {
              return Ok(None);
            }
            return Err(NpmResolutionError::Resolution(
              NpmPackageVersionResolutionError::VersionNotFound(err),
            ));
          }
        };

      packages_by_name
        .entry(pkg_id.nv.name.clone())
        .or_default()
        .push(pkg_id.clone());

      packages.push(NpmResolutionPackage {
        copy_index: 0, // this is set below at the end
        id: pkg_id.clone(),
        system: NpmResolutionPackageSystemInfo {
          cpu: version_info.cpu.clone(),
          os: version_info.os.clone(),
        },
        dist: version_info.dist.clone(),
        optional_dependencies: version_info
          .optional_dependencies
          .keys()
          .cloned()
          .collect(),
        extra: Some(crate::NpmPackageExtraInfo {
          bin: version_info.bin.clone(),
          scripts: version_info.scripts.clone(),
          deprecated: version_info.deprecated.clone(),
        }),
        is_deprecated: version_info.deprecated.is_some(),
        has_bin: version_info.bin.is_some(),
        has_scripts: version_info.scripts.contains_key("preinstall")
          || version_info.scripts.contains_key("install")
          || version_info.scripts.contains_key("postinstall"),
        optional_peer_dependencies: version_info
          .peer_dependencies_meta
          .iter()
          .filter(|(_, meta)| meta.optional)
          .map(|(k, _)| k.clone())
          .collect(),
        dependencies,
      });
    }

    Ok(Some(PackagesForSnapshot {
      packages,
      packages_by_name,
      traversed_ids,
    }))
  }

  fn into_snapshot_from_packages(
    mut self,
    packages_to_pkg_ids: &HashMap<NodeId, NpmPackageId>,
    pkgs_for_snapshot: PackagesForSnapshot<'_>,
  ) -> Result<NpmResolutionSnapshot, NpmResolutionError> {
    let PackagesForSnapshot {
      traversed_ids,
      packages,
      packages_by_name,
    } = pkgs_for_snapshot;

    // after traversing, see if there are any copy indexes that
    // need to be updated to a new location based on an id
    // being replaced with a new id
    for (from_id, to_id) in self.moved_package_ids.values() {
      let from_id = self.get_npm_pkg_id_from_resolved_id(from_id);
      let to_id = self.get_npm_pkg_id_from_resolved_id(to_id);
      if !traversed_ids.contains(&from_id)
        && !self.packages_to_copy_index.contains_key(&to_id)
      {
        // move the copy index to the new package
        if let Some(index) = self.packages_to_copy_index.remove(&from_id) {
          self.packages_to_copy_index.insert(to_id, index);
        }
      }
    }

    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::from_map_with_capacity(
        self.packages_to_copy_index,
        self.nodes.len(),
      );
    Ok(NpmResolutionSnapshot {
      root_packages: self
        .root_packages
        .into_iter()
        .map(|(nv, node_id)| {
          (
            (*nv).clone(),
            packages_to_pkg_ids.get(&node_id).unwrap().clone(),
          )
        })
        .collect(),
      packages_by_name: packages_by_name
        .into_iter()
        .map(|(name, mut ids)| {
          ids.sort();
          ids.dedup();
          (name, ids)
        })
        .collect(),
      packages: packages
        .into_iter()
        .map(|mut pkg| {
          pkg.copy_index = copy_index_resolver.resolve(&pkg.id);
          (pkg.id.clone(), pkg)
        })
        .collect(),
      package_reqs: self
        .package_reqs
        .into_iter()
        .map(|(req, nv)| (req, (*nv).clone()))
        .collect(),
    })
  }

  // Debugging methods

  #[cfg(debug_assertions)]
  #[allow(unused, clippy::print_stderr)]
  fn output_path(&self, path: &Rc<GraphPath>) {
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
  #[allow(unused, clippy::print_stderr)]
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
  #[allow(unused, clippy::print_stderr)]
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
struct DepEntryCache(HashMap<Rc<PackageNv>, Rc<Vec<NpmDependencyEntry>>>);

impl DepEntryCache {
  pub fn store(
    &mut self,
    nv: Rc<PackageNv>,
    version_info: &NpmPackageVersionInfo,
  ) -> Result<Rc<Vec<NpmDependencyEntry>>, Box<NpmDependencyEntryError>> {
    debug_assert_eq!(nv.version, version_info.version);
    debug_assert!(!self.0.contains_key(&nv)); // we should not be re-inserting
    let mut deps = version_info.dependencies_as_entries(&nv.name)?;
    // Ensure name alphabetical and then version descending
    // so these are resolved in that order
    deps.sort();
    let deps = Rc::new(deps);
    self.0.insert(nv, deps.clone());
    Ok(deps)
  }

  pub fn get(&self, id: &PackageNv) -> Option<&Rc<Vec<NpmDependencyEntry>>> {
    self.0.get(id)
  }
}

#[derive(Default)]
struct UnresolvedOptionalPeers {
  seen: HashMap<Rc<PackageNv>, Vec<StackString>>,
  seen_count: usize,
}

impl UnresolvedOptionalPeers {
  pub fn has_seen(
    &self,
    parent_nv: &Rc<PackageNv>,
    specifier: &StackString,
  ) -> bool {
    let Some(entries) = self.seen.get(parent_nv) else {
      return false;
    };
    entries.binary_search(specifier).is_ok()
  }

  pub fn mark_seen(
    &mut self,
    parent_nv: Rc<PackageNv>,
    specifier: &StackString,
  ) {
    let entries = self.seen.entry(parent_nv).or_default();
    if let Err(insert_index) = entries.binary_search(specifier) {
      entries.insert(insert_index, specifier.clone());
      self.seen_count += 1;
    }
  }

  pub fn seen_count(&self) -> usize {
    self.seen_count
  }
}

pub struct GraphDependencyResolverOptions {
  pub should_dedup: bool,
}

pub struct GraphDependencyResolver<'a, TNpmRegistryApi: NpmRegistryApi> {
  unmet_peer_diagnostics: RefCell<IndexSet<UnmetPeerDepDiagnostic>>,
  graph: &'a mut Graph,
  api: &'a TNpmRegistryApi,
  version_resolver: &'a NpmVersionResolver,
  pending_unresolved_nodes: VecDeque<Rc<GraphPath>>,
  dep_entry_cache: DepEntryCache,
  reporter: Option<&'a dyn Reporter>,
  should_dedup: bool,
}

impl<'a, TNpmRegistryApi: NpmRegistryApi>
  GraphDependencyResolver<'a, TNpmRegistryApi>
{
  pub fn new(
    graph: &'a mut Graph,
    api: &'a TNpmRegistryApi,
    version_resolver: &'a NpmVersionResolver,
    reporter: Option<&'a dyn Reporter>,
    options: GraphDependencyResolverOptions,
  ) -> Self {
    Self {
      unmet_peer_diagnostics: Default::default(),
      graph,
      api,
      version_resolver,
      pending_unresolved_nodes: Default::default(),
      dep_entry_cache: Default::default(),
      reporter,
      should_dedup: options.should_dedup,
    }
  }

  pub fn add_package_req(
    &mut self,
    package_req: &PackageReq,
    package_info: &NpmPackageInfo,
  ) -> Result<Rc<PackageNv>, NpmResolutionError> {
    if let Some(nv) = self.graph.get_req_nv(package_req) {
      return Ok(nv.clone()); // already added
    }

    // attempt to find an existing root package that matches this package req
    let version_resolver = self.version_resolver.get_for_package(package_info);
    let existing_root = self
      .graph
      .root_packages
      .iter()
      .find(|(nv, _id)| {
        package_req.name == nv.name
          && version_resolver
            .version_req_satisfies(&package_req.version_req, &nv.version)
            .ok()
            .unwrap_or(false)
      })
      .map(|(nv, id)| (nv.clone(), *id));
    let (pkg_nv, node_id) = match existing_root {
      Some(existing) => existing,
      None => {
        let (pkg_nv, node_id) = self.resolve_node_from_info(
          &package_req.name,
          &package_req.version_req,
          &version_resolver,
          None,
        )?;
        self.pending_unresolved_nodes.push_back(GraphPath::for_root(
          node_id,
          pkg_nv.clone(),
          GraphPathResolutionMode::All,
        ));
        (pkg_nv, node_id)
      }
    };
    self
      .graph
      .package_reqs
      .insert(package_req.clone(), pkg_nv.clone());
    self.graph.root_packages.insert(pkg_nv.clone(), node_id);
    Ok(pkg_nv)
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    version_resolver: &NpmPackageVersionResolver,
    parent_path: &Rc<GraphPath>,
  ) -> Result<NodeId, NpmResolutionError> {
    debug_assert_eq!(entry.kind, NpmDependencyEntryKind::Dep);
    let parent_id = parent_path.node_id();
    let (child_nv, mut child_id) = self.resolve_node_from_info(
      &entry.name,
      &entry.version_req,
      version_resolver,
      Some(parent_id),
    )?;
    // Some packages may resolves to themselves as a dependency. If this occurs,
    // just ignore adding these as dependencies because this is likely a mistake
    // in the package.
    if child_nv != parent_path.nv {
      let maybe_ancestor = parent_path.find_ancestor(&child_nv);
      if let Some(ancestor) = &maybe_ancestor {
        child_id = ancestor.node_id();
      }

      let new_path = parent_path.with_id(
        child_id,
        entry.bare_specifier.clone(),
        child_nv,
        GraphPathResolutionMode::All,
      );
      if let Some(ancestor) = maybe_ancestor {
        // this node is circular, so we link it to the ancestor
        self.add_linked_circular_descendant(&ancestor, new_path);
      } else {
        self.graph.set_child_of_parent_node(
          parent_id,
          &entry.bare_specifier,
          child_id,
        );
        self.pending_unresolved_nodes.push_back(new_path);
      }
    }
    Ok(child_id)
  }

  fn resolve_node_from_info(
    &mut self,
    pkg_req_name: &str,
    version_req: &VersionReq,
    version_resolver: &NpmPackageVersionResolver,
    parent_id: Option<NodeId>,
  ) -> Result<(Rc<PackageNv>, NodeId), NpmResolutionError> {
    let info = version_resolver.resolve_best_package_version_info(
      version_req,
      self
        .graph
        .package_name_versions
        .entry(version_resolver.info().name.clone())
        .or_default()
        .iter(),
    )?;
    let resolved_id = ResolvedId {
      nv: Rc::new(PackageNv {
        name: version_resolver.info().name.clone(),
        version: info.version.clone(),
      }),
      peer_dependencies: Vec::new(),
    };
    let (_, node_id) = self.graph.get_or_create_for_id(&resolved_id);
    let pkg_nv = resolved_id.nv;

    let has_deps = if let Some(deps) = self.dep_entry_cache.get(&pkg_nv) {
      !deps.is_empty()
    } else {
      let deps = self.dep_entry_cache.store(pkg_nv.clone(), info)?;
      !deps.is_empty()
    };

    if !has_deps {
      // ensure this is set if not, as it's an optimization
      let node = self.graph.borrow_node_mut(node_id);
      node.no_peers = true;
    }

    debug!(
      "{} - Resolved {}@{} to {}",
      match parent_id {
        Some(parent_id) => self.graph.get_npm_pkg_id(parent_id).as_serialized(),
        None => "<package-req>".into(),
      },
      pkg_req_name,
      version_req.version_text(),
      pkg_nv,
    );

    if let Some(reporter) = &self.reporter {
      let package_req = PackageReq {
        name: pkg_req_name.into(),
        version_req: version_req.clone(),
      };
      reporter.on_resolved(&package_req, &pkg_nv);
    }

    Ok((pkg_nv, node_id))
  }

  pub async fn resolve_pending(&mut self) -> Result<(), NpmResolutionError> {
    let mut did_dedup = false;
    while !self.pending_unresolved_nodes.is_empty() {
      // go down through the dependencies by tree depth
      let mut previous_seen_optional_peers_count = 0;
      while !self.pending_unresolved_nodes.is_empty() {
        while let Some(parent_path) = self.pending_unresolved_nodes.pop_front()
        {
          self.resolve_next_pending(parent_path).await?;
        }

        let seen_optional_peers_count =
          self.graph.unresolved_optional_peers.seen_count();
        if seen_optional_peers_count > previous_seen_optional_peers_count {
          previous_seen_optional_peers_count = seen_optional_peers_count;
          debug!(
            "Traversing graph to ensure newly seen optional peers are set."
          );
          // go through the graph again resolving any optional peers
          for (nv, node_id) in &self.graph.root_packages {
            self.pending_unresolved_nodes.push_back(GraphPath::for_root(
              *node_id,
              nv.clone(),
              GraphPathResolutionMode::OptionalPeers,
            ));
          }
        }
      }

      if self.should_dedup && !did_dedup {
        self.run_dedup_pass().await?;
        did_dedup = true;
      }
    }

    Ok(())
  }

  async fn resolve_next_pending(
    &mut self,
    parent_path: Rc<GraphPath>,
  ) -> Result<(), NpmResolutionError> {
    let (parent_nv, child_deps) = {
      let node_id = parent_path.node_id();
      if self.graph.nodes.get(&node_id).unwrap().no_peers {
        // We can skip as there's no reason to analyze this graph segment further.
        return Ok(());
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
        // the api is expected to have cached this at this point, so no
        // need to parallelize
        let package_info = self.api.package_info(&pkg_nv.name).await?;
        let version_info = package_info
          .version_info(&pkg_nv, &self.version_resolver.link_packages)
          .map_err(NpmPackageVersionResolutionError::VersionNotFound)?;
        self.dep_entry_cache.store(pkg_nv.clone(), version_info)?
      };

      (pkg_nv, deps)
    };

    // resolve the dependencies
    let mut found_peer = false;

    let mut infos = futures::stream::FuturesOrdered::from_iter(
      child_deps
        .iter()
        .map(|dep| self.api.package_info(&dep.name)),
    );

    let mut child_deps_iter = child_deps.iter();
    while let Some(package_info) = infos.next().await {
      let dep = child_deps_iter.next().unwrap();
      let package_info = match package_info {
        Ok(info) => info,
        // npm doesn't fail on non-existent optional peer dependencies
        Err(NpmRegistryPackageInfoLoadError::PackageNotExists { .. })
          if matches!(dep.kind, NpmDependencyEntryKind::OptionalPeer) =>
        {
          continue;
        }
        Err(e) => return Err(e.into()),
      };
      let version_resolver =
        self.version_resolver.get_for_package(&package_info);

      match dep.kind {
        NpmDependencyEntryKind::Dep => {
          let parent_id = parent_path.node_id();
          let node = self.graph.nodes.get(&parent_id).unwrap();
          let child_id = match node.children.get(&dep.bare_specifier) {
            Some(child_id) => {
              // this dependency was previously analyzed by another path
              // so we don't attempt to resolve the version again
              let child_id = *child_id;
              let child_nv = self
                .graph
                .resolved_node_ids
                .get(child_id)
                .unwrap()
                .nv
                .clone();
              let maybe_ancestor = parent_path.find_ancestor(&child_nv);
              let child_path = parent_path.with_id(
                child_id,
                dep.bare_specifier.clone(),
                child_nv,
                parent_path.mode,
              );
              if let Some(ancestor) = maybe_ancestor {
                // when the nv appears as an ancestor, use that node
                // and mark this as circular
                self.add_linked_circular_descendant(&ancestor, child_path);
              } else {
                // mark the child as pending
                self.pending_unresolved_nodes.push_back(child_path);
              }
              child_id
            }
            None => {
              self.analyze_dependency(dep, &version_resolver, &parent_path)?
            }
          };

          #[cfg(feature = "tracing")]
          {
            self.graph.traces.push(build_trace_graph_snapshot(
              self.graph,
              &self.dep_entry_cache,
              &parent_path.with_id(
                child_id,
                dep.bare_specifier.clone(),
                self
                  .graph
                  .resolved_node_ids
                  .get(child_id)
                  .unwrap()
                  .nv
                  .clone(),
                parent_path.mode,
              ),
            ));
          }

          if !found_peer {
            found_peer = !self.graph.borrow_node_mut(child_id).no_peers;
          }
        }
        NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer => {
          found_peer = true;
          let parent_id = parent_path.node_id();
          let node = self.graph.nodes.get(&parent_id).unwrap();
          let previous_nv = node
            .children
            .get(&dep.bare_specifier)
            .and_then(|child_id| self.graph.resolved_node_ids.get(*child_id))
            .map(|child| child.nv.clone());
          // we need to re-evaluate peer dependencies every time and can't
          // skip over them because they might be evaluated differently based
          // on the current path
          let maybe_new_id = self.resolve_peer_dep(
            &dep.bare_specifier,
            dep,
            &version_resolver,
            &parent_path,
            previous_nv.as_ref(),
          )?;

          #[cfg(feature = "tracing")]
          if let Some(child_id) = maybe_new_id {
            self.graph.traces.push(build_trace_graph_snapshot(
              self.graph,
              &self.dep_entry_cache,
              &parent_path.with_id(
                child_id,
                dep.bare_specifier.clone(),
                self
                  .graph
                  .resolved_node_ids
                  .get(child_id)
                  .unwrap()
                  .nv
                  .clone(),
                parent_path.mode,
              ),
            ));
          }

          if dep.kind == NpmDependencyEntryKind::OptionalPeer
            && maybe_new_id.is_some()
          {
            // mark that we've seen it
            self
              .graph
              .unresolved_optional_peers
              .mark_seen(parent_nv.clone(), &dep.bare_specifier);
          }
        }
      }
    }

    if !found_peer {
      self.graph.borrow_node_mut(parent_path.node_id()).no_peers = true;
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    specifier: &StackString,
    peer_dep: &NpmDependencyEntry,
    peer_version_resolver: &NpmPackageVersionResolver,
    ancestor_path: &Rc<GraphPath>,
    previous_nv: Option<&Rc<PackageNv>>,
  ) -> Result<Option<NodeId>, NpmResolutionError> {
    fn get_path(
      index: usize,
      ancestor_path: &Rc<GraphPath>,
    ) -> Vec<&Rc<GraphPath>> {
      let mut path = Vec::with_capacity(index + 2);
      path.push(ancestor_path);
      path.extend(ancestor_path.ancestors().take(index + 1).filter_map(|a| {
        match a {
          GraphPathNodeOrRoot::Node(graph_path) => Some(graph_path),
          GraphPathNodeOrRoot::Root(_) => None,
        }
      }));
      path
    }

    debug_assert!(matches!(
      peer_dep.kind,
      NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer
    ));

    if !peer_dep.kind.is_optional_peer()
      && matches!(ancestor_path.mode, GraphPathResolutionMode::OptionalPeers)
      && let Some(previous_nv) = previous_nv.cloned()
    {
      // don't re-resolve a peer dependency when only going through the graph
      // resolving optional peers
      let node = self.graph.nodes.get(&ancestor_path.node_id()).unwrap();
      let previous_id = node.children.get(&peer_dep.bare_specifier).unwrap();
      let new_path = ancestor_path.with_id(
        *previous_id,
        peer_dep.bare_specifier.clone(),
        previous_nv,
        GraphPathResolutionMode::OptionalPeers,
      );
      self.pending_unresolved_nodes.push_back(new_path);
      return Ok(None);
    }

    // the current dependency might have had the peer dependency
    // in another bare specifier slot... if so resolve it to that
    {
      let maybe_peer_dep = self.find_peer_dep_in_node(
        ancestor_path,
        peer_dep,
        peer_version_resolver,
        // exclude the current resolving specifier so that we don't find the
        // peer dependency in the current slot, which might be out of date
        Some(specifier),
        ancestor_path,
      )?;

      if let Some((peer_parent, peer_dep_id)) = maybe_peer_dep {
        // this will always have an ancestor because we're not at the root
        self.set_new_peer_dep(
          &[ancestor_path],
          peer_parent,
          specifier,
          peer_dep_id,
        );
        return Ok(Some(peer_dep_id));
      }
    }

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut matching_peers_going_up = ancestor_path
      .ancestors()
      .enumerate()
      .filter_map(|(i, ancestor_node)| {
        match ancestor_node {
          GraphPathNodeOrRoot::Node(ancestor_graph_path_node) => {
            let maybe_peer_dep_result = self.find_peer_dep_in_node(
              ancestor_graph_path_node,
              peer_dep,
              peer_version_resolver,
              None,
              ancestor_path,
            );
            match maybe_peer_dep_result {
              Ok(maybe_peer_dep) => {
                maybe_peer_dep.map(|(peer_parent, peer_dep_id)| {
                  let path = get_path(i, ancestor_path);
                  Ok((path, peer_parent, peer_dep_id))
                })
              }
              Err(err) => Some(Err(err)),
            }
          }
          GraphPathNodeOrRoot::Root(root_pkg_nv) => {
            // in this case, the parent is the root so the children are all the package requirements
            let maybe_peer_dep_result = self.find_matching_child_for_peer_dep(
              peer_dep,
              peer_version_resolver,
              self.graph.root_packages.iter().map(|(nv, id)| (*id, nv)),
              ancestor_path,
            );
            match maybe_peer_dep_result {
              Ok(maybe_peer_dep) => maybe_peer_dep.map(|peer_dep_id| {
                let path = get_path(i, ancestor_path);
                let peer_parent =
                  GraphPathNodeOrRoot::Root(root_pkg_nv.clone());
                Ok((path, peer_parent, peer_dep_id))
              }),
              Err(err) => Some(Err(err)),
            }
          }
        }
      });
    let mut found_result = None;
    if let Some(previous_nv) = previous_nv {
      // when this child previously matched to an nv, we want to
      // see if that nv is anywhere in the ancestor peers... if so
      // match to that instead of the closest peer as it reduces
      // duplicate packages
      for item in matching_peers_going_up {
        let item = item?;
        let id = self.graph.resolved_node_ids.get(item.2).unwrap();
        if id.nv == *previous_nv {
          found_result = Some(item);
          break;
        } else if found_result.is_none() {
          found_result = Some(item);
        }
      }
    } else if let Some(found_peer) = matching_peers_going_up.next() {
      found_result = Some(found_peer?);
    }

    if let Some((path, peer_parent, peer_dep_id)) = found_result {
      self.set_new_peer_dep(&path, peer_parent, specifier, peer_dep_id);
      return Ok(Some(peer_dep_id));
    }

    if peer_dep.kind.is_optional_peer() {
      if self
        .graph
        .unresolved_optional_peers
        .has_seen(&ancestor_path.nv, specifier)
      {
        // for optional peer deps that haven't been found, traverse the entire
        // graph searching for the first same nv that uses this
        let mut seen_ids = HashSet::with_capacity(self.graph.nodes.len());
        let mut pending_ids = VecDeque::new();
        for id in self.graph.root_packages.values().copied() {
          if seen_ids.insert(id) {
            pending_ids.push_back(id);
          }
        }
        while let Some(node_id) = pending_ids.pop_front() {
          let Some(node) = self.graph.nodes.get(&node_id) else {
            continue;
          };

          if let Some(id) = self.graph.resolved_node_ids.get(node_id)
            && id.nv == ancestor_path.nv
            && let Some(node_id) = node.children.get(specifier).copied()
          {
            let peer_parent = GraphPathNodeOrRoot::Node(ancestor_path.clone());
            self.set_new_peer_dep(
              &[ancestor_path],
              peer_parent,
              specifier,
              node_id,
            );
            return Ok(Some(node_id));
          }

          for child_id in node.children.values().copied() {
            if seen_ids.insert(child_id) {
              pending_ids.push_back(child_id);
            }
          }
        }
      }
      Ok(None)
    } else {
      // We didn't find anything by searching the ancestor siblings, so we need
      // to resolve based on the package info
      let parent_id = ancestor_path.node_id();
      let (_, node_id) = self.resolve_node_from_info(
        &peer_dep.name,
        peer_dep
          .peer_dep_version_req
          .as_ref()
          .unwrap_or(&peer_dep.version_req),
        peer_version_resolver,
        Some(parent_id),
      )?;
      let peer_parent = GraphPathNodeOrRoot::Node(ancestor_path.clone());
      self.set_new_peer_dep(&[ancestor_path], peer_parent, specifier, node_id);
      Ok(Some(node_id))
    }
  }

  fn find_peer_dep_in_node(
    &self,
    path: &Rc<GraphPath>,
    peer_dep: &NpmDependencyEntry,
    peer_version_resolver: &NpmPackageVersionResolver,
    exclude_key: Option<&str>,
    original_resolving_path: &Rc<GraphPath>,
  ) -> Result<Option<(GraphPathNodeOrRoot, NodeId)>, NpmResolutionError> {
    let node_id = path.node_id();
    let resolved_node_id = self.graph.resolved_node_ids.get(node_id).unwrap();
    // check if this node itself is a match for
    // the peer dependency and if so use that
    if resolved_node_id.nv.name == peer_dep.name {
      if !peer_version_resolver.version_req_satisfies(
        &peer_dep.version_req,
        &resolved_node_id.nv.version,
      )? {
        self.add_unmet_peer_dep_diagnostic(
          original_resolving_path,
          peer_dep,
          resolved_node_id.nv.version.clone(),
        );
      }
      let parent = path.previous_node.as_ref().unwrap().clone();
      Ok(Some((parent, node_id)))
    } else {
      let node = self.graph.nodes.get(&node_id).unwrap();
      let children = node
        .children
        .iter()
        .filter(|(key, _value)| Some(key.as_str()) != exclude_key)
        .map(|(_specifier, child_node_id)| {
          let child_node_id = *child_node_id;
          (
            child_node_id,
            &self.graph.resolved_node_ids.get(child_node_id).unwrap().nv,
          )
        });
      self
        .find_matching_child_for_peer_dep(
          peer_dep,
          peer_version_resolver,
          children,
          original_resolving_path,
        )
        .map(|maybe_child_id| {
          maybe_child_id.map(|child_id| {
            let parent = GraphPathNodeOrRoot::Node(path.clone());
            (parent, child_id)
          })
        })
    }
  }

  fn add_unmet_peer_dep_diagnostic(
    &self,
    original_resolving_path: &Rc<GraphPath>,
    peer_dep: &NpmDependencyEntry,
    resolved_version: Version,
  ) {
    self
      .unmet_peer_diagnostics
      .borrow_mut()
      .insert(UnmetPeerDepDiagnostic {
        ancestors: std::iter::once(original_resolving_path)
          .chain(original_resolving_path.ancestors().filter_map(|n| match n {
            GraphPathNodeOrRoot::Node(path) => Some(path),
            GraphPathNodeOrRoot::Root(_) => None,
          }))
          .filter_map(|node| {
            Some(
              self
                .graph
                .resolved_node_ids
                .get(node.node_id())?
                .nv
                .as_ref()
                .clone(),
            )
          })
          .collect(),
        dependency: PackageReq {
          name: peer_dep.name.clone(),
          version_req: peer_dep.version_req.clone(),
        },
        resolved: resolved_version,
      });
  }

  fn add_peer_deps_to_path(
    &mut self,
    // path from the node above the resolved dep to just above the peer dep
    path: &[&Rc<GraphPath>],
    peer_deps: &[(&ResolvedIdPeerDep, Rc<PackageNv>)],
  ) {
    debug_assert!(!path.is_empty());

    for graph_path_node in path.iter().rev() {
      let old_node_id = graph_path_node.node_id();
      let old_resolved_id =
        self.graph.resolved_node_ids.get(old_node_id).unwrap();

      let Some(new_resolved_id) =
        self.add_peer_deps_to_id(old_resolved_id, peer_deps)
      else {
        continue; // nothing to change
      };

      let old_resolved_id = old_resolved_id.clone();
      let (created, new_node_id) =
        self.graph.get_or_create_for_id(&new_resolved_id);

      if created {
        let old_children =
          self.graph.borrow_node_mut(old_node_id).children.clone();
        // copy over the old children to this new one
        for (specifier, child_id) in &old_children {
          self.graph.set_child_of_parent_node(
            new_node_id,
            specifier,
            *child_id,
          );
        }

        // the moved_package_ids is only used to update copy indexes
        // at the end, so only bother inserting if it's not empty
        if !self.graph.packages_to_copy_index.is_empty() {
          // Store how package ids were moved. The order is important
          // here because one id might be moved around a few times
          let new_value = (old_resolved_id.clone(), new_resolved_id.clone());
          match self.graph.moved_package_ids.entry(old_node_id) {
            indexmap::map::Entry::Occupied(occupied_entry) => {
              // move it the the back of the index map
              occupied_entry.shift_remove();
              self.graph.moved_package_ids.insert(old_node_id, new_value);
            }
            indexmap::map::Entry::Vacant(vacant_entry) => {
              vacant_entry.insert(new_value);
            }
          }
        }
      }

      graph_path_node.change_id(new_node_id);

      let circular_descendants =
        graph_path_node.linked_circular_descendants.borrow().clone();
      for descendant in circular_descendants {
        let path = descendant.get_path_to_ancestor_exclusive(new_node_id);
        self.add_peer_deps_to_path(&path, peer_deps);
        descendant.change_id(new_node_id);

        // update the bottom node to point to this new node id
        let bottom_node_id = path[0].node_id();
        self.graph.set_child_of_parent_node(
          bottom_node_id,
          descendant.specifier(),
          descendant.node_id(),
        );
      }

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
            .insert(graph_path_node.specifier().clone(), new_node_id);
        }
      }
    }
  }

  fn add_peer_deps_to_id(
    &self,
    id: &ResolvedId,
    peer_deps: &[(&ResolvedIdPeerDep, Rc<PackageNv>)],
  ) -> Option<ResolvedId> {
    let mut new_resolved_id = Cow::Borrowed(id);
    let peer_nvs = new_resolved_id
      .peer_dependencies
      .iter()
      .filter_map(|p| {
        self
          .graph
          .peer_dep_to_maybe_node_id_and_resolved_id(p)
          .map(|(_, resolved_id)| resolved_id.nv.clone())
      })
      .collect::<HashSet<_>>();
    for (peer_dep, nv) in peer_deps {
      if *nv == new_resolved_id.nv {
        continue;
      }
      if peer_nvs.contains(nv) {
        continue;
      }
      match &mut new_resolved_id {
        Cow::Borrowed(id) => {
          let mut new_id = (*id).clone();
          new_id.peer_dependencies.push((*peer_dep).clone());
          new_resolved_id = Cow::Owned(new_id);
        }
        Cow::Owned(new_id) => {
          new_id.peer_dependencies.push((*peer_dep).clone());
        }
      }
    }
    match new_resolved_id {
      Cow::Borrowed(_) => None,
      Cow::Owned(id) => Some(id),
    }
  }

  fn set_new_peer_dep(
    &mut self,
    // path from the node above the resolved dep to just above the peer dep
    path: &[&Rc<GraphPath>],
    peer_dep_parent: GraphPathNodeOrRoot,
    peer_dep_specifier: &StackString,
    peer_dep_id: NodeId,
  ) {
    debug_assert!(!path.is_empty());
    let peer_dep_nv = self
      .graph
      .resolved_node_ids
      .get(peer_dep_id)
      .unwrap()
      .nv
      .clone();

    let peer_dep = ResolvedIdPeerDep::ParentReference {
      parent: peer_dep_parent,
      child_pkg_nv: peer_dep_nv.clone(),
    };

    let top_node = path.last().unwrap();
    let (maybe_circular_ancestor, path) = if top_node.nv == peer_dep_nv {
      // it's circular, so exclude the top node
      (Some(top_node), &path[0..path.len() - 1])
    } else {
      (None, path)
    };

    if path.is_empty() {
      // the peer dep is the same as the parent, so we don't need to do anything
      return;
    }

    self.add_peer_deps_to_path(path, &[(&peer_dep, peer_dep_nv.clone())]);

    // now set the peer dependency
    let bottom_node = path.first().unwrap();
    self.graph.set_child_of_parent_node(
      bottom_node.node_id(),
      peer_dep_specifier,
      peer_dep_id,
    );

    // queue next step
    let new_path = bottom_node.with_id(
      peer_dep_id,
      peer_dep_specifier.clone(),
      peer_dep_nv,
      GraphPathResolutionMode::All,
    );
    if let Some(ancestor_node) = maybe_circular_ancestor {
      // it's circular, so link this in step with the ancestor node
      ancestor_node
        .linked_circular_descendants
        .borrow_mut()
        .push(new_path);
    } else {
      // mark the peer dep as needing to be analyzed
      self.pending_unresolved_nodes.push_back(new_path);
    }

    debug!(
      "Resolved peer dependency for {} in {} to {}",
      peer_dep_specifier,
      &self
        .graph
        .get_npm_pkg_id(bottom_node.node_id())
        .as_serialized(),
      &self.graph.get_npm_pkg_id(peer_dep_id).as_serialized(),
    );
  }

  fn add_linked_circular_descendant(
    &mut self,
    ancestor: &Rc<GraphPath>,
    descendant: Rc<GraphPath>,
  ) {
    let ancestor_node_id = ancestor.node_id();
    let path = descendant.get_path_to_ancestor_exclusive(ancestor_node_id);

    let ancestor_resolved_id = self
      .graph
      .resolved_node_ids
      .get(ancestor_node_id)
      .unwrap()
      .clone();

    let peer_deps = ancestor_resolved_id
      .peer_dependencies
      .iter()
      .map(|peer_dep| {
        (
          peer_dep,
          match &peer_dep {
            ResolvedIdPeerDep::ParentReference { child_pkg_nv, .. } => {
              child_pkg_nv.clone()
            }
            ResolvedIdPeerDep::SnapshotNodeId(node_id) => self
              .graph
              .resolved_node_ids
              .get(*node_id)
              .unwrap()
              .nv
              .clone(),
          },
        )
      })
      .collect::<Vec<_>>();
    if !peer_deps.is_empty() {
      self.add_peer_deps_to_path(&path, &peer_deps);
    }

    let bottom_node_id = path[0].node_id();
    self.graph.set_child_of_parent_node(
      bottom_node_id,
      descendant.specifier(),
      descendant.node_id(),
    );

    ancestor
      .linked_circular_descendants
      .borrow_mut()
      .push(descendant);
  }

  fn find_matching_child_for_peer_dep<'nv>(
    &self,
    peer_dep: &NpmDependencyEntry,
    peer_version_resolver: &NpmPackageVersionResolver,
    children: impl Iterator<Item = (NodeId, &'nv Rc<PackageNv>)>,
    original_resolving_path: &Rc<GraphPath>,
  ) -> Result<Option<NodeId>, NpmResolutionError> {
    for (child_id, pkg_id) in children {
      if pkg_id.name == peer_dep.name {
        if !peer_version_resolver
          .version_req_satisfies(&peer_dep.version_req, &pkg_id.version)?
        {
          self.add_unmet_peer_dep_diagnostic(
            original_resolving_path,
            peer_dep,
            pkg_id.version.clone(),
          );
        }
        return Ok(Some(child_id));
      }
    }
    Ok(None)
  }

  pub fn take_unmet_peer_diagnostics(&self) -> Vec<UnmetPeerDepDiagnostic> {
    self.unmet_peer_diagnostics.borrow_mut().drain(..).collect()
  }

  async fn run_dedup_pass(&mut self) -> Result<(), NpmResolutionError> {
    debug!("Running npm dedup pass.");
    type VersionReqsByVersion = BTreeMap<Version, Vec<VersionReq>>;
    let mut package_version_reqs_by_version: HashMap<
      PackageName,
      VersionReqsByVersion,
    > = HashMap::with_capacity(self.graph.nodes.len());
    let mut seen_nodes: HashSet<NodeId> =
      HashSet::with_capacity(self.graph.nodes.len());
    let mut pending_nodes: VecDeque<NodeId> = Default::default();

    for (req, pkg_nv) in &self.graph.package_reqs {
      if let Some(node_id) = self.graph.root_packages.get(pkg_nv) {
        package_version_reqs_by_version
          .entry(req.name.clone())
          .or_default()
          .entry(pkg_nv.version.clone())
          .or_default()
          .push(req.version_req.clone());
        if seen_nodes.insert(*node_id) {
          pending_nodes.push_back(*node_id);
        }
      }
    }

    let mut futures = FuturesUnordered::new();
    let mut pending_dep_entries = VecDeque::new();
    while !pending_nodes.is_empty() || !futures.is_empty() {
      for node_id in pending_nodes.drain(..) {
        let Some(nv) = self
          .graph
          .resolved_node_ids
          .get(node_id)
          .map(|id| id.nv.clone())
        else {
          continue;
        };
        if let Some(deps) = self.dep_entry_cache.get(&nv) {
          pending_dep_entries.push_back((node_id, deps.clone()));
        } else {
          let api = self.api;
          futures.push(async move {
            let package_info = api.package_info(&nv.name).await?;
            Result::<_, NpmResolutionError>::Ok((node_id, nv, package_info))
          });
        }
      }

      if let Some(result) = futures.next().await {
        let (node_id, nv, package_info) = result?;
        let version_info = package_info
          .version_info(&nv, &self.version_resolver.link_packages)
          .map_err(NpmPackageVersionResolutionError::VersionNotFound)?;
        let deps = self.dep_entry_cache.store(nv.clone(), version_info)?;
        pending_dep_entries.push_back((node_id, deps));
      }

      while let Some((node_id, deps)) = pending_dep_entries.pop_front() {
        if let Some(node) = self.graph.nodes.get(&node_id) {
          for dep in deps.iter() {
            if let Some(child_node_id) = node.children.get(&dep.bare_specifier)
            {
              let child_id =
                self.graph.resolved_node_ids.get(*child_node_id).unwrap();
              package_version_reqs_by_version
                .entry(child_id.nv.name.clone())
                .or_default()
                .entry(child_id.nv.version.clone())
                .or_default()
                .push(dep.version_req.clone());
              if seen_nodes.insert(*child_node_id) {
                pending_nodes.push_back(*child_node_id);
              }
            }
          }
        }
      }
    }

    let mut consolidated_versions: BTreeMap<
      PackageName,
      HashMap<VersionReq, Version>,
    > = Default::default();

    for (package_name, reqs_by_version) in package_version_reqs_by_version {
      if reqs_by_version.len() <= 1 {
        continue;
      }
      let final_versions = self
        .assign_highest_satisfying(&package_name, &reqs_by_version)
        .await;
      if !final_versions.is_empty() {
        // update the graph to only have the new versions in it
        if let Some(versions) =
          self.graph.package_name_versions.get_mut(&package_name)
        {
          versions
            .retain(|version| final_versions.values().any(|v| v == version));
        }

        consolidated_versions.insert(package_name, final_versions);
      }
    }

    if consolidated_versions.is_empty() {
      return Ok(()); // nothing to do
    }

    debug!("Consolidating npm versions.");

    if log::log_enabled!(log::Level::Debug) {
      for (package_name, versions_by_version_req) in &consolidated_versions {
        for (version_req, version) in versions_by_version_req {
          debug!("{}: {} -> {}", package_name, version_req, version);
        }
      }
    }

    // set the root package reqs
    let mut added_root_package_ids = Vec::new();
    let mut maybe_root_nvs_to_remove = Vec::new();
    for (pkg_req, pkg_nv) in &mut self.graph.package_reqs {
      if let Some(new_versions) = consolidated_versions.get(&pkg_req.name)
        && let Some(new_version) = new_versions.get(&pkg_req.version_req)
        && pkg_nv.version != *new_version
      {
        maybe_root_nvs_to_remove.push(pkg_nv.clone());
        *pkg_nv = Rc::new(PackageNv {
          name: pkg_nv.name.clone(),
          version: new_version.clone(),
        });
        let resolved_id = ResolvedId {
          nv: pkg_nv.clone(),
          peer_dependencies: Vec::new(),
        };
        added_root_package_ids.push(resolved_id);
      }
    }

    // set the root package nvs
    for resolved_id in added_root_package_ids {
      let (_, node_id) = self.graph.get_or_create_for_id(&resolved_id);
      self.graph.root_packages.insert(resolved_id.nv, node_id);
    }

    // remove any root packages no longer in the reqs
    for pkg_nv in &maybe_root_nvs_to_remove {
      if !self.graph.package_reqs.values().any(|v| v == pkg_nv) {
        self.graph.root_packages.remove(pkg_nv);
      }
    }

    // now go through each node clearing it out
    for (node_id, node) in &mut self.graph.nodes {
      node.no_peers = false; // reset
      let Some(id) = self.graph.resolved_node_ids.get(*node_id) else {
        continue;
      };
      let Some(deps) = self.dep_entry_cache.get(&id.nv) else {
        continue;
      };
      for dep in deps.iter() {
        let Some(child_node_id) = node.children.get(&dep.bare_specifier) else {
          continue;
        };
        let child_id =
          self.graph.resolved_node_ids.get(*child_node_id).unwrap();
        let Some(versions) = consolidated_versions.get(&child_id.nv.name)
        else {
          continue;
        };
        if versions.contains_key(&dep.version_req) {
          node.children.remove(&dep.bare_specifier);
        }
      }
    }

    // reset some details
    self.graph.unresolved_optional_peers = Default::default();
    self.graph.resolved_node_ids.clear_peer_deps();
    self.graph.moved_package_ids.clear();
    self.unmet_peer_diagnostics.borrow_mut().clear();
    self.graph.packages_to_copy_index.clear(); // ok because we haven't started running code yet

    // add the pending nodes from the root
    for (pkg_nv, node_id) in &self.graph.root_packages {
      self.pending_unresolved_nodes.push_back(GraphPath::for_root(
        *node_id,
        pkg_nv.clone(),
        GraphPathResolutionMode::All,
      ));
    }

    Ok(())
  }

  async fn assign_highest_satisfying(
    &self,
    package_name: &PackageName,
    by_version: &BTreeMap<Version, Vec<VersionReq>>,
  ) -> HashMap<VersionReq, Version> {
    // this should already be cached
    let package_info = self.api.package_info(package_name).await.unwrap();
    let version_resolver = self.version_resolver.get_for_package(&package_info);

    // collect unique reqs across all versions
    let reqs = by_version
      .values()
      .flat_map(|rs| rs.iter())
      .collect::<HashSet<_>>();

    // candidate versions = keys of by_version, highest -> lowest
    let mut candidates: Vec<Version> = by_version.keys().cloned().collect();
    candidates.sort_by(|a, b| b.cmp(a));

    // try one global winner
    if let Some(global) = candidates.iter().find(|v| {
      reqs.iter().all(|r| {
        version_resolver
          .version_req_satisfies(r, v)
          .ok()
          .unwrap_or(false)
      })
    }) {
      return reqs
        .iter()
        .map(|r| ((*r).clone(), global.clone()))
        .collect();
    }

    // otherwise, use highest-first per-range
    let mut unassigned = reqs;
    let mut assigned: HashMap<VersionReq, Version> =
      HashMap::with_capacity(unassigned.len());

    for v in candidates.into_iter() {
      // assign all still-unassigned reqs that accept this version
      let matching = unassigned
        .iter()
        .filter(|r| {
          version_resolver
            .version_req_satisfies(r, &v)
            .ok()
            .unwrap_or(false)
        })
        .map(|v| (*v).clone())
        .collect::<Vec<_>>();

      if matching.is_empty() {
        continue;
      }

      for r in matching {
        unassigned.remove(&r);
        assigned.insert(r, v.clone());
      }

      if unassigned.is_empty() {
        break;
      }
    }

    assigned
  }
}

#[cfg(feature = "tracing")]
fn build_trace_graph_snapshot(
  graph: &Graph,
  dep_entry_cache: &DepEntryCache,
  current_path: &GraphPath,
) -> super::tracing::TraceGraphSnapshot {
  use super::tracing::*;

  fn build_path(current_path: &GraphPath) -> TraceGraphPath {
    TraceGraphPath {
      specifier: current_path.specifier.to_string(),
      node_id: current_path.node_id().0,
      nv: current_path.nv.to_string(),
      previous: current_path.previous_node.as_ref().and_then(|n| match n {
        GraphPathNodeOrRoot::Node(graph_path) => {
          Some(Box::new(build_path(graph_path)))
        }
        GraphPathNodeOrRoot::Root(_) => None,
      }),
    }
  }

  TraceGraphSnapshot {
    nodes: graph
      .nodes
      .iter()
      .map(|(node_id, node)| {
        let id = graph.get_npm_pkg_id(*node_id);
        TraceNode {
          id: node_id.0,
          resolved_id: id.as_serialized().to_string(),
          children: node
            .children
            .iter()
            .map(|(k, v)| (k.to_string(), v.0))
            .collect(),
          dependencies: dep_entry_cache
            .get(&id.nv)
            .map(|d| {
              d.iter()
                .map(|dep| TraceNodeDependency {
                  kind: format!("{:?}", dep.kind),
                  bare_specifier: dep.bare_specifier.to_string(),
                  name: dep.name.to_string(),
                  version_req: dep.version_req.to_string(),
                  peer_dep_version_req: dep
                    .peer_dep_version_req
                    .as_ref()
                    .map(|r| r.to_string()),
                })
                .collect()
            })
            .unwrap_or_default(),
        }
      })
      .collect(),
    roots: graph
      .root_packages
      .iter()
      .map(|(nv, id)| (nv.to_string(), id.0))
      .collect(),
    path: build_path(current_path),
  }
}

#[cfg(test)]
mod test {
  use std::collections::BTreeSet;
  use std::sync::Arc;

  use pretty_assertions::assert_eq;

  use super::*;
  use crate::NpmSystemInfo;
  use crate::registry::NpmDependencyEntryErrorSource;
  use crate::registry::TestNpmRegistryApi;
  use crate::resolution::NewestDependencyDate;
  use crate::resolution::NewestDependencyDateOptions;
  use crate::resolution::NpmPackageVersionNotFound;
  use crate::resolution::SerializedNpmResolutionSnapshot;

  #[test]
  fn resolved_id_tests() {
    let mut ids = ResolvedNodeIds::default();
    let node_id = NodeId(0);
    let resolved_id = ResolvedId {
      nv: Rc::new(PackageNv::from_str("package@1.1.1").unwrap()),
      peer_dependencies: Vec::new(),
    };
    ids.set(node_id, resolved_id.clone());
    assert!(ids.get(node_id).is_some());
    assert!(ids.get(NodeId(1)).is_none());
    assert_eq!(ids.get_node_id(&resolved_id), Some(node_id));

    let resolved_id_new = ResolvedId {
      nv: Rc::new(PackageNv::from_str("package@1.1.2").unwrap()),
      peer_dependencies: Vec::new(),
    };
    ids.set(node_id, resolved_id_new.clone());
    assert_eq!(ids.get_node_id(&resolved_id), None); // stale entry should have been removed
    assert!(ids.get(node_id).is_some());
    assert_eq!(ids.get_node_id(&resolved_id_new), Some(node_id));
  }

  #[tokio::test]
  async fn resolve_deps_no_peer() {
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@2.0.0".to_string(),),
            ("package-c".to_string(), "package-c@0.1.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@0.1.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-d".to_string(),
            "package-d@3.2.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@3.2.1".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_dependency(("package-b", "2.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@2.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn skips_bundle_dependencies() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.add_bundle_dependency(("package-a", "1.0.0"), ("package-b", "1"));

    let (packages, _package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::new(),
      },]
    );
  }

  #[tokio::test]
  async fn peer_deps_simple_top_tree() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["package-a@1.0", "package-peer@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-0", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@1.0.0".to_string(),)
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-1".to_string(),
            "package-1@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-1@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@1.0.0".to_string(),)
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
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
      vec!["package-a@1", "package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer@4.0.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer@4.0.0".to_string(),
            ),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@4.0.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-0@1.1.1"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.1.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-peer@4.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer@4.0.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer@4.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@4.0.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@4.0.0".to_string(),
          copy_index: 0,
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
  async fn resolve_with_peer_deps_non_matching_version() {
    let api = TestNpmRegistryApi::default();
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
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-peer", "1"));
    api.add_peer_dependency(("package-c", "3.0.0"), ("package-peer", "1"));

    let (packages, package_reqs) =
      run_resolver_with_options_and_get_output(
        api,
        RunResolverOptions {
          reqs: vec!["package-0@1.1.1"],
          expected_diagnostics: vec![
            "package-0@1.1.1 -> package-a@1.0.0 -> package-b@2.0.0: package-peer@1 -> 4.0.0",
            "package-0@1.1.1 -> package-a@1.0.0 -> package-c@3.0.0: package-peer@1 -> 4.0.0"
          ],
          ..Default::default()
        },
      )
      .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.1.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-peer@4.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer@4.0.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer@4.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@4.0.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@4.0.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer@4.1.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer@4.1.0".to_string(),
            ),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer@4.1.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.1.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer@4.1.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.1.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@4.1.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@2.0.0".to_string(),),
            ("package-c".to_string(), "package-c@3.0.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0".to_string(),
          copy_index: 0,
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
  async fn resolve_with_optional_peer_found() {
    let api = TestNpmRegistryApi::default();
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
      vec!["package-a@1", "package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer@4.0.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer@4.0.0".to_string(),
            ),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer@4.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@4.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@4.0.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.ensure_package_version("package-peer-unresolved", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "^1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "^1"));
    api.add_optional_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-peer", "*"),
    );
    api.add_optional_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-peer-unresolved", "*"),
    );

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1", "package-b@1"])
        .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@1.0.0_package-peer@1.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@1.0.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");
    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "*"),
    );
    api.add_dependency(("package-b", "1.0.0"), ("package-a", "1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "2.0.0"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1", "package-b@1"])
        .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@2.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@2.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@2.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@2.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@2.0.0".to_string(),)
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@2.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "*"),
    );

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1", "package-peer@1"])
        .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
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

    let input_reqs = vec!["package-a@1", "package-b@1", "package-peer@1.0.0"];
    let expected_packages = vec![
      TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer".to_string(),
          "package-peer@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0_package-peer@2.0.0".to_string(),
        copy_index: 1,
        dependencies: BTreeMap::from([(
          "package-peer".to_string(),
          "package-peer@2.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-b@1.0.0_package-peer@2.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
          ("package-peer".to_string(), "package-peer@2.0.0".to_string()),
          (
            "package-a".to_string(),
            "package-a@1.0.0_package-peer@2.0.0".to_string(),
          ),
        ]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: Default::default(),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-peer@2.0.0".to_string(),
        copy_index: 0,
        dependencies: Default::default(),
      },
    ];
    let expected_reqs = vec![
      (
        "package-a@1".to_string(),
        "package-a@1.0.0_package-peer@1.0.0".to_string(),
      ),
      (
        "package-b@1".to_string(),
        "package-b@1.0.0_package-peer@2.0.0".to_string(),
      ),
      (
        "package-peer@1.0.0".to_string(),
        "package-peer@1.0.0".to_string(),
      ),
    ];
    // skipping dedup
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup: true,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(packages, expected_packages);
      assert_eq!(package_reqs, expected_reqs);
    }
    // doing dedup
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(packages, expected_packages);
      assert_eq!(package_reqs, expected_reqs);
    }
  }

  #[tokio::test]
  async fn resolve_peer_dep_other_specifier_slot() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");
    // bit of an edge case... probably nobody has ever done this
    api.add_dependency(
      ("package-a", "1.0.0"),
      ("package-peer2", "npm:package-peer@2"),
    );
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "2"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@2.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-peer".to_string(), "package-peer@2.0.0".to_string(),),
            (
              "package-peer2".to_string(),
              "package-peer@2.0.0".to_string(),
            ),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@2.0.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-peer-a", "2.0.0");
    api.ensure_package_version("package-peer-b", "3.0.0");
    api.add_peer_dependency(("package-0", "1.0.0"), ("package-peer-a", "2"));
    api.add_peer_dependency(
      ("package-peer-a", "2.0.0"),
      ("package-peer-b", "3"),
    );

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-a".to_string(),
            "package-peer-a@2.0.0_package-peer-b@3.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-a@2.0.0_package-peer-b@3.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-b".to_string(),
            "package-peer-b@3.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-b@3.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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
    let api = TestNpmRegistryApi::default();
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
      vec!["package-0@1.0", "package-peer-a@2", "package-peer-b@3"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-peer-a".to_string(),
              "package-peer-a@2.0.0_package-peer-b@3.0.0".to_string(),
            ),
            (
              "package-peer-b".to_string(),
              "package-peer-b@3.0.0".to_string(),
            )
          ]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-a@2.0.0_package-peer-b@3.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-b".to_string(),
            "package-peer-b@3.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-b@3.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),

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
    let api = TestNpmRegistryApi::default();
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

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-0@1.1.1", "package-e@3"])
        .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.1.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1".to_string(),
            ),
            (
              "package-d".to_string(),
              "package-d@3.5.0".to_string(),
            ),
            (
              "package-peer-a".to_string(),
              "package-peer-a@4.0.0_package-peer-b@5.4.1".to_string(),
            ),
          ]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-peer-a".to_string(),
              "package-peer-a@4.0.0_package-peer-b@5.4.1".to_string(),
            ),
            (
              "package-peer-c".to_string(),
              "package-peer-c@6.2.0".to_string(),
            )
          ])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-a".to_string(),
            "package-peer-a@4.0.0_package-peer-b@5.4.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@3.5.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-e@3.6.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-a@4.0.0_package-peer-b@5.4.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-b".to_string(),
            "package-peer-b@5.4.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-b@5.4.1".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-c@6.2.0".to_string(),
          copy_index: 0,
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@2.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
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
      let api = TestNpmRegistryApi::default();
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

      let (packages, package_reqs) =
        run_resolver_and_get_output(api, vec!["package-a@1", "package-b@2"])
          .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@4.0.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-dep".to_string(),
                "package-dep@3.0.0_package-peer@4.0.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@4.0.0".to_string(),),
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@2.0.0_package-peer@5.0.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-dep".to_string(),
                "package-dep@3.0.0_package-peer@5.0.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@5.0.0".to_string(),),
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-dep@3.0.0_package-peer@4.0.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@4.0.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-dep@3.0.0_package-peer@5.0.0".to_string(),
            copy_index: 1,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@5.0.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@4.0.0".to_string(),
            copy_index: 0,
            dependencies: Default::default(),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@5.0.0".to_string(),
            copy_index: 0,
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
            "package-b@2".to_string(),
            "package-b@2.0.0_package-peer@5.0.0".to_string()
          )
        ]
      );
    }
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_dep_then_peer() {
    // a -> c -> b (peer)
    //   -> peer
    // b -> peer
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0", "package-b@1.0"])
        .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0_package-peer@1.0.0"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-c".to_string(),
              "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0_package-peer@1.0.0".to_string(),
            ),
            ("package-peer".to_string(), "package-peer@1.0.0".to_string(),)
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.0_package-peer@1.0.0"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-a@1.0".to_string(),
          "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0_package-peer@1.0.0".to_string()
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.1.0");
    api.ensure_package_version("package-peer", "1.2.0");
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "*")); // should select 1.2.0, then 1.1.0
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "=1.1.0"));
    api.add_dependency(("package-c", "1.0.0"), ("package-a", "1"));

    let input_reqs = vec!["package-a@1.0", "package-b@1.0"];
    // before deduping
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup: true,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 1,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.2.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.2.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-c".to_string(),
                "package-c@1.0.0_package-peer@1.1.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@1.1.0".to_string(),)
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-c@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.2.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
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
    // deduping
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api,
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-c".to_string(),
                "package-c@1.0.0_package-peer@1.1.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@1.1.0".to_string(),)
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-c@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
          },
        ]
      );
      assert_eq!(
        package_reqs,
        vec![
          (
            "package-a@1.0".to_string(),
            "package-a@1.0.0_package-peer@1.1.0".to_string()
          ),
          (
            "package-b@1.0".to_string(),
            "package-b@1.0.0_package-peer@1.1.0".to_string()
          )
        ]
      );
    }
  }

  #[tokio::test]
  async fn resolve_dep_and_peer_dist_tag() {
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-d@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@2.0.0".to_string(),),
            (
              "package-c".to_string(),
              "package-c@1.0.0_package-d@1.0.0".to_string(),
            ),
            ("package-d".to_string(), "package-d@1.0.0".to_string(),),
            ("package-e".to_string(), "package-e@1.0.0".to_string(),),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-d@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-d".to_string(),
            "package-d@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-e@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@2.0.0".to_string(),
          )]),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0".to_string(),
        copy_index: 0,
        // in this case, we just ignore that the package did this
        dependencies: Default::default(),
      }]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn package_has_self_but_different_version_as_dependency() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-a", "0.5.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-a", "^0.5"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@0.5.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@0.5.0".to_string(),
          )]),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "2"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@2.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
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
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-0", "1.0.0");
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "2.0.0");
    api.add_dependency(("package-0", "1.0.0"), ("package-a", "*"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "2"));
    api.add_peer_dependency(("package-b", "2.0.0"), ("package-a", "*"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@2.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@2.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
        }
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-0@1.0".to_string(), "package-0@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_peer_deps_in_ancestor_root() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_peer_deps_in_ancestor_non_root() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn nested_deps_same_peer_dep_ancestor() {
    let api = TestNpmRegistryApi::default();
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
      run_resolver_and_get_output(api, vec!["package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-0@1.0.0".to_string(),
          ), (
            "package-1".to_string(),
            "package-1@1.0.0_package-0@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-1@1.0.0_package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-0@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-0".to_string(),
              "package-0@1.0.0".to_string(),
            ),
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-0@1.0.0".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
            )
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-0".to_string(),
              "package-0@1.0.0".to_string(),
            ),
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-0@1.0.0".to_string(),
            ),
            (
              "package-d".to_string(),
              "package-d@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
            )
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0_package-0@1.0.0_package-a@1.0.0__package-0@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-0".to_string(),
              "package-0@1.0.0".to_string(),
            ),
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-0@1.0.0".to_string(),
            )
          ]),
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
    let api = TestNpmRegistryApi::default();
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
      vec!["package-0@1.0", "package-peer@1.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-0@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-1".to_string(),
              "package-1@1.0.0_package-peer@1.0.0".to_string(),
            ),
            (
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.0.0".to_string(),
            )
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-1@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
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

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_circular_1() {
    // a -> b -> c -> d -> c where c has a peer dependency on b
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_dependency(("package-d", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@1.0.0".to_string(),),
            (
              "package-d".to_string(),
              "package-d@1.0.0_package-b@1.0.0".to_string(),
            )
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0_package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_circular_2() {
    // a -> b -> c -> d -> c where c has a peer dependency on b
    //             -> e -> f -> d -> c where f has a peer dep on a
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.ensure_package_version("package-f", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-e", "1"));
    api.add_dependency(("package-d", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-e", "1.0.0"), ("package-f", "1"));
    api.add_dependency(("package-f", "1.0.0"), ("package-d", "1"));
    api.add_peer_dependency(("package-f", "1.0.0"), ("package-a", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@1.0.0_package-a@1.0.0".to_string(),
            ),
            (
              "package-d".to_string(),
              "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
            ),
            (
              "package-e".to_string(),
              "package-e@1.0.0_package-a@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string()
            )
          ]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-e@1.0.0_package-a@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-f".to_string(),
            "package-f@1.0.0_package-a@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-f@1.0.0_package-a@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string(),
          ), (
            "package-d".to_string(),
            "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_dep_with_peer_deps_circular_3() {
    // a -> b -> c -> d -> c (peer)
    //                  -> e -> a (peer)
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_dependency(("package-d", "1.0.0"), ("package-e", "1"));
    api.add_peer_dependency(("package-d", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-e", "1.0.0"), ("package-a", "1"));

    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-a@1.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-d".to_string(),
            "package-d@1.0.0_package-c@1.0.0__package-a@1.0.0_package-a@1.0.0"
              .to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id:
            "package-d@1.0.0_package-c@1.0.0__package-a@1.0.0_package-a@1.0.0"
              .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-c".to_string(),
              "package-c@1.0.0_package-a@1.0.0".to_string(),
            ),
            (
              "package-e".to_string(),
              "package-e@1.0.0_package-a@1.0.0".to_string()
            ),
          ]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-e@1.0.0_package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0".to_string()
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_sibling_peer_deps() {
    // a -> b -> peer c
    //   -> c -> peer b
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "1"));

    let expected_packages = vec![
      TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0_package-c@1.0.0__package-b@1.0.0___package-c@1.0.0_package-b@1.0.0__package-c@1.0.0___package-b@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
          (
            "package-b".to_string(),
            "package-b@1.0.0_package-c@1.0.0__package-b@1.0.0".to_string(),
          ),
          (
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0__package-c@1.0.0".to_string(),
          )
        ])
      },
      TestNpmResolutionPackage {
        // This is stored like so:
        //   b (id: 0) -> c (id: 1) -> b (id: 0)
        // So it's circular. Storing a circular dependency serialized here is a
        // little difficult, so when this is encountered we assume it's circular.
        // I have a feeling this is not exactly correct, but perhaps it is good enough
        // and edge cases won't be seen in the wild...
        pkg_id: "package-b@1.0.0_package-c@1.0.0__package-b@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-c".to_string(),
          "package-c@1.0.0_package-b@1.0.0__package-c@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-c@1.0.0_package-b@1.0.0__package-c@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-b".to_string(),
          "package-b@1.0.0_package-c@1.0.0__package-b@1.0.0".to_string(),
        )]),
      },
    ];
    let (packages, package_reqs) =
      run_resolver_and_get_output(api.clone(), vec!["package-a@1.0.0"]).await;
    assert_eq!(packages, expected_packages.clone());
    assert_eq!(
      package_reqs,
      vec![(
        "package-a@1.0.0".to_string(),
        "package-a@1.0.0_package-c@1.0.0__package-b@1.0.0___package-c@1.0.0_package-b@1.0.0__package-c@1.0.0___package-b@1.0.0".to_string()
      )]
    );

    // now try with b at the top level
    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["package-a@1.0.0", "package-b@1.0.0"],
    )
    .await;
    assert_eq!(packages, expected_packages.clone());
    assert_eq!(
      package_reqs,
      vec![(
        "package-a@1.0.0".to_string(),
        "package-a@1.0.0_package-c@1.0.0__package-b@1.0.0___package-c@1.0.0_package-b@1.0.0__package-c@1.0.0___package-b@1.0.0".to_string()
      ), (
        "package-b@1.0.0".to_string(),
        "package-b@1.0.0_package-c@1.0.0__package-b@1.0.0".to_string()
      )]
    );
  }

  #[tokio::test]
  async fn dep_depending_on_self_when_has_peer_deps() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "*"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "*"));
    api.add_dependency(("package-c", "1.0.0"), ("package-c", "1.0.0"));
    let (packages, package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_optional_deps() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dep_and_optional_dep(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_optional_dep(("package-d", "1.0.0"), ("package-e", "1"));
    api.with_version_info(("package-c", "1.0.0"), |info| {
      info.os = vec!["win32".into(), "darwin".into()];
    });
    api.with_version_info(("package-e", "1.0.0"), |info| {
      info.os = vec!["win32".into()];
    });

    let snapshot =
      run_resolver_and_get_snapshot(api, vec!["package-a@1.0.0"]).await;
    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "win32".into(),
        cpu: "x86".into(),
      },
    );
    assert_eq!(
      packages,
      vec![
        "package-a@1.0.0".to_string(),
        "package-b@1.0.0".to_string(),
        "package-c@1.0.0".to_string(),
        "package-d@1.0.0".to_string(),
        "package-e@1.0.0".to_string(),
      ]
    );

    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "darwin".into(),
        cpu: "x86".into(),
      },
    );
    assert_eq!(
      packages,
      vec![
        "package-a@1.0.0".to_string(),
        "package-b@1.0.0".to_string(),
        "package-c@1.0.0".to_string(),
        "package-d@1.0.0".to_string(),
      ]
    );

    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "linux".into(),
        cpu: "x86".into(),
      },
    );
    assert_eq!(
      packages,
      vec!["package-a@1.0.0".to_string(), "package-b@1.0.0".to_string()]
    );
  }

  #[tokio::test]
  async fn resolve_optional_to_required() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b1", "1.0.0");
    api.ensure_package_version("package-b2", "1.0.0");
    api.ensure_package_version("package-b3", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b1", "1"));
    api.add_dependency(("package-b1", "1.0.0"), ("package-b2", "1"));
    api.add_dependency(("package-b2", "1.0.0"), ("package-b3", "1"));
    // deep down this is set back to being required, so it and its required
    // dependency should be marked as required
    api.add_dependency(("package-b3", "1.0.0"), ("package-c", "1"));
    api.add_dep_and_optional_dep(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_dep_and_optional_dep(("package-d", "1.0.0"), ("package-e", "1"));

    api.with_version_info(("package-c", "1.0.0"), |info| {
      info.os = vec!["win32".into()];
    });
    api.with_version_info(("package-e", "1.0.0"), |info| {
      info.os = vec!["win32".into()];
    });

    let snapshot =
      run_resolver_and_get_snapshot(api, vec!["package-a@1.0.0"]).await;

    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "darwin".into(),
        cpu: "x86".into(),
      },
    );
    assert_eq!(
      packages,
      vec![
        "package-a@1.0.0".to_string(),
        "package-b1@1.0.0".to_string(),
        "package-b2@1.0.0".to_string(),
        "package-b3@1.0.0".to_string(),
        "package-c@1.0.0".to_string(),
        "package-d@1.0.0".to_string(),
      ]
    );
  }

  #[tokio::test]
  async fn errors_for_git_dep() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_dependency(("package-b", "1.0.0"), ("SomeGitDep", "git:somerepo"));
    let err = run_resolver_and_get_error(api, vec!["package-a@1.0.0"]).await;
    match err {
      NpmResolutionError::DependencyEntry(err) => match err.source {
        NpmDependencyEntryErrorSource::RemoteDependency { specifier } => {
          assert_eq!(specifier, "git:somerepo")
        }
        _ => unreachable!(),
      },
      _ => unreachable!(),
    }
  }

  #[tokio::test]
  async fn peer_dep_on_self() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-a", "1"));

    let snapshot =
      run_resolver_and_get_snapshot(api, vec!["package-a@1.0.0"]).await;
    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "darwin".into(),
        cpu: "x86_64".into(),
      },
    );
    assert_eq!(packages, vec!["package-a@1.0.0".to_string()]);
  }

  #[tokio::test]
  async fn non_existent_optional_peer_dep() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.add_optional_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-non-existent", "*"),
    );
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    let snapshot =
      run_resolver_and_get_snapshot(api, vec!["package-a@1.0.0"]).await;
    let packages = package_names_with_info(
      &snapshot,
      &NpmSystemInfo {
        os: "darwin".into(),
        cpu: "x86_64".into(),
      },
    );
    assert_eq!(
      packages,
      vec!["package-a@1.0.0".to_string(), "package-b@1.0.0".to_string(),]
    );
  }

  #[tokio::test]
  async fn resolve_optional_peer_dep_first_then_after() {
    // This tests when a package is resolved later but doesn't have the
    // optional peer dep in its ancestor siblings.
    //
    // a -> package-peer-parent
    //
    // Then resolve b, which will have package-peer in its siblings:
    //
    //  b -> b-child -> package-peer-parent -> package-peer
    //    -> package-peer
    //  c -> c-child -> c-grand-child -> package-peer-parent -> package-peer
    //
    // Then later resolve package-d, which should resolve to package:
    //
    //  d -> package-peer-parent -> package-peer
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-b-child", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-c-child", "1.0.0");
    api.ensure_package_version("package-c-grandchild", "1.0.0");
    api.ensure_package_version("package-peer-parent", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");

    api.add_optional_peer_dependency(
      ("package-peer-parent", "1.0.0"),
      ("package-peer", "1"),
    );

    // a
    api.add_dependency(("package-a", "1.0.0"), ("package-peer-parent", "1"));

    // b
    api.add_dependency(("package-b", "1.0.0"), ("package-b-child", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(
      ("package-b-child", "1.0.0"),
      ("package-peer-parent", "1"),
    );

    // c
    api.add_dependency(("package-c", "1.0.0"), ("package-c-child", "1"));
    api.add_dependency(
      ("package-c-child", "1.0.0"),
      ("package-c-grandchild", "1"),
    );
    api.add_dependency(
      ("package-c-grandchild", "1.0.0"),
      ("package-peer-parent", "1"),
    );

    // d
    api.add_dependency(("package-d", "1.0.0"), ("package-peer-parent", "1"));

    // first run for just package-a
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["package-a@1"],
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let (packages, package_reqs) = snapshot_to_packages(snapshot.clone());
    assert_eq!(
      packages,
      Vec::from([
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer-parent".to_string(),
            "package-peer-parent@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer-parent@1.0.0".to_string(),
          copy_index: 0,
          // no optional peer
          dependencies: Default::default(),
        },
      ])
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1".to_string(), "package-a@1.0.0".to_string())]
    );

    let b_c_packages = Vec::from([
      TestNpmResolutionPackage {
        pkg_id: "package-a@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer-parent".to_string(),
          // this should update to now have the peer
          "package-peer-parent@1.0.0_package-peer@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-b@1.0.0_package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
          (
            "package-b-child".to_string(),
            "package-b-child@1.0.0_package-peer@1.0.0".to_string(),
          ),
          ("package-peer".to_string(), "package-peer@1.0.0".to_string()),
        ]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-b-child@1.0.0_package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer-parent".to_string(),
          "package-peer-parent@1.0.0_package-peer@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-c@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-c-child".to_string(),
          "package-c-child@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-c-child@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-c-grandchild".to_string(),
          "package-c-grandchild@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-c-grandchild@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer-parent".to_string(),
          "package-peer-parent@1.0.0_package-peer@1.0.0".to_string(),
        )]),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: Default::default(),
      },
      TestNpmResolutionPackage {
        pkg_id: "package-peer-parent@1.0.0_package-peer@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer".to_string(),
          "package-peer@1.0.0".to_string(),
        )]),
      },
    ]);
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["package-b@1", "package-c@1"],
        snapshot,
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let (packages, package_reqs) = snapshot_to_packages(snapshot.clone());
    assert_eq!(packages, b_c_packages);
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-peer@1.0.0".to_string(),
        ),
        ("package-c@1".to_string(), "package-c@1.0.0".to_string(),)
      ]
    );

    // now try resolving package-d and ensure it resolves to package-peer-parent w/ package-peer
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["package-d@1"],
        snapshot,
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let (packages, package_reqs) = snapshot_to_packages(snapshot.clone());
    let mut d_packages = b_c_packages;
    d_packages.insert(
      6,
      TestNpmResolutionPackage {
        pkg_id: "package-d@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([(
          "package-peer-parent".to_string(),
          "package-peer-parent@1.0.0_package-peer@1.0.0".to_string(),
        )]),
      },
    );
    assert_eq!(packages, d_packages);
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-peer@1.0.0".to_string(),
        ),
        ("package-c@1".to_string(), "package-c@1.0.0".to_string()),
        ("package-d@1".to_string(), "package-d@1.0.0".to_string()),
      ]
    );
  }

  #[tokio::test]
  async fn dudpes_dep_overlapping_high_version_constraint_then_low() {
    // a -> b (1.x)
    //   -> c -> b (1.0.0)
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-b", "1.0.1");
    api.ensure_package_version("package-c", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-b", "1.0.0"));

    let (packages, _package_reqs) =
      run_resolver_and_get_output(api, vec!["package-a@1.0.0"]).await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@1.0.0".to_string(),),
            ("package-c".to_string(), "package-c@1.0.0".to_string(),)
          ])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::new(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
      ]
    );
  }

  #[tokio::test]
  async fn dudpes_dep_overlapping_high_version_constraint_then_low_with_peer_deps()
   {
    // a -> b (1.x) -> d
    //   -> c -> b (1.0.0)
    // d
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-b", "1.0.1");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-b", "1.0.0"));
    // this is only a peer dep of the 1.0.1 package and not 1.0.0 so it initially
    // resolves the peer dep, but then it resets itself so there's no longer any
    api.add_peer_dependency(("package-b", "1.0.1"), ("package-d", "1"));

    let (packages, _package_reqs) = run_resolver_and_get_output(
      api,
      vec!["package-a@1.0.0", "package-d@1.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            ("package-b".to_string(), "package-b@1.0.0".to_string(),),
            ("package-c".to_string(), "package-c@1.0.0".to_string(),)
          ])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::new(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::new(),
        },
      ]
    );
  }

  #[tokio::test]
  async fn graph_from_snapshot_dep_on_self() {
    // there are some lockfiles in the wild that when loading have a dependency
    // on themselves and causes a panic, so ensure this doesn't panic
    let snapshot = SerializedNpmResolutionSnapshot {
      root_packages: HashMap::from([(
        PackageReq::from_str("package-0").unwrap(),
        NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
      )]),
      packages: Vec::from([
        crate::resolution::SerializedNpmResolutionSnapshotPackage {
          id: NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          system: Default::default(),
          dependencies: HashMap::from([(
            "package-a".into(),
            NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          )]),

          optional_peer_dependencies: Default::default(),
          optional_dependencies: HashSet::new(),
          extra: None,
          is_deprecated: false,
          dist: Some(crate::registry::NpmPackageVersionDistInfo {
            tarball: "https://example.com/package-0@1.0.0.tgz".to_string(),
            shasum: None,
            integrity: None,
          }),
          has_bin: false,
          has_scripts: false,
        },
      ]),
    };
    let snapshot = NpmResolutionSnapshot::new(snapshot.into_valid().unwrap());
    // assert this doesn't panic
    let _graph = Graph::from_snapshot(snapshot);
  }

  #[tokio::test]
  async fn link_packages() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b1", "1.0.0");
    api.ensure_package_version("package-b2", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b1", "1"));
    api.add_dependency(("package-b1", "1.0.0"), ("package-b2", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));

    let link_packages = HashMap::from([(
      PackageName::from_static("package-b1"),
      vec![
        NpmPackageVersionInfo {
          // should not select this one because 1.0.1 is higher
          version: Version::parse_standard("1.0.0").unwrap(),
          ..Default::default()
        },
        NpmPackageVersionInfo {
          version: Version::parse_standard("1.0.1").unwrap(),
          dependencies: HashMap::from([(
            StackString::from_static("package-c"),
            StackString::from_static("1"),
          )]),
          ..Default::default()
        },
      ],
    )]);

    let (packages, package_reqs) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        link_packages: Some(&link_packages),
        ..Default::default()
      },
    )
    .await;

    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b1".to_string(),
            "package-b1@1.0.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b1@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-c".to_string(),
            "package-c@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-d".to_string(),
            "package-d@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn link_package_tag() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.add_dist_tag("package-a", "next", "1.0.0");

    let link_packages = HashMap::from([(
      PackageName::from_static("package-a"),
      vec![NpmPackageVersionInfo {
        version: Version::parse_standard("1.0.0").unwrap(),
        dependencies: HashMap::from([(
          StackString::from_static("package-b"),
          StackString::from_static("1"),
        )]),
        ..Default::default()
      }],
    )]);

    let (packages, package_reqs) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@next"],
        link_packages: Some(&link_packages),
        ..Default::default()
      },
    )
    .await;

    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![("package-a@next".to_string(), "package-a@1.0.0".to_string())]
    );
  }

  #[tokio::test]
  async fn resolve_link_copy_index() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.1.0");
    api.ensure_package_version("package-peer", "1.2.0");
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-peer", "=1.1.0"));
    api.add_dependency(("package-c", "1.0.0"), ("package-a", "1"));

    let link_packages = HashMap::from([(
      PackageName::from_static("package-a"),
      vec![NpmPackageVersionInfo {
        version: Version::parse_standard("1.0.0").unwrap(),
        peer_dependencies: HashMap::from([(
          StackString::from_static("package-peer"),
          StackString::from_static("*"), // should select 1.2.0, then 1.1.0
        )]),
        ..Default::default()
      }],
    )]);

    let input_reqs = vec!["package-a@1.0", "package-b@1.0"];
    // skip deduping
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          link_packages: Some(&link_packages),
          skip_dedup: true,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 1,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.2.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.2.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-c".to_string(),
                "package-c@1.0.0_package-peer@1.1.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@1.1.0".to_string(),)
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-c@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.2.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
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
    // dedup should consolidate to a single package
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api,
        RunResolverOptions {
          reqs: input_reqs.clone(),
          link_packages: Some(&link_packages),
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([
              (
                "package-c".to_string(),
                "package-c@1.0.0_package-peer@1.1.0".to_string(),
              ),
              ("package-peer".to_string(), "package-peer@1.1.0".to_string(),)
            ]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-c@1.0.0_package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-a".to_string(),
              "package-a@1.0.0_package-peer@1.1.0".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.1.0".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([]),
          },
        ]
      );
      assert_eq!(
        package_reqs,
        vec![
          (
            "package-a@1.0".to_string(),
            "package-a@1.0.0_package-peer@1.1.0".to_string()
          ),
          (
            "package-b@1.0".to_string(),
            "package-b@1.0.0_package-peer@1.1.0".to_string()
          )
        ]
      );
    }
  }

  #[tokio::test]
  async fn aws_sdk_issue() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("@aws-sdk/client-s3", "3.679.0");
    api.ensure_package_version("@aws-sdk/client-sts", "3.679.0");
    api.ensure_package_version("@aws-sdk/client-sso-oidc", "3.679.0");
    api.ensure_package_version("@aws-sdk/credential-provider-node", "3.679.0");
    api.ensure_package_version("@aws-sdk/credential-provider-ini", "3.679.0");
    api.ensure_package_version("@aws-sdk/credential-provider-sso", "3.679.0");
    api.ensure_package_version(
      "@aws-sdk/credential-provider-web-identity",
      "3.679.0",
    );
    api.ensure_package_version("@aws-sdk/token-providers", "3.679.0");

    api.add_dependency(
      ("@aws-sdk/client-s3", "3.679.0"),
      ("@aws-sdk/client-sts", "3.679.0"),
    );
    api.add_dependency(
      ("@aws-sdk/client-s3", "3.679.0"),
      ("@aws-sdk/client-sso-oidc", "3.679.0"),
    );

    api.add_dependency(
      ("@aws-sdk/client-sts", "3.679.0"),
      ("@aws-sdk/client-sso-oidc", "3.679.0"),
    );
    api.add_dependency(
      ("@aws-sdk/client-sts", "3.679.0"),
      ("@aws-sdk/credential-provider-node", "3.679.0"),
    );

    api.add_peer_dependency(
      ("@aws-sdk/client-sso-oidc", "3.679.0"),
      ("@aws-sdk/client-sts", "^3.679.0"),
    );

    api.add_peer_dependency(
      ("@aws-sdk/credential-provider-ini", "3.679.0"),
      ("@aws-sdk/client-sts", "^3.679.0"),
    );
    api.add_dependency(
      ("@aws-sdk/credential-provider-ini", "3.679.0"),
      ("@aws-sdk/credential-provider-sso", "3.679.0"),
    );

    api.add_dependency(
      ("@aws-sdk/credential-provider-node", "3.679.0"),
      ("@aws-sdk/credential-provider-ini", "3.679.0"),
    );
    api.add_peer_dependency(
      ("@aws-sdk/credential-provider-sso", "3.679.0"),
      ("@aws-sdk/client-sso-oidc", "^3.679.0"),
    );

    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["@aws-sdk/client-s3@3.679.0"],
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let (packages, package_reqs) = snapshot_to_packages(snapshot.clone());
    // not sure if this is exactly correct, but there are no duplicate packages
    let expected_packages = Vec::from([
      TestNpmResolutionPackage {
        pkg_id: "@aws-sdk/client-s3@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
            ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0".to_string()),
            ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string())
        ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0".to_string()),
              ("@aws-sdk/credential-provider-node".to_string(), "@aws-sdk/credential-provider-node@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-ini@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string()),
              ("@aws-sdk/credential-provider-sso".to_string(), "@aws-sdk/credential-provider-sso@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-node@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/credential-provider-ini".to_string(), "@aws-sdk/credential-provider-ini@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-sso@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0___@aws-sdk+client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0".to_string()),
          ]),
      }]
    );
    assert_eq!(packages, expected_packages);
    assert_eq!(package_reqs, vec![(
      "@aws-sdk/client-s3@3.679.0".to_string(),
      "@aws-sdk/client-s3@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string(),
    )]);

    // now run again with a broad specifier
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["@aws-sdk/client-s3@*"],
        snapshot,
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let (packages, package_reqs) = snapshot_to_packages(snapshot);
    assert_eq!(packages, expected_packages);
    assert_eq!(package_reqs, vec![(
      "@aws-sdk/client-s3".to_string(),
      "@aws-sdk/client-s3@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string(),
    ), (
      "@aws-sdk/client-s3@3.679.0".to_string(),
      "@aws-sdk/client-s3@3.679.0_@aws-sdk+client-sts@3.679.0__@aws-sdk+client-sso-oidc@3.679.0___@aws-sdk+client-sts@3.679.0".to_string(),
    )]);
  }

  // This was an attempt at reducing duplicate dependencies. Essentially, if we previously
  // resolved a package (in this case package-b) with a certain peer dep (package-peer@1.0.2)
  // then when re-resolving it in a different position, we check for the existence of
  // package-peer@1.0.2 in all the ancestor peers and use that rather than using package-peer@1.0.1
  // which is the first found resolved ancestor peer dep. The reason we don't use it is because
  // it would create a duplicate copy of package-b.
  #[tokio::test]
  async fn prefer_previously_resolved_peer_in_ancestors() {
    let api = TestNpmRegistryApi::default();
    // package-peer@1 (1.0.2)
    // a -> b -> package-peer@1 (peer)
    //   -> c -> d -> b -> package-peer@1 (peer)
    //        -> package-peer@1.0.1 (dep) <-- this should be ignored for resolving b's peer dep because b previously resolved to 1.0.2
    //   -> package-peer@1 (peer)
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.1");
    api.ensure_package_version("package-peer", "1.0.2");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");

    api.add_dependency(("package-a", "1.0.0"), ("package-b", "*"));
    api.add_dependency(("package-a", "1.0.0"), ("package-c", "*"));
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "*"));
    api.add_dependency(("package-c", "1.0.0"), ("package-peer", "1.0.1"));
    api.add_peer_dependency(("package-d", "1.0.0"), ("package-b", "1"));

    // skipping dedup
    let input_reqs = vec!["package-a@1.0.0", "package-peer@1"];
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup: true,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0_package-peer@1.0.2_package-b@1.0.0__package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-b".to_string(),
              "package-b@1.0.0_package-peer@1.0.2".to_string(),
            ), (
              "package-c".to_string(),
              "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.2_package-peer@1.0.2".to_string(),
            ), (
              "package-peer".to_string(),
              "package-peer@1.0.2".to_string()
            )])
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.0.2".to_string(),
            )])
          },
          TestNpmResolutionPackage {
            pkg_id: "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.2_package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-d".to_string(),
              "package-d@1.0.0_package-b@1.0.0__package-peer@1.0.2_package-peer@1.0.2".to_string(),
            ), (
              "package-peer".to_string(),
              "package-peer@1.0.1".to_string(),
            )]),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-d@1.0.0_package-b@1.0.0__package-peer@1.0.2_package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-b".to_string(),
              "package-b@1.0.0_package-peer@1.0.2".to_string(),
            )])
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.0.1".to_string(),
            copy_index: 0,
            dependencies: Default::default(),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: Default::default(),
          },
        ]
      );
      assert_eq!(
        package_reqs,
        vec![
          ("package-a@1.0.0".to_string(), "package-a@1.0.0_package-peer@1.0.2_package-b@1.0.0__package-peer@1.0.2".to_string()),
          ("package-peer@1".to_string(), "package-peer@1.0.2".to_string()),
        ]
      );
    }

    // dedup pass should consolidate to 1.0.1
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api,
        RunResolverOptions {
          reqs: input_reqs,
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer@1.0.1_package-b@1.0.0__package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.1".to_string(),
          ), (
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1_package-peer@1.0.1".to_string(),
          ), (
            "package-peer".to_string(),
            "package-peer@1.0.1".to_string()
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-peer".to_string(),
            "package-peer@1.0.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1_package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-d".to_string(),
            "package-d@1.0.0_package-b@1.0.0__package-peer@1.0.1_package-peer@1.0.1".to_string(),
          ), (
            "package-peer".to_string(),
            "package-peer@1.0.1".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0_package-b@1.0.0__package-peer@1.0.1_package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.1".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
      ]
    );
      assert_eq!(
      package_reqs,
      vec![
        ("package-a@1.0.0".to_string(), "package-a@1.0.0_package-peer@1.0.1_package-b@1.0.0__package-peer@1.0.1".to_string()),
        ("package-peer@1".to_string(), "package-peer@1.0.1".to_string()),
      ]
    );
    }
  }

  #[tokio::test]
  async fn test_newest_dependency_date() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("a", "1.0.0");
    api.ensure_package_version("a", "1.0.1");
    api.ensure_package_version("a", "1.0.2");
    api.ensure_package_version("b", "1.0.0");
    api.ensure_package_version("b", "1.0.1");

    api.with_package("a", |info| {
      info.dist_tags.insert("tag".to_string(), version("1.0.2"));
      info.time.insert(
        version("1.0.0"),
        "2015-11-07T00:00:00.000Z".parse().unwrap(),
      );
      info.time.insert(
        version("1.0.1"),
        "2020-11-07T00:00:00.000Z".parse().unwrap(),
      );
      info.time.insert(
        version("1.0.2"),
        "2022-11-07T00:00:00.000Z".parse().unwrap(),
      );
    });

    api.with_package("b", |info| {
      info.dist_tags.insert("tag".to_string(), version("1.0.1"));
      info.time.insert(
        version("1.0.0"),
        "2015-11-07T00:00:00.000Z".parse().unwrap(),
      );
      info.time.insert(
        version("1.0.1"),
        "2022-11-07T00:00:00.000Z".parse().unwrap(),
      );
    });

    {
      let (packages, _package_reqs) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: vec!["a@1", "b@1"],
          newest_dependency_date: NewestDependencyDateOptions {
            date: Some(NewestDependencyDate(
              "2021-11-07T00:00:00.000Z".parse().unwrap(),
            )),
            exclude: BTreeSet::from(["b".into()]),
          },
          ..Default::default()
        },
      )
      .await;
      assert_eq!(packages.len(), 2);
      assert_eq!(packages[0].pkg_id, "a@1.0.1");
      assert_eq!(packages[1].pkg_id, "b@1.0.1");
    }

    {
      let err = run_resolver_with_options_and_get_err(
        &api,
        RunResolverOptions {
          reqs: vec!["a@1"],
          newest_dependency_date: NewestDependencyDateOptions::from_date(
            "2010-11-07T00:00:00.000Z".parse().unwrap(),
          ),
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        err.to_string(),
        "Could not find npm package 'a' matching '1'.\n\nA newer matching version was found, but it was not used because it was newer than the specified minimum dependency date of 2010-11-07 00:00:00 UTC."
      );
    }
    {
      let err = run_resolver_with_options_and_get_err(
        &api,
        RunResolverOptions {
          reqs: vec!["a@tag"],
          newest_dependency_date: NewestDependencyDateOptions::from_date(
            "2010-11-07T00:00:00.000Z".parse().unwrap(),
          ),
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        err.to_string(),
        "Failed resolving tag 'a@tag' mapped to 'a@1.0.2' because the package version was published at 2022-11-07 00:00:00 UTC, but dependencies newer than 2010-11-07 00:00:00 UTC are not allowed because it is newer than the specified minimum dependency date."
      );
    }
  }

  #[tokio::test]
  async fn vite_tailwind_optional_peer_duplicates() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("@deno/vite-plugin", "1.0.4");
    api.ensure_package_version("@tailwindcss/vite", "4.0.17");
    api.ensure_package_version("lightningcss", "1.29.2");
    api.ensure_package_version("vite", "6.2.4");

    api.add_peer_dependency(
      ("@deno/vite-plugin", "1.0.4"),
      ("vite", "5.x || 6.x"),
    );

    api.add_dependency(
      ("@tailwindcss/vite", "4.0.17"),
      ("lightningcss", "1.29.2"),
    );
    api.add_peer_dependency(
      ("@tailwindcss/vite", "4.0.17"),
      ("vite", "^5.2.0 || ^6"),
    );

    api.add_optional_peer_dependency(
      ("vite", "6.2.4"),
      ("lightningcss", "^1.21.0"),
    );

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["@deno/vite-plugin@~1.0.4", "@tailwindcss/vite@~4.0.17"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "@deno/vite-plugin@1.0.4_vite@6.2.4__lightningcss@1.29.2"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "vite".to_string(),
            "vite@6.2.4_lightningcss@1.29.2".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "@tailwindcss/vite@4.0.17_vite@6.2.4__lightningcss@1.29.2_lightningcss@1.29.2"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "lightningcss".to_string(),
            "lightningcss@1.29.2".to_string(),
          ), (
            "vite".to_string(),
            "vite@6.2.4_lightningcss@1.29.2".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "lightningcss@1.29.2".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "vite@6.2.4_lightningcss@1.29.2".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "lightningcss".to_string(),
            "lightningcss@1.29.2".to_string(),
          )])
        },
      ]
    );
    assert_eq!(package_reqs, vec![
      ("@deno/vite-plugin@~1.0.4".to_string(), "@deno/vite-plugin@1.0.4_vite@6.2.4__lightningcss@1.29.2".to_string()),
      ("@tailwindcss/vite@~4.0.17".to_string(), "@tailwindcss/vite@4.0.17_vite@6.2.4__lightningcss@1.29.2_lightningcss@1.29.2".to_string()),
    ]);
  }

  #[tokio::test]
  async fn snapshot_version_missing_registry_force_reload() {
    struct ReloadRegistry(RefCell<Vec<TestNpmRegistryApi>>);

    #[async_trait::async_trait(?Send)]
    impl NpmRegistryApi for ReloadRegistry {
      async fn package_info(
        &self,
        name: &str,
      ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
        let reg = self.0.borrow()[0].clone();
        reg.package_info(name).await
      }

      fn mark_force_reload(&self) -> bool {
        let mut regs = self.0.borrow_mut();
        if regs.len() == 2 {
          regs.remove(0);
          true
        } else {
          false
        }
      }
    }

    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "0.5.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));

    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: Vec::from(["package-a@1"]),
        ..Default::default()
      },
    )
    .await
    .unwrap();
    let missing_api = TestNpmRegistryApi::default();
    missing_api.ensure_package_version("package-a", "1.0.0");
    missing_api.ensure_package_version("package-b", "0.5.0");
    missing_api.ensure_package_version("package-c", "1.0.0");

    let err = run_resolver_with_options_and_get_snapshot(
      &missing_api,
      RunResolverOptions {
        snapshot: snapshot.clone(),
        reqs: Vec::from(["package-c@1"]),
        ..Default::default()
      },
    )
    .await
    .unwrap_err();
    match err {
      NpmResolutionError::Resolution(
        NpmPackageVersionResolutionError::VersionNotFound(
          NpmPackageVersionNotFound(nv),
        ),
      ) => {
        assert_eq!(nv, PackageNv::from_str("package-b@1.0.0").unwrap());
      }
      _ => unreachable!(),
    }

    let reload_registry =
      ReloadRegistry(RefCell::new(Vec::from([missing_api, api])));
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &reload_registry,
      RunResolverOptions {
        snapshot,
        reqs: Vec::from(["package-c@1"]),
        ..Default::default()
      },
    )
    .await
    .unwrap();
    assert_eq!(reload_registry.0.borrow().len(), 1); // ensure the reload happened
    let (packages, package_reqs) = snapshot_to_packages(snapshot);
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-c@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
        ("package-c@1".to_string(), "package-c@1.0.0".to_string()),
      ]
    );
  }

  #[tokio::test]
  async fn dedup_lower_specific_with_overlapping_then_higher_root_req_added_later()
   {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-a", "1.1.0");

    let snapshot = {
      let snapshot = run_resolver_with_options_and_get_snapshot(
        &api,
        RunResolverOptions {
          reqs: vec!["package-a@^1.0.0", "package-a@1.0.0"],
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await
      .unwrap();
      let (packages, package_reqs) = snapshot_to_packages(snapshot.clone());
      assert_eq!(
        packages,
        vec![TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },]
      );
      assert_eq!(
        package_reqs,
        vec![
          ("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string()),
          (
            "package-a@^1.0.0".to_string(),
            "package-a@1.0.0".to_string()
          ),
        ]
      );
      snapshot
    };
    {
      let (packages, package_reqs) = run_resolver_with_options_and_get_output(
        api,
        RunResolverOptions {
          snapshot,
          reqs: vec!["package-a@1.1.0"],
          skip_dedup: false,
          ..Default::default()
        },
      )
      .await;
      assert_eq!(
        packages,
        vec![
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.0.0".to_string(),
            copy_index: 0,
            dependencies: Default::default(),
          },
          TestNpmResolutionPackage {
            pkg_id: "package-a@1.1.0".to_string(),
            copy_index: 0,
            dependencies: Default::default(),
          }
        ]
      );
      assert_eq!(
        package_reqs,
        vec![
          ("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string()),
          ("package-a@1.1.0".to_string(), "package-a@1.1.0".to_string()),
          (
            "package-a@^1.0.0".to_string(),
            "package-a@1.1.0".to_string()
          ),
        ]
      );
    }
  }

  fn version(text: &str) -> Version {
    Version::parse_from_npm(text).unwrap()
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct TestNpmResolutionPackage {
    pub pkg_id: String,
    pub copy_index: u8,
    pub dependencies: BTreeMap<String, String>,
  }

  async fn run_resolver_and_get_output(
    api: TestNpmRegistryApi,
    reqs: Vec<&str>,
  ) -> (Vec<TestNpmResolutionPackage>, Vec<(String, String)>) {
    run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs,
        ..Default::default()
      },
    )
    .await
  }

  async fn run_resolver_with_options_and_get_output(
    api: TestNpmRegistryApi,
    options: RunResolverOptions<'_>,
  ) -> (Vec<TestNpmResolutionPackage>, Vec<(String, String)>) {
    let snapshot = run_resolver_with_options_and_get_snapshot(&api, options)
      .await
      .unwrap();
    snapshot_to_packages(snapshot)
  }

  fn snapshot_to_packages(
    snapshot: NpmResolutionSnapshot,
  ) -> (Vec<TestNpmResolutionPackage>, Vec<(String, String)>) {
    let mut packages = snapshot
      .all_packages_for_every_system()
      .cloned()
      .collect::<Vec<_>>();
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut package_reqs = snapshot
      .package_reqs
      .into_iter()
      .map(|(a, b)| {
        (
          a.to_string(),
          snapshot
            .root_packages
            .get(&b)
            .unwrap()
            .as_serialized()
            .to_string(),
        )
      })
      .collect::<Vec<_>>();
    package_reqs.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));

    let packages = packages
      .into_iter()
      .map(|pkg| TestNpmResolutionPackage {
        pkg_id: pkg.id.as_serialized().to_string(),
        copy_index: pkg.copy_index,
        dependencies: pkg
          .dependencies
          .into_iter()
          .map(|(key, value)| {
            (key.to_string(), value.as_serialized().to_string())
          })
          .collect(),
      })
      .collect();

    (packages, package_reqs)
  }

  fn package_names_with_info(
    snapshot: &NpmResolutionSnapshot,
    system_info: &NpmSystemInfo,
  ) -> Vec<String> {
    let mut packages = snapshot
      .all_system_packages(system_info)
      .into_iter()
      .map(|p| p.id.as_serialized().to_string())
      .collect::<Vec<_>>();
    packages.sort();
    let mut serialized_pkgs = snapshot
      .as_valid_serialized_for_system(system_info)
      .into_serialized()
      .packages
      .into_iter()
      .map(|p| p.id.as_serialized().to_string())
      .collect::<Vec<_>>();
    serialized_pkgs.sort();
    // ensure the output of both of these are the same
    assert_eq!(serialized_pkgs, packages);
    packages
  }

  async fn run_resolver_and_get_snapshot(
    api: TestNpmRegistryApi,
    reqs: Vec<&str>,
  ) -> NpmResolutionSnapshot {
    run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs,
        link_packages: None,
        newest_dependency_date: Default::default(),
        snapshot: Default::default(),
        expected_diagnostics: Default::default(),
        skip_dedup: false,
      },
    )
    .await
    .unwrap()
  }

  #[derive(Default)]
  struct RunResolverOptions<'a> {
    snapshot: NpmResolutionSnapshot,
    reqs: Vec<&'a str>,
    link_packages: Option<&'a HashMap<PackageName, Vec<NpmPackageVersionInfo>>>,
    expected_diagnostics: Vec<&'a str>,
    newest_dependency_date: NewestDependencyDateOptions,
    skip_dedup: bool,
  }

  async fn run_resolver_with_options_and_get_err(
    api: &impl NpmRegistryApi,
    options: RunResolverOptions<'_>,
  ) -> NpmResolutionError {
    run_resolver_with_options_and_get_snapshot(api, options)
      .await
      .unwrap_err()
  }

  async fn run_resolver_with_options_and_get_snapshot(
    api: &impl NpmRegistryApi,
    options: RunResolverOptions<'_>,
  ) -> Result<NpmResolutionSnapshot, NpmResolutionError> {
    fn snapshot_to_serialized(
      snapshot: &NpmResolutionSnapshot,
    ) -> SerializedNpmResolutionSnapshot {
      let mut snapshot = snapshot.as_valid_serialized().into_serialized();
      snapshot.packages.sort_by(|a, b| a.id.cmp(&b.id));
      snapshot
    }

    let snapshot = options.snapshot;
    let mut graph = Graph::from_snapshot(snapshot);
    let link_packages = Arc::new(
      options
        .link_packages
        .cloned()
        .unwrap_or_else(HashMap::default),
    );
    let npm_version_resolver = NpmVersionResolver {
      link_packages: link_packages.clone(),
      newest_dependency_date_options: options.newest_dependency_date,
    };
    let mut resolver = GraphDependencyResolver::new(
      &mut graph,
      api,
      &npm_version_resolver,
      None,
      GraphDependencyResolverOptions {
        should_dedup: !options.skip_dedup,
      },
    );

    for req in options.reqs {
      let req = PackageReq::from_str(req).unwrap();
      resolver
        .add_package_req(&req, &api.package_info(&req.name).await.unwrap())?;
    }

    resolver.resolve_pending().await?;
    {
      let diagnostics = resolver.take_unmet_peer_diagnostics();
      let diagnostics = diagnostics
        .iter()
        .map(|d| {
          format!(
            "{}: {} -> {}",
            d.ancestors
              .iter()
              .rev()
              .map(|v| v.to_string())
              .collect::<Vec<_>>()
              .join(" -> "),
            d.dependency,
            d.resolved
          )
        })
        .collect::<Vec<_>>();
      assert_eq!(diagnostics, options.expected_diagnostics);
    }
    let snapshot = graph.into_snapshot(api, &link_packages).await?;

    {
      let graph = Graph::from_snapshot(snapshot.clone());
      let new_snapshot = graph.into_snapshot(api, &link_packages).await?;
      assert_eq!(
        snapshot_to_serialized(&snapshot),
        snapshot_to_serialized(&new_snapshot),
        "recreated snapshot should be the same"
      );
      // create one again from the new snapshot
      let graph = Graph::from_snapshot(new_snapshot.clone());
      let new_snapshot2 = graph.into_snapshot(api, &link_packages).await?;
      assert_eq!(
        snapshot_to_serialized(&snapshot),
        snapshot_to_serialized(&new_snapshot2),
        "second recreated snapshot should be the same"
      );
    }

    Ok(snapshot)
  }

  #[tokio::test]
  async fn dedup_with_initially_partially_resolved_graph() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-shared", "1.0.0");
    api.add_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-shared", "^1.0.0"),
    );

    // first, resolve package-a which pulls in package-shared@1.0.0
    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: Vec::from(["package-a@1"]),
        ..Default::default()
      },
    )
    .await
    .unwrap();

    // now "publish" package-b and package-shared 1.1.0
    api.ensure_package_version("package-b", "1.0.0");
    api.add_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-shared", "^1.1.0"),
    );
    api.ensure_package_version("package-shared", "1.1.0");

    // now resolve package-b
    let (packages, package_reqs) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        snapshot,
        reqs: Vec::from(["package-b@1"]),
        ..Default::default()
      },
    )
    .await;

    // after dedup, package-b should use package-shared@1.1.0 (consolidated)
    assert_eq!(
      packages,
      vec![
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-shared@1.1.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-shared".to_string(),
            "package-shared@1.1.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-b@1.0.0_package-shared@1.1.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-shared".to_string(),
            "package-shared@1.1.0".to_string(),
          )])
        },
        TestNpmResolutionPackage {
          pkg_id: "package-shared@1.1.0".to_string(),
          copy_index: 0,
          dependencies: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![
        (
          "package-a@1".to_string(),
          "package-a@1.0.0_package-shared@1.1.0".to_string()
        ),
        (
          "package-b@1".to_string(),
          "package-b@1.0.0_package-shared@1.1.0".to_string()
        ),
      ]
    );
  }

  async fn run_resolver_and_get_error(
    api: TestNpmRegistryApi,
    reqs: Vec<&str>,
  ) -> NpmResolutionError {
    let snapshot = NpmResolutionSnapshot::new(Default::default());
    let mut graph = Graph::from_snapshot(snapshot);
    let npm_version_resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: Default::default(),
    };
    let mut resolver = GraphDependencyResolver::new(
      &mut graph,
      &api,
      &npm_version_resolver,
      None,
      GraphDependencyResolverOptions { should_dedup: true },
    );

    for req in reqs {
      let req = PackageReq::from_str(req).unwrap();
      resolver
        .add_package_req(&req, &api.package_info(&req.name).await.unwrap())
        .unwrap();
    }

    resolver.resolve_pending().await.unwrap_err()
  }
}
