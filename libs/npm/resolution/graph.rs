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
use super::overrides::NpmOverrides;
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

/// Result of resolving peer deps for a subtree (Phase 2).
struct PeersResolution {
  /// Peers that were resolved from outside this subtree (bubble up to parent).
  resolved_peers: BTreeMap<StackString, NodeId>,
  /// Peers that couldn't be found anywhere.
  missing_peers: BTreeMap<StackString, VersionReq>,
  /// Optional peers that were not resolved in this context.
  /// Tracked for cache invalidation: if a different context has one of
  /// these available, the cache entry should NOT match.
  unresolved_optional_peers: Vec<StackString>,
  /// The final NodeId for this node (may be a copy with peer deps in identity).
  node_id: NodeId,
}

/// A cached peer resolution result for a specific (nv, parent_context).
struct PeersCacheEntry {
  /// Which peers were resolved from the parent scope.
  resolved_peers: BTreeMap<StackString, NodeId>,
  /// Which peers were missing.
  missing_peers: BTreeMap<StackString, VersionReq>,
  /// Optional peers that were not resolved in this context.
  unresolved_optional_peers: Vec<StackString>,
  /// The resulting NodeId (with peer deps in identity).
  node_id: NodeId,
}

/// A pending resolved identifier used in the graph. At the end of resolution, these
/// will become fully resolved to an `NpmPackageId`.
#[derive(Clone)]
struct ResolvedId {
  nv: Rc<PackageNv>,
  /// NodeIds of peer dependency nodes resolved during snapshotting or Phase 2.
  peer_dependencies: Vec<NodeId>,
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
      dep.hash(&mut hasher);
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

  pub fn remove(&mut self, node_id: NodeId) {
    if let Some((_, resolved_id_hash)) =
      self.node_to_resolved_id.remove(&node_id)
    {
      self.resolved_to_node_id.remove(&resolved_id_hash);
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum GraphPathResolutionMode {
  All,
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
  /// The currently active override rules at this point in the tree traversal.
  active_overrides: Rc<NpmOverrides>,
}

impl GraphPath {
  pub fn for_root(
    node_id: NodeId,
    nv: Rc<PackageNv>,
    mode: GraphPathResolutionMode,
    active_overrides: Rc<NpmOverrides>,
  ) -> Rc<Self> {
    // scope the overrides for this root package so that any scoped
    // overrides targeting this package have their children activated
    let scoped = active_overrides.for_child(&nv.name, &nv.version);
    Rc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Root(nv.clone())),
      node_id_ref: NodeIdRef::new(node_id),
      // use an empty specifier
      specifier: "".into(),
      nv,
      linked_circular_descendants: Default::default(),
      mode,
      active_overrides: scoped,
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
    let active_overrides =
      self.active_overrides.for_child(&nv.name, &nv.version);
    Rc::new(Self {
      previous_node: Some(GraphPathNodeOrRoot::Node(self.clone())),
      node_id_ref: NodeIdRef::new(node_id),
      specifier,
      nv,
      linked_circular_descendants: Default::default(),
      mode,
      active_overrides,
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

/// Tarjan's strongly connected components algorithm.
///
/// Returns SCCs in reverse topological order (leaf/sink SCCs first),
/// which is the order we need for bottom-up ID computation.
fn tarjan_scc(adj: &HashMap<NodeId, Vec<NodeId>>) -> Vec<Vec<NodeId>> {
  struct TarjanState {
    index_counter: usize,
    stack: Vec<NodeId>,
    on_stack: HashSet<NodeId>,
    indices: HashMap<NodeId, usize>,
    lowlinks: HashMap<NodeId, usize>,
    result: Vec<Vec<NodeId>>,
  }

  fn strongconnect(
    node: NodeId,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    state: &mut TarjanState,
  ) {
    state.indices.insert(node, state.index_counter);
    state.lowlinks.insert(node, state.index_counter);
    state.index_counter += 1;
    state.stack.push(node);
    state.on_stack.insert(node);

    if let Some(neighbors) = adj.get(&node) {
      for &neighbor in neighbors {
        if !state.indices.contains_key(&neighbor) {
          strongconnect(neighbor, adj, state);
          let neighbor_low = *state.lowlinks.get(&neighbor).unwrap();
          let node_low = state.lowlinks.get_mut(&node).unwrap();
          if neighbor_low < *node_low {
            *node_low = neighbor_low;
          }
        } else if state.on_stack.contains(&neighbor) {
          let neighbor_idx = *state.indices.get(&neighbor).unwrap();
          let node_low = state.lowlinks.get_mut(&node).unwrap();
          if neighbor_idx < *node_low {
            *node_low = neighbor_idx;
          }
        }
      }
    }

    if state.lowlinks.get(&node) == state.indices.get(&node) {
      let mut scc = Vec::new();
      loop {
        let w = state.stack.pop().unwrap();
        state.on_stack.remove(&w);
        scc.push(w);
        if w == node {
          break;
        }
      }
      state.result.push(scc);
    }
  }

  let mut state = TarjanState {
    index_counter: 0,
    stack: Vec::new(),
    on_stack: HashSet::new(),
    indices: HashMap::new(),
    lowlinks: HashMap::new(),
    result: Vec::new(),
  };

  for &node in adj.keys() {
    if !state.indices.contains_key(&node) {
      strongconnect(node, adj, &mut state);
    }
  }

  state.result
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
          get_or_create_graph_node(
            graph,
            peer_dep,
            packages,
            created_package_ids,
            &ancestor_ids_with_current,
          )
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

  /// Computes NpmPackageIds for ALL nodes using SCC-based cycle detection.
  ///
  /// Uses SCC-based cycle detection:
  /// 1. Build a peer dependency graph from all ResolvedId.peer_dependencies
  /// 2. Detect strongly connected components (cycles) using Tarjan's algorithm
  /// 3. Compute NpmPackageIds bottom-up: non-cyclic leaf nodes first, then
  ///    parents using cached child IDs
  /// 4. For peers within the same SCC (cycle), use flat name@version
  ///    (no recursive nesting) to avoid infinite recursion
  ///
  /// Each node's ID is computed exactly ONCE, avoiding the O(n²) expansion
  /// that occurred with the previous per-path computing HashSet approach.
  fn compute_all_npm_pkg_ids(&self) -> HashMap<NodeId, NpmPackageId> {
    // Step 1: Build per-node peer info and NV-level adjacency graph.
    //
    // We need TWO levels of analysis:
    // - NV-level (PackageNv): for cycle detection. Multiple NodeIds can
    //   map to the same NV. A cycle at the NV level (A peers with B,
    //   B peers with A) means those NVs should use flat IDs for each other,
    //   even if the specific NodeIds involved are different.
    // - Node-level (NodeId): for computation ordering. We process nodes
    //   in topological order so non-cyclic peers are cached before use.
    let mut node_peers: HashMap<NodeId, Vec<(NodeId, Rc<PackageNv>)>> =
      HashMap::new();
    let mut nv_peer_adj: HashMap<Rc<PackageNv>, HashSet<Rc<PackageNv>>> =
      HashMap::new();
    let mut node_adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for (&node_id, (resolved_id, _)) in
      &self.resolved_node_ids.node_to_resolved_id
    {
      let nv_entry = nv_peer_adj.entry(resolved_id.nv.clone()).or_default();
      let mut seen_nvs = HashSet::new();
      let mut peers = Vec::new();
      let mut adj = Vec::new();

      for peer_dep in &resolved_id.peer_dependencies {
        if let Some((child_id, child_resolved_id)) =
          self.peer_dep_to_maybe_node_id_and_resolved_id(peer_dep)
        {
          if seen_nvs.insert(child_resolved_id.nv.clone()) {
            nv_entry.insert(child_resolved_id.nv.clone());
            adj.push(child_id);
            peers.push((child_id, child_resolved_id.nv.clone()));
          }
        }
      }

      node_adj.insert(node_id, adj);
      node_peers.insert(node_id, peers);
    }

    // Step 2: Detect cycles at the NV level using Tarjan's algorithm.
    //
    // Map each unique NV to a pseudo NodeId for Tarjan's, which operates
    // on HashMap<NodeId, Vec<NodeId>>.
    let all_nvs: Vec<Rc<PackageNv>> = nv_peer_adj.keys().cloned().collect();
    let nv_to_idx: HashMap<&PackageNv, usize> = all_nvs
      .iter()
      .enumerate()
      .map(|(i, nv)| (nv.as_ref(), i))
      .collect();

    let nv_adj_for_tarjan: HashMap<NodeId, Vec<NodeId>> = all_nvs
      .iter()
      .enumerate()
      .map(|(i, nv)| {
        let to: Vec<NodeId> = nv_peer_adj
          .get(nv)
          .map(|peers| {
            peers
              .iter()
              .filter_map(|p| {
                nv_to_idx.get(p.as_ref()).map(|&j| NodeId(j as u32))
              })
              .collect()
          })
          .unwrap_or_default();
        (NodeId(i as u32), to)
      })
      .collect();

    let nv_sccs = tarjan_scc(&nv_adj_for_tarjan);

    // Build: for each NV, which SCC index is it in? And which NVs are cyclic?
    let mut nv_scc_idx: HashMap<&PackageNv, usize> = HashMap::new();
    let mut cyclic_nvs: HashSet<&PackageNv> = HashSet::new();
    for (scc_idx, scc) in nv_sccs.iter().enumerate() {
      let is_cycle = scc.len() > 1
        || (scc.len() == 1
          && nv_adj_for_tarjan
            .get(&scc[0])
            .map_or(false, |adj| adj.contains(&scc[0])));
      for &pseudo_node in scc {
        let nv = &all_nvs[pseudo_node.0 as usize];
        nv_scc_idx.insert(nv.as_ref(), scc_idx);
        if is_cycle {
          cyclic_nvs.insert(nv.as_ref());
        }
      }
    }

    // Step 3: Run Tarjan's at the node level for computation ordering.
    // This gives us SCCs in reverse topological order (leaves first).
    let node_sccs = tarjan_scc(&node_adj);

    // Step 4: Compute NpmPackageIds in topological order.
    // For each node's peer deps, check if the peer's NV forms a cycle
    // with the node's own NV. If so, use flat name@version. Otherwise
    // use the cached (already computed) full ID.
    let mut cache: HashMap<NodeId, NpmPackageId> =
      HashMap::with_capacity(self.nodes.len());

    for scc in &node_sccs {
      for &node_id in scc {
        let resolved_id = match self.resolved_node_ids.get(node_id) {
          Some(id) => id,
          None => continue,
        };

        let peers = node_peers.get(&node_id).cloned().unwrap_or_default();

        if peers.is_empty() {
          cache.insert(
            node_id,
            NpmPackageId {
              nv: (*resolved_id.nv).clone(),
              peer_dependencies: Default::default(),
            },
          );
          continue;
        }

        let my_nv_scc = nv_scc_idx.get(resolved_id.nv.as_ref()).copied();

        let mut npm_pkg_id = NpmPackageId {
          nv: (*resolved_id.nv).clone(),
          peer_dependencies: crate::NpmPackageIdPeerDependencies::with_capacity(
            peers.len(),
          ),
        };

        for (child_id, child_nv) in &peers {
          let child_nv_scc = nv_scc_idx.get(child_nv.as_ref()).copied();
          let is_in_same_nv_cycle = my_nv_scc.is_some()
            && child_nv_scc == my_nv_scc
            && cyclic_nvs.contains(child_nv.as_ref());

          if is_in_same_nv_cycle {
            // Cyclic peer deps: use flat name@version (no nested peers).
            npm_pkg_id.peer_dependencies.push(NpmPackageId {
              nv: (**child_nv).clone(),
              peer_dependencies: Default::default(),
            });
          } else if let Some(cached) = cache.get(child_id) {
            // Non-cyclic, already computed: use cached full ID
            npm_pkg_id.peer_dependencies.push(cached.clone());
          } else {
            // Fallback: flat name@version (shouldn't happen with correct
            // topological ordering, but be safe)
            npm_pkg_id.peer_dependencies.push(NpmPackageId {
              nv: (**child_nv).clone(),
              peer_dependencies: Default::default(),
            });
          }
        }

        cache.insert(node_id, npm_pkg_id);
      }
    }

    // Cap ID length: if any serialized ID exceeds the threshold,
    // replace its peer dependencies with a hash-based synthetic entry.
    const MAX_SERIALIZED_ID_LENGTH: usize = 2000;
    for npm_pkg_id in cache.values_mut() {
      if npm_pkg_id.peer_dependencies.0.is_empty() {
        continue;
      }
      let serialized = npm_pkg_id.as_serialized();
      if serialized.len() > MAX_SERIALIZED_ID_LENGTH {
        // Hash the peer suffix to produce a short, deterministic ID
        let peer_suffix = npm_pkg_id.peer_dependencies.as_serialized();
        let mut hasher = DefaultHasher::new();
        peer_suffix.as_str().hash(&mut hasher);
        let hash = hasher.finish();

        // Replace peer deps with a single synthetic entry using a name
        // that starts with '.' (which is invalid in npm), ensuring no
        // collision with real packages. The hash is encoded as a hex
        // string in a valid semver prerelease tag.
        npm_pkg_id.peer_dependencies =
          crate::NpmPackageIdPeerDependencies::from([NpmPackageId {
            nv: PackageNv {
              name: StackString::from_static(".peerhash"),
              version: Version::parse_from_npm(&format!("0.0.0-{:016x}", hash))
                .unwrap(),
            },
            peer_dependencies: Default::default(),
          }]);
      }
    }

    cache
  }

  /// Resolves a `ResolvedId` to an `NpmPackageId` using the precomputed cache.
  /// Falls back to a flat name@version if the node isn't found.
  fn get_npm_pkg_id_from_resolved_id_using_cache(
    &self,
    resolved_id: &ResolvedId,
    cache: &HashMap<NodeId, NpmPackageId>,
  ) -> NpmPackageId {
    if let Some(node_id) = self.resolved_node_ids.get_node_id(resolved_id) {
      if let Some(pkg_id) = cache.get(&node_id) {
        return pkg_id.clone();
      }
    }
    // Fallback: construct without peer dependencies
    NpmPackageId {
      nv: (*resolved_id.nv).clone(),
      peer_dependencies: Default::default(),
    }
  }

  fn peer_dep_to_maybe_node_id_and_resolved_id(
    &self,
    peer_dep: &NodeId,
  ) -> Option<(NodeId, &ResolvedId)> {
    self
      .resolved_node_ids
      .get(*peer_dep)
      .map(|resolved_id| (*peer_dep, resolved_id))
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

    let packages_to_pkg_ids = self.compute_all_npm_pkg_ids();

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
      let from_id = self.get_npm_pkg_id_from_resolved_id_using_cache(
        from_id,
        packages_to_pkg_ids,
      );
      let to_id = self.get_npm_pkg_id_from_resolved_id_using_cache(
        to_id,
        packages_to_pkg_ids,
      );
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
    let pkg_ids = self.compute_all_npm_pkg_ids();
    eprintln!("-----------");
    Self::output_node_with_ids(&self.nodes, &pkg_ids, path.node_id(), false);
    for path in path.ancestors() {
      match path {
        GraphPathNodeOrRoot::Node(node) => {
          Self::output_node_with_ids(
            &self.nodes,
            &pkg_ids,
            node.node_id(),
            false,
          );
        }
        GraphPathNodeOrRoot::Root(pkg_id) => {
          let node_id = self.root_packages.get(pkg_id).unwrap();
          let id = pkg_ids.get(node_id).map(|id| id.as_serialized());
          eprintln!(
            "Root: {} ({}: {})",
            pkg_id,
            node_id.0,
            id.as_deref().unwrap_or("?")
          )
        }
      }
    }
    eprintln!("-----------");
  }

  #[cfg(debug_assertions)]
  #[allow(unused, clippy::print_stderr)]
  fn output_node_with_ids(
    nodes: &HashMap<NodeId, Node>,
    pkg_ids: &HashMap<NodeId, NpmPackageId>,
    node_id: NodeId,
    show_children: bool,
  ) {
    let id = pkg_ids.get(&node_id).map(|id| id.as_serialized());
    eprintln!("{:>4}: {}", node_id.0, id.as_deref().unwrap_or("?"));

    if show_children {
      let node = nodes.get(&node_id).unwrap();
      eprintln!("       Children:");
      for (specifier, child_id) in &node.children {
        eprintln!("         {}: {}", specifier, child_id.0);
      }
    }
  }

  #[cfg(debug_assertions)]
  #[allow(unused, clippy::print_stderr)]
  pub fn output_nodes(&self) {
    let pkg_ids = self.compute_all_npm_pkg_ids();
    eprintln!("~~~");
    let mut node_ids = self
      .resolved_node_ids
      .node_to_resolved_id
      .keys()
      .copied()
      .collect::<Vec<_>>();
    node_ids.sort_by(|a, b| a.0.cmp(&b.0));
    for node_id in node_ids {
      Self::output_node_with_ids(&self.nodes, &pkg_ids, node_id, true);
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
  /// The initial overrides from the root package.json.
  /// Used when creating root-level GraphPaths.
  initial_overrides: Rc<NpmOverrides>,
  /// Tracks (canonical_parent_node_id, canonical_child_node_id, mode) tuples
  /// that have already been re-queued for processing. Uses canonical node IDs
  /// (via `node_id_mappings`) so that node copies from `add_peer_deps_to_path`
  /// share the same dedup entries as their originals.
  visited_requeue: HashSet<(NodeId, NodeId, GraphPathResolutionMode)>,
  /// Maps old NodeId → new NodeId when `add_peer_deps_to_path` creates a copy.
  /// Used to canonicalize node IDs for `visited_requeue` dedup, so that copies
  /// don't bypass the dedup check.
  node_id_mappings: HashMap<NodeId, NodeId>,
  // --- Phase 2: Peer resolution with caching ---
  /// Packages whose entire subtree has no externally resolved peer deps.
  /// Once marked pure, subsequent encounters skip the entire subtree.
  pure_pkgs: HashSet<Rc<PackageNv>>,
  /// Cache of peer resolution results per package version.
  /// Each entry records which peers were resolved and to what NodeId.
  /// `find_peers_cache_hit` checks if the current parent context
  /// matches a cached entry.
  peers_cache: HashMap<Rc<PackageNv>, Vec<PeersCacheEntry>>,
  /// Auto-installed peer deps as fallback, not in root_packages so they
  /// don't pollute the root scope. Phase 2 uses these when a required
  /// peer isn't found in scope.
  peer_fallbacks: BTreeMap<StackString, NodeId>,
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
      initial_overrides: Rc::new((*version_resolver.overrides).clone()),
      visited_requeue: HashSet::new(),
      node_id_mappings: HashMap::new(),
      pure_pkgs: HashSet::new(),
      peers_cache: HashMap::new(),
      peer_fallbacks: BTreeMap::new(),
    }
  }

  /// Follows the `node_id_mappings` chain to find the canonical (latest)
  /// NodeId for a node that may have been copied by `add_peer_deps_to_path`.
  /// Uses path compression so repeated lookups are O(1) amortized.
  fn canonical_node_id(&mut self, node_id: NodeId) -> NodeId {
    // Quick check: no mapping at all
    if !self.node_id_mappings.contains_key(&node_id) {
      return node_id;
    }
    // Follow the chain to find the root
    let mut current = node_id;
    while let Some(&mapped) = self.node_id_mappings.get(&current) {
      current = mapped;
    }
    // Path compression: point all intermediate nodes directly to root
    let root = current;
    let mut compress = node_id;
    while let Some(&mapped) = self.node_id_mappings.get(&compress) {
      if mapped == root {
        break;
      }
      self.node_id_mappings.insert(compress, root);
      compress = mapped;
    }
    root
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
    // check if an override applies to this root-level package:
    // try unconditional overrides first, then resolve naturally to check
    // selector-based overrides.
    // clone the Rc so we can borrow from the local rather than from self,
    // avoiding a conflict with later &mut self calls.
    let overrides = self.initial_overrides.clone();
    let req_version_req =
      match overrides.get_override_for(&package_req.name, None) {
        Some(req) => req,
        None => {
          // resolve naturally to check for selector-based overrides
          let natural_version = version_resolver
            .resolve_best_package_version_info(
              &package_req.version_req,
              self
                .graph
                .package_name_versions
                .entry(version_resolver.info().name.clone())
                .or_default()
                .iter(),
            )
            .ok()
            .map(|info| info.version.clone());
          match natural_version.as_ref().and_then(|v| {
            overrides.get_override_for(&package_req.name, Some(v))
          }) {
            Some(req) => req,
            None => &package_req.version_req,
          }
        }
      };
    let existing_root = self
      .graph
      .root_packages
      .iter()
      .find(|(nv, _id)| {
        package_req.name == nv.name
          && version_resolver
            .version_req_satisfies(req_version_req, &nv.version)
            .ok()
            .unwrap_or(false)
      })
      .map(|(nv, id)| (nv.clone(), *id));
    let (pkg_nv, node_id) = match existing_root {
      Some(existing) => existing,
      None => {
        let (pkg_nv, node_id) = self.resolve_node_from_info(
          &package_req.name,
          req_version_req,
          &version_resolver,
          None,
        )?;
        self.pending_unresolved_nodes.push_back(GraphPath::for_root(
          node_id,
          pkg_nv.clone(),
          GraphPathResolutionMode::All,
          self.initial_overrides.clone(),
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
    // check if an override applies to this dependency:
    // first try unconditional overrides (no selector), then resolve the
    // version naturally and check selector-based overrides against it.
    // parent_path is a parameter (not a field of self) so borrowing from
    // its active_overrides doesn't conflict with &mut self.
    let effective_req = match parent_path
      .active_overrides
      .get_override_for(&entry.name, None)
    {
      Some(req) => req,
      None => {
        // resolve just the version to check for selector-based overrides
        // without creating a graph node yet
        let natural_version = version_resolver
          .resolve_best_package_version_info(
            &entry.version_req,
            self
              .graph
              .package_name_versions
              .entry(version_resolver.info().name.clone())
              .or_default()
              .iter(),
          )
          .ok()
          .map(|info| info.version.clone());
        match natural_version.as_ref().and_then(|v| {
          parent_path
            .active_overrides
            .get_override_for(&entry.name, Some(v))
        }) {
          Some(req) => req,
          None => &entry.version_req,
        }
      }
    };
    let (child_nv, mut child_id) = self.resolve_node_from_info(
      &entry.name,
      effective_req,
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
    self.create_node_from_version_info(
      pkg_req_name,
      version_req,
      version_resolver,
      parent_id,
      info,
    )
  }

  /// Like `resolve_node_from_info` but skips `package_name_versions`,
  /// resolving directly from the registry.
  fn resolve_node_from_registry(
    &mut self,
    pkg_req_name: &str,
    version_req: &VersionReq,
    version_resolver: &NpmPackageVersionResolver,
    parent_id: Option<NodeId>,
  ) -> Result<(Rc<PackageNv>, NodeId), NpmResolutionError> {
    let info = version_resolver
      .resolve_best_package_version_info(version_req, std::iter::empty())?;
    self.create_node_from_version_info(
      pkg_req_name,
      version_req,
      version_resolver,
      parent_id,
      info,
    )
  }

  fn create_node_from_version_info(
    &mut self,
    pkg_req_name: &str,
    version_req: &VersionReq,
    version_resolver: &NpmPackageVersionResolver,
    parent_id: Option<NodeId>,
    info: &NpmPackageVersionInfo,
  ) -> Result<(Rc<PackageNv>, NodeId), NpmResolutionError> {
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
        Some(parent_id) => self
          .graph
          .resolved_node_ids
          .get(parent_id)
          .map(|r| r.nv.to_string())
          .unwrap_or_else(|| "?".into()),
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
    // Phase 1: resolve all regular (non-peer) dependencies via BFS.
    // Peer deps are skipped here; Phase 2 handles them.
    while let Some(parent_path) = self.pending_unresolved_nodes.pop_front() {
      self.resolve_next_pending(parent_path).await?;
    }

    // Auto-install unmet peer deps.
    // Results go into `peer_fallbacks` (NOT root_packages) to avoid
    // polluting the root scope. Phase 2 uses fallbacks only when a peer
    // isn't found in the normal scope chain.
    // Iterate because newly installed peers may themselves have peer deps.
    for _ in 0..10 {
      let prev_fallback_count = self.peer_fallbacks.len();
      self.auto_install_missing_peers().await?;
      // BFS to resolve regular deps of newly auto-installed packages
      while let Some(parent_path) = self.pending_unresolved_nodes.pop_front() {
        self.resolve_next_pending(parent_path).await?;
      }
      if self.peer_fallbacks.len() == prev_fallback_count {
        break;
      }
    }

    // Version dedup runs BEFORE Phase 2 so peer resolution sees the
    // final consolidated version set and only needs to run once.
    if self.should_dedup {
      self.run_dedup_pass().await?;
      // BFS for any deps queued by version consolidation
      while let Some(parent_path) = self.pending_unresolved_nodes.pop_front() {
        self.resolve_next_pending(parent_path).await?;
      }
    }

    // Phase 2: resolve peer dependencies top-down with caching.
    // Traverses the graph built by Phase 1, resolving peer deps from
    // parent scope and creating identity copies as needed.
    self.resolve_peers_phase()?;

    // Dedup peer dependents: merge copies
    // of the same package where one's children are a superset of another's.
    // E.g., vite@6.2.4 (no peers) and vite@6.2.4_lightningcss@1.29.2
    // (with optional peer) get merged to the superset version.
    self.dedup_peer_dependents();

    Ok(())
  }

  /// Scan all packages for peer deps that no existing package satisfies
  /// and resolve them into the `peer_fallbacks` map.
  async fn auto_install_missing_peers(
    &mut self,
  ) -> Result<(), NpmResolutionError> {
    // Collect all declared peer deps across all packages
    let mut needed: BTreeMap<StackString, VersionReq> = BTreeMap::new();
    for (resolved_id, _) in
      self.graph.resolved_node_ids.node_to_resolved_id.values()
    {
      if let Some(deps) = self.dep_entry_cache.get(&resolved_id.nv) {
        for dep in deps.iter() {
          if matches!(
            dep.kind,
            NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer
          ) {
            let effective_req = dep
              .peer_dep_version_req
              .as_ref()
              .unwrap_or(&dep.version_req);
            if !dep.kind.is_optional_peer() {
              needed
                .entry(StackString::from(dep.name.as_str()))
                .or_insert_with(|| effective_req.clone());
            }
          }
        }
      }
    }

    for (name, req) in &needed {
      // Skip if already satisfied by root_packages or fallbacks
      let already_in_root = self.graph.root_packages.iter().any(|(nv, _)| {
        nv.name.as_str() == name.as_str()
          && (req.tag().is_some() || req.matches(&nv.version))
      });
      if already_in_root || self.peer_fallbacks.contains_key(name) {
        continue;
      }

      // Resolve from registry and add to fallback (not root_packages)
      let package_info = match self.api.package_info(name.as_str()).await {
        Ok(info) => info,
        Err(_) => continue,
      };
      let version_resolver =
        self.version_resolver.get_for_package(&package_info);
      match self.resolve_node_from_registry(
        name.as_str(),
        req,
        &version_resolver,
        None,
      ) {
        Ok((child_nv, child_id)) => {
          self.peer_fallbacks.insert(name.clone(), child_id);
          // Queue BFS to resolve the fallback package's own deps
          let root_path = GraphPath::for_root(
            child_id,
            child_nv,
            GraphPathResolutionMode::All,
            self.initial_overrides.clone(),
          );
          self.pending_unresolved_nodes.push_back(root_path);
        }
        Err(_) => continue,
      }
    }

    Ok(())
  }

  // =========================================================================
  // Phase 2: Recursive peer dependency resolution with caching.
  //
  // After Phase 1 resolves all regular deps (building the graph structure),
  // Phase 2 traverses the graph top-down resolving peer deps:
  //
  // 1. Build a "scope" (parent_pkgs) of available packages at each level
  // 2. Recursively resolve children's peers first (bottom-up results)
  // 3. Resolve own peer deps from scope
  // 4. Cache results per (nv, peer_context) via pure_pkgs + peers_cache
  // 5. Create copies (new NodeIds) when peer deps change node identity
  // =========================================================================

  fn resolve_peers_phase(&mut self) -> Result<(), NpmResolutionError> {
    self.pure_pkgs.clear();
    self.peers_cache.clear();

    // Build initial scope from root packages (package name → NodeId)
    let root_scope: BTreeMap<StackString, NodeId> = self
      .graph
      .root_packages
      .iter()
      .map(|(nv, id)| (StackString::from(nv.name.as_str()), *id))
      .collect();

    // Resolve peers for each root package's children
    let roots: Vec<_> = self
      .graph
      .root_packages
      .iter()
      .map(|(nv, id)| (nv.clone(), *id))
      .collect();

    for (_nv, node_id) in &roots {
      let mut visiting = HashSet::new();
      let result = self.resolve_peers_of_node(
        *node_id,
        &root_scope,
        &mut visiting,
        &[],
      )?;
      // If the root got a copy (new node_id), update root_packages
      if result.node_id != *node_id {
        self.graph.root_packages.insert(_nv.clone(), result.node_id);
      }
    }

    Ok(())
  }

  /// After Phase 2 creates copies of packages with different peer dep
  /// configurations, merge copies where one's children are a strict superset
  /// of another's. Iterates until convergence because merging one pair can
  /// make other pairs mergeable.
  fn dedup_peer_dependents(&mut self) {
    // Group NodeIds by PackageNv (name@version). Only groups with 2+
    // entries are candidates for dedup.
    let mut nv_to_nodes: HashMap<Rc<PackageNv>, Vec<NodeId>> = HashMap::new();
    for (&node_id, (resolved_id, _)) in
      &self.graph.resolved_node_ids.node_to_resolved_id
    {
      nv_to_nodes
        .entry(resolved_id.nv.clone())
        .or_default()
        .push(node_id);
    }
    let mut duplicates: Vec<Vec<NodeId>> = nv_to_nodes
      .into_values()
      .filter(|nodes| nodes.len() > 1)
      .collect();

    if duplicates.is_empty() {
      return;
    }

    // Iterative dedup until convergence
    loop {
      let (dep_paths_map, remaining) = self.dedup_dep_paths(&duplicates);

      if dep_paths_map.is_empty() {
        break;
      }

      // Apply remappings: replace subset NodeIds with superset NodeIds
      self.apply_dedup_mappings(&dep_paths_map);

      // No remaining groups or no progress → stop
      if remaining.is_empty() || remaining.len() == duplicates.len() {
        break;
      }

      duplicates = remaining;
    }
  }

  /// Try to deduplicate groups of NodeIds that share the same PackageNv.
  /// Returns a mapping of subset → superset NodeIds and remaining
  /// unresolved groups for the next iteration.
  fn dedup_dep_paths(
    &self,
    duplicates: &[Vec<NodeId>],
  ) -> (HashMap<NodeId, NodeId>, Vec<Vec<NodeId>>) {
    let mut dep_paths_map: HashMap<NodeId, NodeId> = HashMap::new();
    let mut remaining: Vec<Vec<NodeId>> = Vec::new();

    for node_ids in duplicates {
      let mut unresolved: Vec<NodeId> = Vec::new();
      // Sort by dep count (lowest first); pop from end as superset candidate
      let mut sorted: Vec<NodeId> = node_ids.clone();
      sorted.sort_by_key(|id| self.node_deps_count(*id));

      while !sorted.is_empty() {
        let superset_candidate = sorted.pop().unwrap();
        let mut next_round = Vec::new();

        while let Some(subset_candidate) = sorted.pop() {
          if self.is_superset_node(superset_candidate, subset_candidate) {
            dep_paths_map.insert(subset_candidate, superset_candidate);
          } else {
            next_round.push(subset_candidate);
          }
        }

        if !dep_paths_map.contains_key(&superset_candidate)
          && !next_round.is_empty()
        {
          unresolved.push(superset_candidate);
        }

        sorted = next_round;
        sorted.sort_by_key(|id| self.node_deps_count(*id));
      }

      // Keep unresolved nodes for the next iteration
      if unresolved.len() > 1 {
        remaining.push(unresolved);
      }
    }

    (dep_paths_map, remaining)
  }

  /// Count total dependencies of a node (children + resolved peers).
  fn node_deps_count(&self, node_id: NodeId) -> usize {
    let children_count = self
      .graph
      .nodes
      .get(&node_id)
      .map(|n| n.children.len())
      .unwrap_or(0);
    let peer_count = self
      .graph
      .resolved_node_ids
      .get(node_id)
      .map(|id| id.peer_dependencies.len())
      .unwrap_or(0);
    children_count + peer_count
  }

  /// Check if node `a` is a superset of node `b`:
  /// 1. `a` has >= total deps than `b`
  /// 2. All of `b`'s children exist in `a` (by name, with same NV)
  /// 3. All of `b`'s resolved peer dep NVs exist in `a`'s peer deps
  ///
  /// Compares children by name+NV and peers by NV, NOT by exact NodeId.
  /// This allows merging copies where children point to different copies
  /// of the same package version.
  fn is_superset_node(&self, a: NodeId, b: NodeId) -> bool {
    if self.node_deps_count(a) < self.node_deps_count(b) {
      return false;
    }

    let (Some(node_a), Some(node_b)) =
      (self.graph.nodes.get(&a), self.graph.nodes.get(&b))
    else {
      return false;
    };

    // Check children containment by name+NV (not exact NodeId)
    for (spec, b_child_id) in &node_b.children {
      let Some(a_child_id) = node_a.children.get(spec) else {
        return false;
      };
      // Same NodeId → trivially compatible
      if a_child_id == b_child_id {
        continue;
      }
      // Different NodeId → check if same NV
      let a_nv = self.graph.resolved_node_ids.get(*a_child_id).map(|r| &r.nv);
      let b_nv = self.graph.resolved_node_ids.get(*b_child_id).map(|r| &r.nv);
      if a_nv != b_nv {
        return false;
      }
    }

    // Check peer dep containment by NV (not exact NodeId).
    let (Some(resolved_a), Some(resolved_b)) = (
      self.graph.resolved_node_ids.get(a),
      self.graph.resolved_node_ids.get(b),
    ) else {
      return false;
    };

    // All of b's peer dep NVs must appear in a's peer dep NVs.
    // Use linear scan since peer dep lists are typically very small (1-5 items).
    resolved_b.peer_dependencies.iter().all(|b_id| {
      let Some(b_nv) = self.graph.resolved_node_ids.get(*b_id).map(|r| &r.nv)
      else {
        return true;
      };
      resolved_a.peer_dependencies.iter().any(|a_id| {
        self
          .graph
          .resolved_node_ids
          .get(*a_id)
          .map(|r| &r.nv == b_nv)
          .unwrap_or(false)
      })
    })
  }

  /// Apply dedup mappings to the entire graph.
  /// Replace all references to subset NodeIds with their superset NodeIds.
  fn apply_dedup_mappings(&mut self, mappings: &HashMap<NodeId, NodeId>) {
    // Update node children
    let all_node_ids: Vec<NodeId> = self.graph.nodes.keys().copied().collect();
    for node_id in &all_node_ids {
      if let Some(node) = self.graph.nodes.get(node_id) {
        let updates: Vec<(StackString, NodeId)> = node
          .children
          .iter()
          .filter_map(|(spec, child_id)| {
            mappings.get(child_id).map(|new_id| (spec.clone(), *new_id))
          })
          .collect();
        for (spec, new_id) in updates {
          self.graph.set_child_of_parent_node(*node_id, &spec, new_id);
        }
      }
    }

    // Update root_packages
    for (_nv, root_node_id) in self.graph.root_packages.iter_mut() {
      if let Some(new_id) = mappings.get(root_node_id) {
        *root_node_id = *new_id;
      }
    }

    // Update resolved_node_ids peer_dependencies
    let nodes_with_peers: Vec<NodeId> = self
      .graph
      .resolved_node_ids
      .node_to_resolved_id
      .keys()
      .copied()
      .collect();
    for node_id in nodes_with_peers {
      if let Some(resolved) = self.graph.resolved_node_ids.get(node_id).cloned()
      {
        if resolved.peer_dependencies.is_empty() {
          continue;
        }
        let mut updated = false;
        let mut new_peers = resolved.peer_dependencies.clone();
        for id in new_peers.iter_mut() {
          if let Some(new_id) = mappings.get(id) {
            *id = *new_id;
            updated = true;
          }
        }
        if updated {
          let updated_resolved = ResolvedId {
            nv: resolved.nv.clone(),
            peer_dependencies: new_peers,
          };
          self.graph.resolved_node_ids.set(node_id, updated_resolved);
        }
      }
    }

    // Remove merged (subset) nodes from resolved_node_ids so they
    // don't appear in the snapshot (compute_all_npm_pkg_ids iterates
    // node_to_resolved_id). We keep them in graph.nodes since other
    // code may still reference them by NodeId.
    for subset_id in mappings.keys() {
      self.graph.resolved_node_ids.remove(*subset_id);
    }
  }

  /// Recursively resolve peer deps for a node and its subtree.
  ///
  /// Returns which peers bubble up (resolved from outside this subtree)
  /// and the final NodeId (possibly a copy with peers in identity).
  fn resolve_peers_of_node(
    &mut self,
    node_id: NodeId,
    parent_pkgs: &BTreeMap<StackString, NodeId>,
    visiting: &mut HashSet<NodeId>,
    ancestors: &[PackageNv],
  ) -> Result<PeersResolution, NpmResolutionError> {
    let nv = self
      .graph
      .resolved_node_ids
      .get(node_id)
      .unwrap()
      .nv
      .clone();

    // Pure check: if this package's entire subtree has no external peers,
    // skip entirely.
    if self.pure_pkgs.contains(&nv) {
      return Ok(PeersResolution {
        resolved_peers: BTreeMap::new(),
        missing_peers: BTreeMap::new(),
        unresolved_optional_peers: Vec::new(),
        node_id,
      });
    }

    // Cycle detection: if we're already resolving this node, stop.
    // Unlike a simple empty return, we peek at the node's declared peer
    // deps and return those that are resolvable from the current scope.
    // This ensures transitive peer deps propagate through cycles
    // (e.g., a→b→c→d→c where c peers with b: d needs b as transitive peer).
    if !visiting.insert(node_id) {
      let mut resolved_peers = BTreeMap::new();
      let mut missing_peers = BTreeMap::new();
      if let Some(deps) = self.dep_entry_cache.get(&nv) {
        for dep in deps.iter() {
          match dep.kind {
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              let found = parent_pkgs
                .get(&dep.bare_specifier)
                .or_else(|| parent_pkgs.get(dep.name.as_str()))
                .or_else(|| self.peer_fallbacks.get(&dep.bare_specifier))
                .or_else(|| self.peer_fallbacks.get(dep.name.as_str()));
              if let Some(&peer_id) = found {
                resolved_peers.insert(dep.bare_specifier.clone(), peer_id);
              } else if !dep.kind.is_optional_peer() {
                let effective_req = dep
                  .peer_dep_version_req
                  .as_ref()
                  .unwrap_or(&dep.version_req);
                missing_peers
                  .insert(dep.bare_specifier.clone(), effective_req.clone());
              }
            }
            _ => {}
          }
        }
      }
      return Ok(PeersResolution {
        resolved_peers,
        missing_peers,
        unresolved_optional_peers: Vec::new(),
        node_id,
      });
    }

    // Build scope for children: parent_pkgs + this node itself + children.
    // We add the current node to scope so children and grandchildren can
    // find ancestor packages when resolving peer deps. Children override
    // parent scope entries (closer scope wins).
    let node = self.graph.nodes.get(&node_id).unwrap();
    let mut scope = parent_pkgs.clone();
    scope.insert(StackString::from(nv.name.as_str()), node_id);
    for (spec, child_id) in &node.children {
      // Use the package NAME as key (not the specifier/alias)
      if let Some(child_resolved) = self.graph.resolved_node_ids.get(*child_id)
      {
        scope.insert(
          StackString::from(child_resolved.nv.name.as_str()),
          *child_id,
        );
      }
      // Also add by specifier (alias) since peer deps may reference by alias
      scope.insert(spec.clone(), *child_id);
    }

    // Cache check: if a previous resolution for this nv with
    // equivalent peer context exists, reuse it.
    if let Some(hit) = self.find_peers_cache_hit(&nv, parent_pkgs) {
      visiting.remove(&node_id);
      return Ok(hit);
    }

    // Step 1: Resolve this node's own peer deps from parent_pkgs FIRST,
    // before recursing into children.
    let deps = self.dep_entry_cache.get(&nv).cloned().unwrap_or_default();
    let mut own_peer_deps: Vec<(StackString, NodeId)> = Vec::new();
    let mut all_resolved_peers = BTreeMap::new();
    let mut all_missing_peers = BTreeMap::new();
    let mut all_unresolved_optional_peers: Vec<StackString> = Vec::new();

    for dep in deps.iter() {
      match dep.kind {
        NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer => {
          // Skip self-referencing peer deps (a package peering with itself)
          if dep.name.as_str() == nv.name.as_str() {
            continue;
          }
          // Look up by specifier first, then by package name,
          // then fall back to auto-installed peers
          let found = parent_pkgs
            .get(&dep.bare_specifier)
            .or_else(|| parent_pkgs.get(dep.name.as_str()))
            .or_else(|| self.peer_fallbacks.get(&dep.bare_specifier))
            .or_else(|| self.peer_fallbacks.get(dep.name.as_str()));
          if let Some(&peer_id) = found {
            if let Some(peer_resolved) =
              self.graph.resolved_node_ids.get(peer_id)
            {
              let effective_req = dep
                .peer_dep_version_req
                .as_ref()
                .unwrap_or(&dep.version_req);
              let satisfies = effective_req.tag().is_some()
                || effective_req.matches(&peer_resolved.nv.version);
              own_peer_deps.push((dep.bare_specifier.clone(), peer_id));
              all_resolved_peers.insert(dep.bare_specifier.clone(), peer_id);
              if !satisfies && !dep.kind.is_optional_peer() {
                let mut diag_ancestors =
                  Vec::with_capacity(ancestors.len() + 1);
                diag_ancestors.push((*nv).clone());
                diag_ancestors.extend_from_slice(ancestors);
                self.unmet_peer_diagnostics.borrow_mut().insert(
                  UnmetPeerDepDiagnostic {
                    ancestors: diag_ancestors,
                    dependency: PackageReq {
                      name: dep.name.clone(),
                      version_req: dep.version_req.clone(),
                    },
                    resolved: peer_resolved.nv.version.clone(),
                  },
                );
              }
            }
          } else if !dep.kind.is_optional_peer() {
            let effective_req = dep
              .peer_dep_version_req
              .as_ref()
              .unwrap_or(&dep.version_req);
            all_missing_peers
              .insert(dep.bare_specifier.clone(), effective_req.clone());
          } else {
            // Unresolved optional peer — track for cache invalidation
            all_unresolved_optional_peers.push(dep.bare_specifier.clone());
          }
        }
        _ => {} // regular deps already resolved in Phase 1
      }
    }

    // Step 2: Add resolved peers to scope so children can see them.
    for (spec, peer_id) in &own_peer_deps {
      if let Some(peer_resolved) = self.graph.resolved_node_ids.get(*peer_id) {
        scope
          .entry(StackString::from(peer_resolved.nv.name.as_str()))
          .or_insert(*peer_id);
      }
      scope.entry(spec.clone()).or_insert(*peer_id);
    }

    // Step 3: Recurse into regular children AND resolved peer dep nodes.
    let children: Vec<_> = self
      .graph
      .nodes
      .get(&node_id)
      .unwrap()
      .children
      .iter()
      .map(|(s, id)| (s.clone(), *id))
      .collect();

    // Build the list of all deps to recurse into: regular children + peer deps
    let mut all_deps_to_recurse: Vec<(StackString, NodeId)> = children.clone();
    for (spec, peer_id) in &own_peer_deps {
      // Only recurse into peer deps that aren't already regular children
      if !children.iter().any(|(s, _)| s == spec) {
        all_deps_to_recurse.push((spec.clone(), *peer_id));
      }
    }

    let mut resolved_children: BTreeMap<StackString, NodeId> = BTreeMap::new();

    let mut child_ancestors = Vec::with_capacity(ancestors.len() + 1);
    child_ancestors.push((*nv).clone());
    child_ancestors.extend_from_slice(ancestors);

    for (spec, child_id) in &all_deps_to_recurse {
      let child_result = self.resolve_peers_of_node(
        *child_id,
        &scope,
        visiting,
        &child_ancestors,
      )?;
      resolved_children.insert(spec.clone(), child_result.node_id);
      for (name, id) in child_result.resolved_peers {
        all_resolved_peers.insert(name, id);
      }
      for (name, req) in child_result.missing_peers {
        all_missing_peers.insert(name, req);
      }
      all_unresolved_optional_peers
        .extend(child_result.unresolved_optional_peers);
    }

    // Step 4a: Update all_resolved_peers for own peer deps to use the
    // resolved (copy) node_ids. The initial insertion used the original
    // peer_id, but after recursion the peer may have gotten a copy.
    for (spec, _) in &own_peer_deps {
      if let Some(&copy_id) = resolved_children.get(spec) {
        all_resolved_peers.insert(spec.clone(), copy_id);
      }
    }

    // Step 4b: Update sibling cross-references.
    // When siblings are peers of each other, copies initially reference
    // the ORIGINAL nodes. Update them to reference copies.
    {
      let original_to_copy: HashMap<NodeId, NodeId> = all_deps_to_recurse
        .iter()
        .filter_map(|(spec, orig_id)| {
          let copy_id = resolved_children.get(spec)?;
          if *copy_id != *orig_id {
            Some((*orig_id, *copy_id))
          } else {
            None
          }
        })
        .collect();

      if !original_to_copy.is_empty() {
        let copy_ids: Vec<NodeId> =
          resolved_children.values().copied().collect();
        for child_copy_id in &copy_ids {
          if let Some(child_node) = self.graph.nodes.get(child_copy_id) {
            let updates: Vec<_> = child_node
              .children
              .iter()
              .filter_map(|(spec, id)| {
                original_to_copy.get(id).map(|copy| (spec.clone(), *copy))
              })
              .collect();
            for (spec, copy_id) in updates {
              self.graph.set_child_of_parent_node(
                *child_copy_id,
                &spec,
                copy_id,
              );
            }
          }
        }
      }
    }

    // Determine the final NodeId. If peer deps were resolved, create a copy
    // with the peer deps in its ResolvedId.
    let children_changed = !resolved_children.iter().all(|(s, id)| {
      children
        .iter()
        .find(|(cs, _)| cs == s)
        .is_some_and(|(_, cid)| cid == id)
    });

    // Check if there are transitive peers that would be added to identity.
    // These are peers from children that are NOT own children or own peers.
    let original_child_names_set: HashSet<&StackString> =
      children.iter().map(|(s, _)| s).collect();
    let has_transitive_peers = all_resolved_peers.keys().any(|name| {
      !original_child_names_set.contains(name)
        && !own_peer_deps.iter().any(|(s, _)| s == name)
        && name.as_str() != nv.name.as_str()
    });

    let final_node_id =
      if own_peer_deps.is_empty() && !children_changed && !has_transitive_peers
      {
        // No changes needed — same node
        node_id
      } else {
        // Build new ResolvedId with peer deps.
        // Start fresh (don't clone from old) since we're computing the
        // complete set. This ensures that when Phase 2 re-runs, updated
        // copies (with nested peers) replace stale entries.
        let old_resolved_id =
          self.graph.resolved_node_ids.get(node_id).unwrap().clone();
        let mut new_peer_deps: Vec<NodeId> = Vec::new();
        let mut seen_nvs: HashSet<Rc<PackageNv>> = HashSet::new();

        // Add own peer deps first
        for (spec, _orig_peer_id) in &own_peer_deps {
          let resolved_peer_id = resolved_children
            .get(spec)
            .copied()
            .unwrap_or(*_orig_peer_id);
          let peer_nv = self
            .graph
            .resolved_node_ids
            .get(resolved_peer_id)
            .unwrap()
            .nv
            .clone();
          if seen_nvs.insert(peer_nv) {
            new_peer_deps.push(resolved_peer_id);
          }
        }

        // Add transitive peer deps from children
        for (name, peer_id) in &all_resolved_peers {
          if resolved_children.contains_key(name)
            || own_peer_deps.iter().any(|(s, _)| s == name)
            || name.as_str() == nv.name.as_str()
          {
            continue;
          }
          let peer_nv = self
            .graph
            .resolved_node_ids
            .get(*peer_id)
            .unwrap()
            .nv
            .clone();
          if seen_nvs.insert(peer_nv) {
            new_peer_deps.push(*peer_id);
          }
        }

        let new_resolved_id = ResolvedId {
          nv: old_resolved_id.nv.clone(),
          peer_dependencies: new_peer_deps,
        };

        let (created, new_node_id) =
          self.graph.get_or_create_for_id(&new_resolved_id);

        // resolved_children includes both regular deps and peer dep nodes
        // (from all_deps_to_recurse), with correct copy NodeIds.
        for (spec, child_id) in &resolved_children {
          self
            .graph
            .set_child_of_parent_node(new_node_id, spec, *child_id);
        }
        if created {
          // Copy no_peers flag
          let old_no_peers = self.graph.nodes.get(&node_id).unwrap().no_peers;
          self.graph.borrow_node_mut(new_node_id).no_peers = old_no_peers;
        }

        // Step 5: Fix circular parent references. When children have
        // circular peer deps back to this node (e.g., expo-plugin peers
        // with expo), the recursion returns the ORIGINAL parent node_id
        // because the parent is in `visiting`. Now that we have the
        // parent's copy, update all children that reference the original
        // parent to reference the copy instead.
        // This updates BOTH node.children AND ResolvedId.peer_dependencies,
        // since the NpmPackageId computation uses the latter.
        if new_node_id != node_id {
          for child_copy_id in resolved_children.values() {
            // Update node children
            if let Some(child_node) = self.graph.nodes.get(child_copy_id) {
              let updates: Vec<_> = child_node
                .children
                .iter()
                .filter(|(_, id)| **id == node_id)
                .map(|(spec, _)| spec.clone())
                .collect();
              for spec in updates {
                self.graph.set_child_of_parent_node(
                  *child_copy_id,
                  &spec,
                  new_node_id,
                );
              }
            }
            // Update ResolvedId peer_dependencies that reference original
            if let Some(child_resolved) =
              self.graph.resolved_node_ids.get(*child_copy_id).cloned()
            {
              let mut updated = false;
              let mut new_peers = child_resolved.peer_dependencies.clone();
              for id in new_peers.iter_mut() {
                if *id == node_id {
                  *id = new_node_id;
                  updated = true;
                }
              }
              if updated {
                let updated_resolved = ResolvedId {
                  nv: child_resolved.nv.clone(),
                  peer_dependencies: new_peers,
                };
                self
                  .graph
                  .resolved_node_ids
                  .set(*child_copy_id, updated_resolved);
              }
            }
          }
        }

        new_node_id
      };

    // Filter: only bubble up peers NOT provided by this node's regular
    // deps (Phase 1 children). Peers that this node provides via its own
    // regular deps are "consumed" here and don't bubble further up.
    // We use the original `children` vec (Phase 1 regular deps), NOT the
    // copy node's children (which includes newly-added peer deps).
    let original_child_names: HashSet<&StackString> =
      children.iter().map(|(s, _)| s).collect();
    let bubbling_peers: BTreeMap<StackString, NodeId> = all_resolved_peers
      .into_iter()
      .filter(|(name, _)| {
        !original_child_names.contains(name)
          && name.as_str() != nv.name.as_str()
      })
      .collect();

    let bubbling_missing: BTreeMap<StackString, VersionReq> = all_missing_peers
      .into_iter()
      .filter(|(name, _)| {
        !original_child_names.contains(name)
          && name.as_str() != nv.name.as_str()
      })
      .collect();

    // Filter unresolved optional peers: only bubble up those not provided
    // by own regular children, and deduplicate.
    let bubbling_unresolved_optional: Vec<StackString> =
      all_unresolved_optional_peers
        .into_iter()
        .filter(|name| {
          !original_child_names.contains(name)
            && name.as_str() != nv.name.as_str()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    // Cache the result.
    // A package is "pure" only if nothing bubbles up AND there are no
    // unresolved optional peers (which could be resolved in a different
    // parent context).
    let is_pure = bubbling_peers.is_empty()
      && bubbling_missing.is_empty()
      && bubbling_unresolved_optional.is_empty();
    if is_pure {
      self.pure_pkgs.insert(nv);
    } else {
      self
        .peers_cache
        .entry(nv)
        .or_default()
        .push(PeersCacheEntry {
          resolved_peers: bubbling_peers.clone(),
          missing_peers: bubbling_missing.clone(),
          unresolved_optional_peers: bubbling_unresolved_optional.clone(),
          node_id: final_node_id,
        });
    }

    visiting.remove(&node_id);

    Ok(PeersResolution {
      resolved_peers: bubbling_peers,
      missing_peers: bubbling_missing,
      unresolved_optional_peers: bubbling_unresolved_optional,
      node_id: final_node_id,
    })
  }

  /// Check if a cached peer resolution matches the current parent context.
  fn find_peers_cache_hit(
    &self,
    nv: &PackageNv,
    parent_pkgs: &BTreeMap<StackString, NodeId>,
  ) -> Option<PeersResolution> {
    let mut checking = HashSet::new();
    self.find_peers_cache_hit_inner(nv, parent_pkgs, &mut checking)
  }

  fn find_peers_cache_hit_inner(
    &self,
    nv: &PackageNv,
    parent_pkgs: &BTreeMap<StackString, NodeId>,
    checking: &mut HashSet<PackageNv>,
  ) -> Option<PeersResolution> {
    let entries = self.peers_cache.get(nv)?;
    if !checking.insert(nv.clone()) {
      // Prevent infinite recursion for circular deps
      return None;
    }
    for entry in entries {
      let mut all_match = true;

      // Check each resolved peer: is the same peer available in current scope?
      for (name, cached_id) in &entry.resolved_peers {
        match parent_pkgs.get(name) {
          Some(current_id) if current_id == cached_id => continue,
          Some(current_id) => {
            // Different NodeId. Check if same package version.
            let cached_nv =
              self.graph.resolved_node_ids.get(*cached_id).map(|r| &r.nv);
            let current_nv =
              self.graph.resolved_node_ids.get(*current_id).map(|r| &r.nv);
            if cached_nv != current_nv {
              all_match = false;
              break;
            }
            // Same version. If pure, it's equivalent.
            if let Some(cnv) = cached_nv {
              if self.pure_pkgs.contains(cnv) {
                continue;
              }
              // Not pure. Check recursively if the non-pure
              // package would resolve the same way in this context.
              if self
                .find_peers_cache_hit_inner(cnv, parent_pkgs, checking)
                .is_some()
              {
                continue;
              }
              all_match = false;
              break;
            }
          }
          None => {
            all_match = false;
            break;
          }
        }
      }

      if !all_match {
        continue;
      }

      // Check that previously missing peers are still missing
      for name in entry.missing_peers.keys() {
        if parent_pkgs.contains_key(name) {
          all_match = false;
          break;
        }
      }

      if !all_match {
        continue;
      }

      // Check that previously unresolved optional peers are still
      // unavailable. If one is now available, this cache entry doesn't
      // apply — the package should be re-resolved with the optional
      // peer in scope.
      for name in &entry.unresolved_optional_peers {
        if parent_pkgs.contains_key(name)
          || self.peer_fallbacks.contains_key(name)
        {
          all_match = false;
          break;
        }
      }

      if all_match {
        checking.remove(nv);
        return Some(PeersResolution {
          resolved_peers: entry.resolved_peers.clone(),
          missing_peers: entry.missing_peers.clone(),
          unresolved_optional_peers: entry.unresolved_optional_peers.clone(),
          node_id: entry.node_id,
        });
      }
    }
    checking.remove(nv);
    None
  }

  async fn resolve_next_pending(
    &mut self,
    parent_path: Rc<GraphPath>,
  ) -> Result<(), NpmResolutionError> {
    let (_parent_nv, child_deps) = {
      let node_id = parent_path.node_id();
      if self.graph.nodes.get(&node_id).unwrap().no_peers {
        // Skip: no reason to analyze this graph segment further
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
              } else if !self.graph.nodes.get(&child_id).unwrap().no_peers && {
                // Only requeue if we haven't already queued this canonical
                // (parent, child, mode) tuple. Using canonical IDs means
                // node copies share dedup entries with originals.
                let cp = self.canonical_node_id(parent_id);
                let cc = self.canonical_node_id(child_id);
                self.visited_requeue.insert((cp, cc, parent_path.mode))
              } {
                self.pending_unresolved_nodes.push_back(child_path);
              }
              child_id
            }
            None => {
              // check if an alias override replaces this dependency's package
              if let Some(alias_name) =
                parent_path.active_overrides.get_alias_for(&dep.name)
              {
                let alias_info =
                  self.api.package_info(alias_name.as_str()).await?;
                let alias_resolver =
                  self.version_resolver.get_for_package(&alias_info);
                self.analyze_dependency(dep, &alias_resolver, &parent_path)?
              } else {
                self.analyze_dependency(dep, &version_resolver, &parent_path)?
              }
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
          // Phase 1: Skip peer dep resolution entirely. Phase 2 will
          // resolve peer deps from parent scope. The auto-install step
          // between Phase 1 and Phase 2 handles unmet peers.
          found_peer = true;
        }
      }
    }

    if !found_peer {
      self.graph.borrow_node_mut(parent_path.node_id()).no_peers = true;
    }

    Ok(())
  }

  fn add_peer_deps_to_path(
    &mut self,
    // path from the node above the resolved dep to just above the peer dep
    path: &[&Rc<GraphPath>],
    peer_deps: &[(&NodeId, Rc<PackageNv>)],
  ) {
    debug_assert!(!path.is_empty());

    for graph_path_node in path.iter().rev() {
      let old_node_id = graph_path_node.node_id();

      // Don't propagate peer deps to ancestors that already have the
      // peer package as a direct child.
      let filtered_peer_deps: Vec<_> = {
        let node = self.graph.nodes.get(&old_node_id).unwrap();
        peer_deps
          .iter()
          .filter(|(_, nv)| !node.children.contains_key(nv.name.as_str()))
          .cloned()
          .collect()
      };
      if filtered_peer_deps.is_empty() {
        continue;
      }

      let old_resolved_id =
        self.graph.resolved_node_ids.get(old_node_id).unwrap();

      let Some(new_resolved_id) =
        self.add_peer_deps_to_id(old_resolved_id, &filtered_peer_deps)
      else {
        continue; // nothing to change
      };

      let old_resolved_id = old_resolved_id.clone();
      let (created, new_node_id) =
        self.graph.get_or_create_for_id(&new_resolved_id);

      // Track old → new so visited_requeue canonicalization treats
      // the copy the same as the original for dedup purposes.
      if old_node_id != new_node_id {
        self.node_id_mappings.insert(old_node_id, new_node_id);
      }

      if created {
        let old_node = self.graph.borrow_node_mut(old_node_id);
        let old_children = old_node.children.clone();
        let old_no_peers = old_node.no_peers;
        // copy over the old children to this new one
        for (specifier, child_id) in &old_children {
          self.graph.set_child_of_parent_node(
            new_node_id,
            specifier,
            *child_id,
          );
        }
        // Preserve the no_peers flag so the new node isn't unnecessarily
        // re-processed. The peer dep being added to the ID doesn't affect
        // the node's own deps or its children's peer status.
        self.graph.borrow_node_mut(new_node_id).no_peers = old_no_peers;

        // the moved_package_ids is only used to update copy indexes
        // at the end, so only bother inserting if it's not empty
        if !self.graph.packages_to_copy_index.is_empty() {
          // Store how package ids were moved. The order is important
          // here because one id might be moved around a few times
          let new_value = (old_resolved_id.clone(), new_resolved_id.clone());
          match self.graph.moved_package_ids.entry(old_node_id) {
            indexmap::map::Entry::Occupied(occupied_entry) => {
              // move it to the back of the index map
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
    peer_deps: &[(&NodeId, Rc<PackageNv>)],
  ) -> Option<ResolvedId> {
    let mut new_resolved_id = Cow::Borrowed(id);
    // Collect existing peer dep NVs for dedup. Extract NVs directly
    // from the NodeIds to avoid the expensive
    // peer_dep_to_maybe_node_id_and_resolved_id call which iterates
    // all children of each parent node O(n_children).
    let peer_nvs = new_resolved_id
      .peer_dependencies
      .iter()
      .filter_map(|node_id| {
        self
          .graph
          .resolved_node_ids
          .get(*node_id)
          .map(|r| r.nv.clone())
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
          new_id.peer_dependencies.push(**peer_dep);
          new_resolved_id = Cow::Owned(new_id);
        }
        Cow::Owned(new_id) => {
          new_id.peer_dependencies.push(**peer_dep);
        }
      }
    }
    match new_resolved_id {
      Cow::Borrowed(_) => None,
      Cow::Owned(id) => Some(id),
    }
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
          self
            .graph
            .resolved_node_ids
            .get(*peer_dep)
            .unwrap()
            .nv
            .clone(),
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
                .push(
                  dep
                    .peer_dep_version_req
                    .as_ref()
                    .unwrap_or(&dep.version_req)
                    .clone(),
                );
              if seen_nodes.insert(*child_node_id) {
                pending_nodes.push_back(*child_node_id);
              }
            }
          }
        }
      }
    }

    // Include peer-dep version requirements for auto-installed peer
    // fallbacks. These packages are in `peer_fallbacks` but not
    // connected as children of any node (Phase 2 hasn't run yet),
    // so the traversal above doesn't see them. Without this,
    // run_dedup_pass can't consolidate a version that only exists
    // as a peer fallback.
    for (name, &fallback_node_id) in &self.peer_fallbacks {
      let Some(fallback_id) =
        self.graph.resolved_node_ids.get(fallback_node_id)
      else {
        continue;
      };
      let fallback_nv = fallback_id.nv.clone();
      // Collect peer dep version requirements from all packages
      // that declare a peer dep on this fallback package.
      for (resolved_id, _) in
        self.graph.resolved_node_ids.node_to_resolved_id.values()
      {
        let Some(deps) = self.dep_entry_cache.get(&resolved_id.nv) else {
          continue;
        };
        for dep in deps.iter() {
          if dep.name.as_str() == name.as_str()
            && matches!(
              dep.kind,
              NpmDependencyEntryKind::Peer
                | NpmDependencyEntryKind::OptionalPeer
            )
          {
            let effective_req = dep
              .peer_dep_version_req
              .as_ref()
              .unwrap_or(&dep.version_req);
            package_version_reqs_by_version
              .entry(fallback_nv.name.clone())
              .or_default()
              .entry(fallback_nv.version.clone())
              .or_default()
              .push(effective_req.clone());
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

    // Update peer_fallbacks to point to consolidated versions.
    // Auto-installed peers may have been resolved to a version that
    // was consolidated away (e.g., package-peer@1.2.0 → 1.1.0).
    for (_name, node_id) in self.peer_fallbacks.iter_mut() {
      let current_nv = {
        let Some(resolved_id) = self.graph.resolved_node_ids.get(*node_id)
        else {
          continue;
        };
        resolved_id.nv.clone()
      };
      let Some(versions_by_req) = consolidated_versions.get(&current_nv.name)
      else {
        continue;
      };
      let target_versions: HashSet<&Version> =
        versions_by_req.values().collect();
      if target_versions.contains(&current_nv.version) {
        continue; // already at a target version
      }
      // Find the target version and update the fallback entry.
      if let Some(target_version) = target_versions.into_iter().next() {
        let new_nv = Rc::new(PackageNv {
          name: current_nv.name.clone(),
          version: target_version.clone(),
        });
        let new_resolved_id = ResolvedId {
          nv: new_nv,
          peer_dependencies: Vec::new(),
        };
        let (_, new_node_id) =
          self.graph.get_or_create_for_id(&new_resolved_id);
        *node_id = new_node_id;
      }
    }

    // Remove children that point to consolidated-away versions.
    // Phase 1 BFS (run by the caller after this function) will
    // re-resolve them to the target versions.
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
        let effective_req = dep
          .peer_dep_version_req
          .as_ref()
          .unwrap_or(&dep.version_req);
        if versions.contains_key(effective_req) {
          node.children.remove(&dep.bare_specifier);
        }
      }
    }

    // Clean up stale state. No need to clear peer deps since
    // run_dedup_pass now runs BEFORE Phase 2.
    self.graph.moved_package_ids.clear();
    self.graph.packages_to_copy_index.clear();

    // Queue all root packages for Phase 1 BFS to re-resolve
    // children that were removed above.
    for (pkg_nv, node_id) in &self.graph.root_packages {
      self.pending_unresolved_nodes.push_back(GraphPath::for_root(
        *node_id,
        pkg_nv.clone(),
        GraphPathResolutionMode::All,
        self.initial_overrides.clone(),
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

  let all_pkg_ids = graph.compute_all_npm_pkg_ids();

  TraceGraphSnapshot {
    nodes: graph
      .nodes
      .iter()
      .map(|(node_id, node)| {
        let id = all_pkg_ids.get(node_id).cloned().unwrap_or_else(|| {
          let resolved_id = graph.resolved_node_ids.get(*node_id).unwrap();
          NpmPackageId {
            nv: (*resolved_id.nv).clone(),
            peer_dependencies: Default::default(),
          }
        });
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
          pkg_id: "package-0@1.0.0".to_string(),
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
            "package-1@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-1@1.0.0".to_string(),
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
            "package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
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
            "package-a@1.0.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0".to_string(),
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
          pkg_id: "package-a@1.0.0_package-peer@4.1.0".to_string(),
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
      vec![(
        "package-a@1".to_string(),
        "package-a@1.0.0_package-peer@4.1.0".to_string()
      )]
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
          pkg_id: "package-a@1.0.0".to_string(),
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
        ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
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
          pkg_id: "package-b@1.0.0".to_string(),
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
        ("package-b@1".to_string(), "package-b@1.0.0".to_string())
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
        pkg_id: "package-b@1.0.0".to_string(),
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
      ("package-b@1".to_string(), "package-b@1.0.0".to_string()),
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
          pkg_id: "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0"
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
        "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0_package-peer-b@3.0.0"
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
          pkg_id: "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
            .to_string(),
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
          "package-0@1.0.0_package-peer-a@2.0.0__package-peer-b@3.0.0"
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
          pkg_id: "package-0@1.1.1_package-peer-b@5.4.1_package-peer-c@6.2.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-a".to_string(),
            "package-a@1.0.0_package-peer-b@5.4.1_package-peer-c@6.2.0".to_string(),
          )]),
        },
        TestNpmResolutionPackage {
          pkg_id: "package-a@1.0.0_package-peer-b@5.4.1_package-peer-c@6.2.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "package-b".to_string(),
              "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0_package-peer-b@5.4.1".to_string(),
            ),
            (
              "package-c".to_string(),
              "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-b@5.4.1".to_string(),
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
          pkg_id: "package-b@2.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-c@6.2.0_package-peer-b@5.4.1".to_string(),
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
          pkg_id: "package-c@3.0.0_package-peer-a@4.0.0__package-peer-b@5.4.1_package-peer-b@5.4.1".to_string(),
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
        (
          "package-0@1.1.1".to_string(),
          "package-0@1.1.1_package-peer-b@5.4.1_package-peer-c@6.2.0"
            .to_string()
        ),
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
            pkg_id: "package-a@1.0.0".to_string(),
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
            pkg_id: "package-b@2.0.0".to_string(),
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
          ("package-a@1".to_string(), "package-a@1.0.0".to_string()),
          ("package-b@2".to_string(), "package-b@2.0.0".to_string())
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
          pkg_id: "package-a@1.0.0_package-b@1.0.0__package-peer@1.0.0"
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
            pkg_id: "package-b@1.0.0".to_string(),
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
          ("package-b@1.0".to_string(), "package-b@1.0.0".to_string())
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
            pkg_id: "package-b@1.0.0".to_string(),
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
          ("package-b@1.0".to_string(), "package-b@1.0.0".to_string())
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
          pkg_id: "package-a@1.0.0".to_string(),
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
      vec![("package-a@1.0".to_string(), "package-a@1.0.0".to_string()),]
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
              "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
            ),
            (
              "package-e".to_string(),
              "package-e@1.0.0_package-a@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string()
            )
          ]),

        },
        TestNpmResolutionPackage {
          pkg_id: "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
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
            "package-d@1.0.0_package-b@1.0.0__package-a@1.0.0".to_string(),
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
        pkg_id: "package-a@1.0.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
          (
            "package-b".to_string(),
            "package-b@1.0.0_package-c@1.0.0".to_string(),
          ),
          (
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0".to_string(),
          ),
        ]),
      },
      TestNpmResolutionPackage {
        // This is stored like so:
        //   b (id: 0) -> c (id: 1) -> b (id: 0)
        // So it's circular. Storing a circular dependency serialized here is a
        // little difficult, so when this is encountered we assume it's circular.
        // I have a feeling this is not exactly correct, but perhaps it is good enough
        // and edge cases won't be seen in the wild...
        pkg_id: "package-b@1.0.0_package-c@1.0.0".to_string(),
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
          "package-b@1.0.0_package-c@1.0.0".to_string(),
        )]),
      },
    ];
    let (packages, package_reqs) =
      run_resolver_and_get_output(api.clone(), vec!["package-a@1.0.0"]).await;
    assert_eq!(packages, expected_packages.clone());
    assert_eq!(
      package_reqs,
      vec![("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string())]
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
      vec![
        ("package-a@1.0.0".to_string(), "package-a@1.0.0".to_string()),
        (
          "package-b@1.0.0".to_string(),
          "package-b@1.0.0_package-c@1.0.0".to_string()
        )
      ]
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
          pkg_id: "package-a@1.0.0_package-b@1.0.0".to_string(),
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
      vec![(
        "package-a@1.0.0".to_string(),
        "package-a@1.0.0_package-b@1.0.0".to_string()
      )]
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
        pkg_id: "package-b@1.0.0".to_string(),
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
        copy_index: 1,
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
        ("package-b@1".to_string(), "package-b@1.0.0".to_string(),),
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
        ("package-b@1".to_string(), "package-b@1.0.0".to_string(),),
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
            pkg_id: "package-b@1.0.0".to_string(),
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
          ("package-b@1.0".to_string(), "package-b@1.0.0".to_string())
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
            pkg_id: "package-b@1.0.0".to_string(),
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
          ("package-b@1.0".to_string(), "package-b@1.0.0".to_string())
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
    // After dedup_peer_dependents, the bare client-sso-oidc is merged into
    // the superset (with client-sts peer dep), so all references now point
    // to the superset version with nested peer dep chains.
    let expected_packages = Vec::from([
      TestNpmResolutionPackage {
        pkg_id: "@aws-sdk/client-s3@3.679.0".to_string(),
        copy_index: 0,
        dependencies: BTreeMap::from([
            ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0".to_string()),
            ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0".to_string())
        ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0".to_string()),
              ("@aws-sdk/credential-provider-node".to_string(), "@aws-sdk/credential-provider-node@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-ini@3.679.0_@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sts".to_string(), "@aws-sdk/client-sts@3.679.0".to_string()),
              ("@aws-sdk/credential-provider-sso".to_string(), "@aws-sdk/credential-provider-sso@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-node@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/credential-provider-ini".to_string(), "@aws-sdk/credential-provider-ini@3.679.0_@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      },
      TestNpmResolutionPackage {
          pkg_id: "@aws-sdk/credential-provider-sso@3.679.0_@aws-sdk+client-sso-oidc@3.679.0__@aws-sdk+client-sts@3.679.0_@aws-sdk+client-sts@3.679.0".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
              ("@aws-sdk/client-sso-oidc".to_string(), "@aws-sdk/client-sso-oidc@3.679.0_@aws-sdk+client-sts@3.679.0".to_string()),
          ]),
      }]
    );
    assert_eq!(packages, expected_packages);
    assert_eq!(
      package_reqs,
      vec![(
        "@aws-sdk/client-s3@3.679.0".to_string(),
        "@aws-sdk/client-s3@3.679.0".to_string(),
      )]
    );

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
    assert_eq!(
      package_reqs,
      vec![
        (
          "@aws-sdk/client-s3".to_string(),
          "@aws-sdk/client-s3@3.679.0".to_string(),
        ),
        (
          "@aws-sdk/client-s3@3.679.0".to_string(),
          "@aws-sdk/client-s3@3.679.0".to_string(),
        )
      ]
    );
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
    // The closer scope wins: when package-b's peer dep package-peer is
    // resolved in the context of package-c (which has package-peer@1.0.1
    // as a dep), it resolves to 1.0.1, not the ancestor's 1.0.2. This
    // creates two copies of package-b.
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
            pkg_id: "package-a@1.0.0_package-peer@1.0.2".to_string(),
            copy_index: 0,
            dependencies: BTreeMap::from([(
              "package-b".to_string(),
              "package-b@1.0.0_package-peer@1.0.2".to_string(),
            ), (
              "package-c".to_string(),
              "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1".to_string(),
            ), (
              "package-peer".to_string(),
              "package-peer@1.0.2".to_string()
            )])
          },
          TestNpmResolutionPackage {
            pkg_id: "package-b@1.0.0_package-peer@1.0.1".to_string(),
            copy_index: 1,
            dependencies: BTreeMap::from([(
              "package-peer".to_string(),
              "package-peer@1.0.1".to_string(),
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
            pkg_id: "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1".to_string(),
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
          (
            "package-a@1.0.0".to_string(),
            "package-a@1.0.0_package-peer@1.0.2".to_string()
          ),
          (
            "package-peer@1".to_string(),
            "package-peer@1.0.2".to_string()
          ),
        ]
      );
    }

    // dedup pass should consolidate to 1.0.1.
    // After consolidation, package-b's two copies (one with peer@1.0.2,
    // one with peer@1.0.1) become identical and get merged.
    // package-d's identity includes both package-b (own peer) and
    // package-peer (transitive from package-b).
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
          pkg_id: "package-a@1.0.0_package-peer@1.0.1".to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([(
            "package-b".to_string(),
            "package-b@1.0.0_package-peer@1.0.1".to_string(),
          ), (
            "package-c".to_string(),
            "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1".to_string(),
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
          pkg_id: "package-c@1.0.0_package-b@1.0.0__package-peer@1.0.1".to_string(),
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
          (
            "package-a@1.0.0".to_string(),
            "package-a@1.0.0_package-peer@1.0.1".to_string()
          ),
          (
            "package-peer@1".to_string(),
            "package-peer@1.0.1".to_string()
          ),
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
          pkg_id: "@tailwindcss/vite@4.0.17_vite@6.2.4__lightningcss@1.29.2"
            .to_string(),
          copy_index: 0,
          dependencies: BTreeMap::from([
            (
              "lightningcss".to_string(),
              "lightningcss@1.29.2".to_string(),
            ),
            (
              "vite".to_string(),
              "vite@6.2.4_lightningcss@1.29.2".to_string(),
            )
          ])
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
    assert_eq!(
      package_reqs,
      vec![
        (
          "@deno/vite-plugin@~1.0.4".to_string(),
          "@deno/vite-plugin@1.0.4_vite@6.2.4__lightningcss@1.29.2".to_string()
        ),
        (
          "@tailwindcss/vite@~4.0.17".to_string(),
          "@tailwindcss/vite@4.0.17_vite@6.2.4__lightningcss@1.29.2"
            .to_string()
        ),
      ]
    );
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
        ..Default::default()
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
    overrides: crate::resolution::NpmOverrides,
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
      overrides: Arc::new(options.overrides),
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
      overrides: Default::default(),
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

  // === npm overrides integration tests ===

  fn make_overrides(
    json: serde_json::Value,
  ) -> crate::resolution::NpmOverrides {
    crate::resolution::NpmOverrides::from_value(json, &Default::default())
      .unwrap()
  }

  fn make_overrides_with_root_deps(
    json: serde_json::Value,
    root_deps: std::collections::HashMap<
      deno_semver::StackString,
      deno_semver::StackString,
    >,
  ) -> crate::resolution::NpmOverrides {
    crate::resolution::NpmOverrides::from_value(json, &root_deps).unwrap()
  }

  #[tokio::test]
  async fn override_simple_version() {
    // "foo": "1.0.0" should force foo@1.0.0 everywhere, even though
    // package-a asks for foo@^2.0.0
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("foo", "1.0.0");
    api.ensure_package_version("foo", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("foo", "^2.0.0"));

    let (packages, _package_reqs) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo": "1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // foo should be resolved to 1.0.0, not 2.0.0
    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.pkg_id, "foo@1.0.0");
  }

  #[tokio::test]
  async fn override_does_not_affect_unrelated_packages() {
    // override for "foo" should not affect "bar"
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("foo", "1.0.0");
    api.ensure_package_version("foo", "2.0.0");
    api.ensure_package_version("bar", "1.0.0");
    api.ensure_package_version("bar", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("foo", "^2.0.0"));
    api.add_dependency(("package-a", "1.0.0"), ("bar", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo": "1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.pkg_id, "foo@1.0.0");
    // bar should still resolve to 2.0.0 as normal
    let bar_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("bar@"))
      .unwrap();
    assert_eq!(bar_pkg.pkg_id, "bar@2.0.0");
  }

  #[tokio::test]
  async fn override_scoped_to_parent() {
    // "parent": { "child": "1.0.0" } should only override child
    // when it's under parent's subtree
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("parent", "1.0.0");
    api.ensure_package_version("other", "1.0.0");
    api.ensure_package_version("child", "1.0.0");
    api.ensure_package_version("child", "2.0.0");
    // both parent and other depend on child@^2.0.0
    api.add_dependency(("parent", "1.0.0"), ("child", "^2.0.0"));
    api.add_dependency(("other", "1.0.0"), ("child", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["parent@1.0.0", "other@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "parent": {
            "child": "1.0.0"
          }
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // parent's child should be 1.0.0 (overridden)
    let parent_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("parent@"))
      .unwrap();
    assert_eq!(parent_pkg.dependencies.get("child").unwrap(), "child@1.0.0");
    // other's child should be 2.0.0 (not overridden)
    let other_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("other@"))
      .unwrap();
    assert_eq!(other_pkg.dependencies.get("child").unwrap(), "child@2.0.0");
  }

  #[tokio::test]
  async fn override_with_version_selector() {
    // "foo@^2.0.0": { "bar": "1.0.0" }
    // should only override bar when foo resolves to 2.x
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("foo", "2.1.0");
    api.ensure_package_version("bar", "1.0.0");
    api.ensure_package_version("bar", "3.0.0");
    api.add_dependency(("foo", "2.1.0"), ("bar", "^3.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["foo@^2.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo@^2.0.0": {
            "bar": "1.0.0"
          }
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // bar should be overridden to 1.0.0
    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.dependencies.get("bar").unwrap(), "bar@1.0.0");
  }

  #[tokio::test]
  async fn override_version_selector_no_match() {
    // "foo@^3.0.0": { "bar": "1.0.0" }
    // should NOT override bar when foo resolves to 2.x
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("foo", "2.1.0");
    api.ensure_package_version("bar", "1.0.0");
    api.ensure_package_version("bar", "3.0.0");
    api.add_dependency(("foo", "2.1.0"), ("bar", "^3.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["foo@^2.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo@^3.0.0": {
            "bar": "1.0.0"
          }
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // bar should NOT be overridden (selector doesn't match)
    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.dependencies.get("bar").unwrap(), "bar@3.0.0");
  }

  #[tokio::test]
  async fn override_dollar_reference() {
    // "bar": "$bar" should resolve to the root dependency's version of bar
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("bar", "1.0.0");
    api.ensure_package_version("bar", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("bar", "^2.0.0"));

    let mut root_deps = std::collections::HashMap::new();
    root_deps.insert(
      deno_semver::StackString::from("bar"),
      deno_semver::StackString::from("^1.0.0"),
    );

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0", "bar@^1.0.0"],
        overrides: make_overrides_with_root_deps(
          serde_json::json!({
            "bar": "$bar"
          }),
          root_deps,
        ),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // bar should be resolved to 1.0.0 everywhere
    let bar_pkgs: Vec<_> = packages
      .iter()
      .filter(|p| p.pkg_id.starts_with("bar@"))
      .collect();
    assert_eq!(bar_pkgs.len(), 1);
    assert_eq!(bar_pkgs[0].pkg_id, "bar@1.0.0");
  }

  #[tokio::test]
  async fn override_transitive_dependency() {
    // override should apply to deeply nested transitive deps
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("leaf", "1.0.0");
    api.ensure_package_version("leaf", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("leaf", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "leaf": "1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // leaf should be 1.0.0 even though it's two levels deep
    let leaf_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("leaf@"))
      .unwrap();
    assert_eq!(leaf_pkg.pkg_id, "leaf@1.0.0");
  }

  #[tokio::test]
  async fn override_with_dot_key() {
    // "foo@^2.0.0": { ".": "2.0.0", "bar": "1.0.0" }
    // should override foo itself and also bar within foo's tree
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("foo", "2.0.0");
    api.ensure_package_version("foo", "2.1.0");
    api.ensure_package_version("bar", "1.0.0");
    api.ensure_package_version("bar", "3.0.0");
    api.add_dependency(("foo", "2.0.0"), ("bar", "^3.0.0"));
    api.add_dependency(("foo", "2.1.0"), ("bar", "^3.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["foo@^2.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo@^2.0.0": {
            ".": "2.0.0",
            "bar": "1.0.0"
          }
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // foo should resolve to 2.0.0 (overridden by "." key)
    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.pkg_id, "foo@2.0.0");
    // bar within foo should be 1.0.0
    assert_eq!(foo_pkg.dependencies.get("bar").unwrap(), "bar@1.0.0");
  }

  #[tokio::test]
  async fn override_no_overrides_unchanged() {
    // with empty overrides, resolution should behave exactly as before
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("foo", "2.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("foo", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: Default::default(),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    let foo_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("foo@"))
      .unwrap();
    assert_eq!(foo_pkg.pkg_id, "foo@2.0.0");
  }

  #[tokio::test]
  async fn override_npm_alias() {
    // "foo": "npm:bar@1.0.0" should resolve foo to bar@1.0.0
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("foo", "2.0.0");
    api.ensure_package_version("bar", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("foo", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo": "npm:bar@1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // foo should not appear — bar@1.0.0 should be resolved instead
    assert!(
      packages.iter().all(|p| !p.pkg_id.starts_with("foo@")),
      "foo should not be in the resolved packages"
    );
    let bar_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("bar@"))
      .unwrap();
    assert_eq!(bar_pkg.pkg_id, "bar@1.0.0");
    // package-a's dependency "foo" should point to bar@1.0.0
    let parent = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("package-a@"))
      .unwrap();
    assert_eq!(parent.dependencies.get("foo").unwrap(), "bar@1.0.0");
  }

  #[tokio::test]
  async fn override_npm_alias_transitive() {
    // npm alias override should work on transitive dependencies too
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("leaf", "2.0.0");
    api.ensure_package_version("replacement", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("leaf", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "leaf": "npm:replacement@1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // leaf should not be in the graph; replacement@1.0.0 should be
    assert!(
      packages.iter().all(|p| !p.pkg_id.starts_with("leaf@")),
      "leaf should not be in the resolved packages"
    );
    let replacement = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("replacement@"))
      .unwrap();
    assert_eq!(replacement.pkg_id, "replacement@1.0.0");
    // package-b's "leaf" dep should point to replacement@1.0.0
    let pkg_b = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("package-b@"))
      .unwrap();
    assert_eq!(pkg_b.dependencies.get("leaf").unwrap(), "replacement@1.0.0");
  }

  #[tokio::test]
  async fn override_npm_alias_scoped_to_parent() {
    // "parent": { "child": "npm:alt@1.0.0" }
    // should only alias child under parent, not under other
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("parent", "1.0.0");
    api.ensure_package_version("other", "1.0.0");
    api.ensure_package_version("child", "2.0.0");
    api.ensure_package_version("alt", "1.0.0");
    api.add_dependency(("parent", "1.0.0"), ("child", "^2.0.0"));
    api.add_dependency(("other", "1.0.0"), ("child", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["parent@1.0.0", "other@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "parent": {
            "child": "npm:alt@1.0.0"
          }
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // parent's "child" should resolve to alt@1.0.0
    let parent_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("parent@"))
      .unwrap();
    assert_eq!(parent_pkg.dependencies.get("child").unwrap(), "alt@1.0.0");
    // other's "child" should still be child@2.0.0 (unaffected)
    let other_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("other@"))
      .unwrap();
    assert_eq!(other_pkg.dependencies.get("child").unwrap(), "child@2.0.0");
  }

  #[tokio::test]
  async fn override_jsr_alias() {
    // "foo": "jsr:@std/path@1.0.0" should resolve foo to @jsr/std__path@1.0.0
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("foo", "2.0.0");
    api.ensure_package_version("@jsr/std__path", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("foo", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "foo": "jsr:@std/path@1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // foo should not appear — @jsr/std__path@1.0.0 should be resolved instead
    assert!(
      packages.iter().all(|p| !p.pkg_id.starts_with("foo@")),
      "foo should not be in the resolved packages"
    );
    let jsr_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("@jsr/std__path@"))
      .unwrap();
    assert_eq!(jsr_pkg.pkg_id, "@jsr/std__path@1.0.0");
    // package-a's "foo" dep should point to @jsr/std__path@1.0.0
    let parent = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("package-a@"))
      .unwrap();
    assert_eq!(
      parent.dependencies.get("foo").unwrap(),
      "@jsr/std__path@1.0.0"
    );
  }

  #[tokio::test]
  async fn override_jsr_version_only() {
    // "@std/path": "jsr:1.0.0" should derive the jsr name from the key
    // and resolve to @jsr/std__path@1.0.0
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("@std/path", "2.0.0");
    api.ensure_package_version("@jsr/std__path", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("@std/path", "^2.0.0"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1.0.0"],
        overrides: make_overrides(serde_json::json!({
          "@std/path": "jsr:1.0.0"
        })),
        snapshot: Default::default(),
        link_packages: None,
        expected_diagnostics: Vec::new(),
        newest_dependency_date: Default::default(),
        skip_dedup: false,
      },
    )
    .await;

    // @std/path should not appear — @jsr/std__path@1.0.0 should be resolved
    assert!(
      packages.iter().all(|p| !p.pkg_id.starts_with("@std/path@")),
      "@std/path should not be in the resolved packages"
    );
    let jsr_pkg = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("@jsr/std__path@"))
      .unwrap();
    assert_eq!(jsr_pkg.pkg_id, "@jsr/std__path@1.0.0");
    // package-a's "@std/path" dep should point to @jsr/std__path@1.0.0
    let parent = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("package-a@"))
      .unwrap();
    assert_eq!(
      parent.dependencies.get("@std/path").unwrap(),
      "@jsr/std__path@1.0.0"
    );
  }

  /// Auto-resolved peer deps must not depend on BFS traversal order.
  ///
  /// ```text
  ///   package-a ──dep──→ lib ──peerDep──→ react
  ///   package-b ──dep──→ bridge ──dep──→ react@^18.3.0
  ///             └─dep──→ wrapper ──dep──→ lib ──peerDep──→ react
  /// ```
  ///
  /// `bridge` adds react@18.3.0 to the graph at BFS depth 1. Without the
  /// fix, `lib`'s peer dep would pick react@18.2.0 (latest) on one path
  /// but react@18.3.0 (highest existing) on the other, creating a
  /// duplicate `lib` entry. Both paths should resolve to the same version.
  #[tokio::test]
  async fn peer_dep_bfs_order_dependent_version_selection() {
    let api = TestNpmRegistryApi::default();

    api.ensure_package_version("react", "18.2.0");
    api.ensure_package_version("react", "18.3.0");
    api.add_dist_tag("react", "latest", "18.2.0");

    api.ensure_package_version("lib", "1.0.0");
    api.add_peer_dependency(("lib", "1.0.0"), ("react", "^18.0.0"));

    api.ensure_package_version("package-a", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("lib", "^1.0.0"));

    api.ensure_package_version("bridge", "1.0.0");
    api.add_dependency(("bridge", "1.0.0"), ("react", "^18.3.0"));

    api.ensure_package_version("wrapper", "1.0.0");
    api.add_dependency(("wrapper", "1.0.0"), ("lib", "^1.0.0"));

    api.ensure_package_version("package-b", "1.0.0");
    api.add_dependency(("package-b", "1.0.0"), ("bridge", "^1.0.0"));
    api.add_dependency(("package-b", "1.0.0"), ("wrapper", "^1.0.0"));

    let input_reqs = vec!["package-a@1", "package-b@1"];

    for skip_dedup in [true, false] {
      let (packages, _) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          reqs: input_reqs.clone(),
          skip_dedup,
          ..Default::default()
        },
      )
      .await;

      let lib_entries: Vec<_> = packages
        .iter()
        .filter(|p| p.pkg_id.starts_with("lib@1.0.0"))
        .collect();

      assert_eq!(
        lib_entries.len(),
        1,
        "lib should appear exactly once (skip_dedup={skip_dedup}). \
        Packages: {:#?}",
        packages,
      );
    }
  }

  /// Re-resolution with a snapshot must not create duplicate peer dep
  /// entries when a newer version has been published.
  ///
  /// ```text
  ///   framework ──dep──→ lib ──peerDep──→ react  (snapshot: react@18.2.0)
  ///   new-package ──dep──→ bridge ──dep──→ react@^18.3.0
  ///               └─dep──→ wrapper ──dep──→ lib ──peerDep──→ react
  /// ```
  ///
  /// After react@18.3.0 is published and becomes "latest", adding
  /// `new-package` must not create a second `lib` entry.
  #[tokio::test]
  async fn new_version_published_causes_peer_dep_duplicates() {
    let api = TestNpmRegistryApi::default();

    api.ensure_package_version("react", "18.2.0");
    api.add_dist_tag("react", "latest", "18.2.0");

    api.ensure_package_version("lib", "1.0.0");
    api.add_peer_dependency(("lib", "1.0.0"), ("react", "^18.0.0"));

    api.ensure_package_version("framework", "1.0.0");
    api.add_dependency(("framework", "1.0.0"), ("lib", "^1.0.0"));

    let snapshot = run_resolver_with_options_and_get_snapshot(
      &api,
      RunResolverOptions {
        reqs: vec!["framework@1"],
        ..Default::default()
      },
    )
    .await
    .unwrap();

    let (initial_packages, _) = snapshot_to_packages(snapshot.clone());
    let initial_lib = initial_packages
      .iter()
      .find(|p| p.pkg_id.starts_with("lib@1.0.0"))
      .unwrap();
    assert_eq!(
      initial_lib.dependencies.get("react").unwrap(),
      "react@18.2.0",
    );

    // Publish react@18.3.0 as the new latest
    api.ensure_package_version("react", "18.3.0");
    api.add_dist_tag("react", "latest", "18.3.0");

    api.ensure_package_version("bridge", "1.0.0");
    api.add_dependency(("bridge", "1.0.0"), ("react", "^18.3.0"));
    api.ensure_package_version("wrapper", "1.0.0");
    api.add_dependency(("wrapper", "1.0.0"), ("lib", "^1.0.0"));
    api.ensure_package_version("new-package", "1.0.0");
    api.add_dependency(("new-package", "1.0.0"), ("bridge", "^1.0.0"));
    api.add_dependency(("new-package", "1.0.0"), ("wrapper", "^1.0.0"));

    for skip_dedup in [true, false] {
      let (packages, _) = run_resolver_with_options_and_get_output(
        api.clone(),
        RunResolverOptions {
          snapshot: snapshot.clone(),
          reqs: vec!["new-package@1"],
          skip_dedup,
          ..Default::default()
        },
      )
      .await;

      let lib_entries: Vec<_> = packages
        .iter()
        .filter(|p| p.pkg_id.starts_with("lib@1.0.0"))
        .collect();

      assert_eq!(
        lib_entries.len(),
        1,
        "lib should appear exactly once (skip_dedup={skip_dedup}). \
        Found {}: {:#?}\nAll packages: {:#?}",
        lib_entries.len(),
        lib_entries,
        packages,
      );

      let framework_pkg = packages
        .iter()
        .find(|p| p.pkg_id.starts_with("framework@"))
        .unwrap();
      let wrapper_pkg = packages
        .iter()
        .find(|p| p.pkg_id.starts_with("wrapper@"))
        .unwrap();
      assert_eq!(
        framework_pkg.dependencies.get("lib").unwrap(),
        wrapper_pkg.dependencies.get("lib").unwrap(),
        "framework and wrapper should share the same lib \
        (skip_dedup={skip_dedup}). Packages: {:#?}",
        packages,
      );
    }
  }

  /// Regression test: mimics the Expo/React Native pattern where:
  /// - "expo" depends on many expo-* packages
  /// - Each expo-* package peer-depends on "react", "react-native", AND "expo" itself
  /// - "react-native" peer-depends on "react" and has many regular deps
  /// - This creates circular peer deps + transitive peer propagation
  ///
  /// Without fixes, the resolver re-processes the same subtrees exponentially
  /// because peer dep additions create new ancestor node copies that lose the
  /// no_peers optimization flag, and the queue grows unboundedly.
  #[tokio::test]
  async fn resolve_many_siblings_same_peer_deps() {
    let api = TestNpmRegistryApi::default();

    // === Core packages (like react, react-native, @types/react) ===
    api.ensure_package_version("react", "1.0.0");
    api.ensure_package_version("react-native", "1.0.0");
    api.ensure_package_version("types-react", "1.0.0");

    // react-native peers with react and types-react (like real react-native)
    api.add_peer_dependency(("react-native", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(("react-native", "1.0.0"), ("types-react", "*"));

    // react-native has many regular deps (ws, yargs, etc.)
    for i in 0..15 {
      let dep_name = format!("rn-dep-{}", i);
      api.ensure_package_version(&dep_name, "1.0.0");
      api.add_dependency(("react-native", "1.0.0"), (&dep_name, "1"));
    }

    // === "expo" package ===
    api.ensure_package_version("expo", "1.0.0");

    // expo depends on many expo-* plugins AND on cli/metro-runtime
    api.ensure_package_version("expo-cli", "1.0.0");
    api.ensure_package_version("expo-metro-runtime", "1.0.0");
    api.add_dependency(("expo", "1.0.0"), ("expo-cli", "1"));
    api.add_dependency(("expo", "1.0.0"), ("expo-metro-runtime", "1"));

    // expo-cli and expo-metro-runtime peer with expo (circular!)
    api.add_peer_dependency(("expo-cli", "1.0.0"), ("expo", "*"));
    api.add_peer_dependency(("expo-cli", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(("expo-metro-runtime", "1.0.0"), ("expo", "*"));
    api.add_peer_dependency(("expo-metro-runtime", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(
      ("expo-metro-runtime", "1.0.0"),
      ("react-native", "*"),
    );

    // expo-cli has many deps
    for i in 0..10 {
      let dep_name = format!("cli-dep-{}", i);
      api.ensure_package_version(&dep_name, "1.0.0");
      api.add_dependency(("expo-cli", "1.0.0"), (&dep_name, "1"));
    }

    // === Many expo-* plugins that all peer with react, react-native, expo ===
    let plugin_count: usize = 100;
    for i in 0..plugin_count {
      let name = format!("expo-plugin-{}", i);
      api.ensure_package_version(&name, "1.0.0");
      api.add_dependency(("expo", "1.0.0"), (&name, "1"));

      // Each plugin peers with the big three (like expo-font, expo-image, etc.)
      api.add_peer_dependency((&name, "1.0.0"), ("react", "*"));
      api.add_peer_dependency((&name, "1.0.0"), ("react-native", "*"));
      api.add_peer_dependency((&name, "1.0.0"), ("expo", "*")); // circular!
      api.add_peer_dependency((&name, "1.0.0"), ("types-react", "*"));

      // Each plugin has a couple of internal deps
      for j in 0..2 {
        let dep_name = format!("expo-plugin-{}-dep-{}", i, j);
        api.ensure_package_version(&dep_name, "1.0.0");
        api.add_dependency((&name, "1.0.0"), (&dep_name, "1"));
      }
    }

    // === "expo-router" - peers with even more things ===
    api.ensure_package_version("expo-router", "1.0.0");
    api.add_dependency(("expo", "1.0.0"), ("expo-router", "1"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("react-native", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("expo", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("types-react", "*"));
    api.add_peer_dependency(
      ("expo-router", "1.0.0"),
      ("expo-metro-runtime", "*"),
    );
    // router depends on some plugins too
    for i in 0..3 {
      let name = format!("expo-plugin-{}", i);
      api.add_dependency(("expo-router", "1.0.0"), (&name, "1"));
    }

    // Root reqs: all expo-* plugins are ALSO root deps (like in real Expo
    // package.json where expo-font, expo-image etc. are direct deps)
    let mut root_reqs = vec![
      "expo@1",
      "react@1",
      "react-native@1",
      "types-react@1",
      "expo-router@1",
    ];
    for i in 0..plugin_count {
      // We need to leak the strings to get &'static str for the vec
      let req: &'static str =
        Box::leak(format!("expo-plugin-{}@1", i).into_boxed_str());
      root_reqs.push(req);
    }

    // Use tokio timeout to catch infinite loops
    let result = tokio::time::timeout(
      std::time::Duration::from_secs(30),
      run_resolver_and_get_output(api, root_reqs),
    )
    .await;

    let (packages, _package_reqs) =
      result.expect("Resolution timed out - likely exponential blowup");

    // Verify resolution completed with reasonable package count
    assert!(
      packages.len() >= 20,
      "Expected at least 20 packages, got {}",
      packages.len()
    );

    // Verify plugins got their peer deps resolved
    let plugin_0 = packages
      .iter()
      .find(|p| p.pkg_id.starts_with("expo-plugin-0@"))
      .expect("expo-plugin-0 should exist");
    assert!(
      plugin_0.dependencies.contains_key("react"),
      "expo-plugin-0 should have react as dependency, got: {:?}",
      plugin_0.dependencies
    );
    assert!(
      plugin_0.dependencies.contains_key("react-native"),
      "expo-plugin-0 should have react-native as dependency, got: {:?}",
      plugin_0.dependencies
    );
  }

  /// When a package is referenced twice in the dependency graph and one of
  /// the times it cannot resolve its peers, still try to resolve it in the
  /// other occurrence.
  ///
  /// package-b -> package-a(peer:package-peer) : can't resolve peer (not in scope)
  /// package-c -> package-b -> package-a(peer:package-peer) + package-c -> package-peer : CAN resolve
  ///
  /// Expected: package-a appears with peer resolved from package-c's scope.
  /// package-b should also appear (transitive peer propagation).
  #[tokio::test]
  async fn package_referenced_twice_one_resolves_peer() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");

    api.add_peer_dependency(("package-a", "1.0.0"), ("package-peer", "*"));
    api.add_dependency(("package-b", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-peer", "1"));

    let (packages, _) =
      run_resolver_and_get_output(api, vec!["package-b@1", "package-c@1"])
        .await;

    let pkg_ids: Vec<&str> =
      packages.iter().map(|p| p.pkg_id.as_str()).collect();
    assert!(
      pkg_ids.iter().any(|id| *id == "package-peer@1.0.0"),
      "should have package-peer@1.0.0. Got: {:?}",
      pkg_ids
    );
    assert!(
      pkg_ids.iter().any(|id| *id == "package-c@1.0.0"),
      "should have package-c@1.0.0. Got: {:?}",
      pkg_ids
    );

    // package-a should appear with resolved peer
    let a_with_peer = packages.iter().any(|p| {
      p.pkg_id.contains("package-a@1.0.0") && p.pkg_id.contains("package-peer")
    });
    assert!(
      a_with_peer,
      "package-a should have an instance with peer resolved. Got: {:?}",
      pkg_ids
    );

    // package-b should appear (transitive peer propagated through package-a)
    let b_entries: Vec<_> = packages
      .iter()
      .filter(|p| p.pkg_id.starts_with("package-b@"))
      .collect();
    assert!(
      b_entries.len() >= 1,
      "package-b should have at least 1 entry. Got: {:?}",
      pkg_ids
    );
  }

  /// Resolve peer dependencies of cyclic dependencies.
  ///
  /// package-a(peers: package-c, package-d) -> package-b(peers: package-a, package-d) ->
  ///   package-c(peers: package-a, package-b) -> package-d(peers: package-c) -> package-a, package-b
  #[tokio::test]
  async fn cyclic_peer_deps() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");

    // package-a has peers: package-c, package-d
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-c", "*"));
    api.add_peer_dependency(("package-a", "1.0.0"), ("package-d", "*"));

    // package-b depends on package-c, has peers: package-a, package-d
    api.add_dependency(("package-b", "1.0.0"), ("package-c", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-a", "*"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-d", "*"));

    // package-c has peers: package-a, package-b
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-a", "*"));
    api.add_peer_dependency(("package-c", "1.0.0"), ("package-b", "*"));

    // package-d depends on package-a and package-b, has peer: package-c
    api.add_dependency(("package-d", "1.0.0"), ("package-a", "1"));
    api.add_dependency(("package-d", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-d", "1.0.0"), ("package-c", "*"));

    // package-a is the root entry that kicks off the chain
    // package-a -> package-b -> package-c -> package-d -> package-a (cycle!)
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));

    let (packages, _) =
      run_resolver_and_get_output(api, vec!["package-a@1"]).await;

    // Expected at least 4 unique entries
    let pkg_ids: Vec<&str> =
      packages.iter().map(|p| p.pkg_id.as_str()).collect();
    assert!(
      pkg_ids.len() >= 4,
      "Expected at least 4 unique package entries for cyclic peers, got {}: {:?}",
      pkg_ids.len(),
      pkg_ids
    );

    // All four packages should be present
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-a@")),
      "should have package-a. Got: {:?}",
      pkg_ids
    );
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-b@")),
      "should have package-b. Got: {:?}",
      pkg_ids
    );
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-c@")),
      "should have package-c. Got: {:?}",
      pkg_ids
    );
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-d@")),
      "should have package-d. Got: {:?}",
      pkg_ids
    );
  }

  /// Should ignore conflicts between missing optional peer dependencies.
  ///
  /// package-a has optional peer package-peer@^1, package-b has optional peer package-peer@^2.
  /// Neither is installed. Should NOT produce any conflict diagnostics.
  #[tokio::test]
  async fn optional_peer_conflicts_ignored() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.ensure_package_version("package-peer", "2.0.0");

    api.add_optional_peer_dependency(
      ("package-a", "1.0.0"),
      ("package-peer", "^1"),
    );
    api.add_optional_peer_dependency(
      ("package-b", "1.0.0"),
      ("package-peer", "^2"),
    );

    // Don't install package-peer — both peers are optional and unresolved
    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1", "package-b@1"],
        expected_diagnostics: vec![], // NO diagnostics expected
        ..Default::default()
      },
    )
    .await;

    // package-a and package-b should be resolved without package-peer
    let pkg_ids: Vec<&str> =
      packages.iter().map(|p| p.pkg_id.as_str()).collect();
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-a@")),
      "should have package-a. Got: {:?}",
      pkg_ids
    );
    assert!(
      pkg_ids.iter().any(|id| id.starts_with("package-b@")),
      "should have package-b. Got: {:?}",
      pkg_ids
    );
  }

  /// Should report where the bad peer dependency is resolved from.
  ///
  /// package-a depends on package-peer@1 and package-b. package-b has peer package-peer@^10.
  /// package-peer@1 is found in scope but doesn't satisfy ^10 → unmet peer diag.
  #[tokio::test]
  async fn bad_peer_version_from_subdep() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-peer", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");

    api.add_dependency(("package-a", "1.0.0"), ("package-peer", "1"));
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_peer_dependency(("package-b", "1.0.0"), ("package-peer", "^10"));

    let (packages, _) = run_resolver_with_options_and_get_output(
      api,
      RunResolverOptions {
        reqs: vec!["package-a@1"],
        expected_diagnostics: vec![
          "package-a@1.0.0 -> package-b@1.0.0: package-peer@^10 -> 1.0.0",
        ],
        ..Default::default()
      },
    )
    .await;

    assert!(
      packages.iter().any(|p| p.pkg_id.starts_with("package-a@")),
      "should have package-a"
    );
    assert!(
      packages
        .iter()
        .any(|p| p.pkg_id.starts_with("package-peer@")),
      "should have package-peer"
    );
    assert!(
      packages.iter().any(|p| p.pkg_id.starts_with("package-b@")),
      "should have package-b"
    );
  }

  /// Test that NpmPackageId serialization length stays bounded.
  /// With the Expo-like pattern of many plugins with circular peer deps,
  /// no single package ID should exceed a reasonable length.
  /// We assert < 2000 chars as a safety margin.
  #[tokio::test]
  async fn peer_dep_id_length_bounded() {
    let api = TestNpmRegistryApi::default();

    // Core packages
    api.ensure_package_version("react", "1.0.0");
    api.ensure_package_version("react-native", "1.0.0");
    api.ensure_package_version("types-react", "1.0.0");

    // react-native peers with react and types-react
    api.add_peer_dependency(("react-native", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(("react-native", "1.0.0"), ("types-react", "*"));

    // expo package
    api.ensure_package_version("expo", "1.0.0");

    // 10 expo plugins, each peering with react, react-native, expo
    for i in 0..10 {
      let name = format!("expo-plugin-{}", i);
      api.ensure_package_version(&name, "1.0.0");
      api.add_dependency(("expo", "1.0.0"), (&name, "1"));
      api.add_peer_dependency((&name, "1.0.0"), ("react", "*"));
      api.add_peer_dependency((&name, "1.0.0"), ("react-native", "*"));
      api.add_peer_dependency((&name, "1.0.0"), ("expo", "*")); // circular!
      api.add_peer_dependency((&name, "1.0.0"), ("types-react", "*"));
    }

    // expo-router peers with expo (circular)
    api.ensure_package_version("expo-router", "1.0.0");
    api.add_dependency(("expo", "1.0.0"), ("expo-router", "1"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("react", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("react-native", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("expo", "*"));
    api.add_peer_dependency(("expo-router", "1.0.0"), ("types-react", "*"));

    let mut root_reqs = vec![
      "expo@1",
      "react@1",
      "react-native@1",
      "types-react@1",
      "expo-router@1",
    ];
    for i in 0..10 {
      let req: &'static str =
        Box::leak(format!("expo-plugin-{}@1", i).into_boxed_str());
      root_reqs.push(req);
    }

    let result = tokio::time::timeout(
      std::time::Duration::from_secs(30),
      run_resolver_and_get_output(api, root_reqs),
    )
    .await;

    let (packages, _) = result.expect("Resolution timed out");

    // Assert no package ID exceeds 2000 characters
    for pkg in &packages {
      assert!(
        pkg.pkg_id.len() < 2000,
        "Package ID too long ({} chars): {}...",
        pkg.pkg_id.len(),
        &pkg.pkg_id[..100.min(pkg.pkg_id.len())]
      );
    }
  }
}
