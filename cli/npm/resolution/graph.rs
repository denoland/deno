// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;
use log::debug;

use crate::npm::cache::should_sync_download;
use crate::npm::registry::NpmDependencyEntry;
use crate::npm::registry::NpmDependencyEntryKind;
use crate::npm::registry::NpmPackageInfo;
use crate::npm::registry::NpmPackageVersionInfo;
use crate::npm::semver::NpmVersion;
use crate::npm::semver::NpmVersionReq;
use crate::npm::NpmRegistryApi;

use super::snapshot::NpmResolutionSnapshot;
use super::snapshot::SnapshotPackageCopyIndexResolver;
use super::NpmPackageId;
use super::NpmPackageReq;
use super::NpmResolutionPackage;
use super::NpmVersionMatcher;

/// A memory efficient path of visited name and versions in the graph
/// which is used to detect cycles.
///
/// note(dsherret): although this is definitely more memory efficient
/// than a HashSet, I haven't done any tests about whether this is
/// faster in practice.
#[derive(Default, Clone)]
struct VisitedVersionsPath {
  previous_node: Option<Arc<VisitedVersionsPath>>,
  visited_version_key: String,
}

impl VisitedVersionsPath {
  pub fn new(id: &NpmPackageId) -> Arc<Self> {
    Arc::new(Self {
      previous_node: None,
      visited_version_key: Self::id_to_key(id),
    })
  }

  pub fn with_parent(
    self: &Arc<VisitedVersionsPath>,
    parent: &NodeParent,
  ) -> Option<Arc<Self>> {
    match parent {
      NodeParent::Node(id) => self.with_id(id),
      NodeParent::Req => Some(self.clone()),
    }
  }

  pub fn with_id(
    self: &Arc<VisitedVersionsPath>,
    id: &NpmPackageId,
  ) -> Option<Arc<Self>> {
    if self.has_visited(id) {
      None
    } else {
      Some(Arc::new(Self {
        previous_node: Some(self.clone()),
        visited_version_key: Self::id_to_key(id),
      }))
    }
  }

  pub fn has_visited(self: &Arc<Self>, id: &NpmPackageId) -> bool {
    let mut maybe_next_node = Some(self);
    let key = Self::id_to_key(id);
    while let Some(next_node) = maybe_next_node {
      if next_node.visited_version_key == key {
        return true;
      }
      maybe_next_node = next_node.previous_node.as_ref();
    }
    false
  }

  fn id_to_key(id: &NpmPackageId) -> String {
    format!("{}@{}", id.name, id.version)
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
  /// These are top of the graph npm package requirements
  /// as specified in Deno code.
  Req,
  /// A reference to another node, which is a resolved package.
  Node(NpmPackageId),
}

/// A resolved package in the resolution graph.
#[derive(Debug)]
struct Node {
  pub id: NpmPackageId,
  /// If the node was forgotten due to having no parents.
  pub forgotten: bool,
  // Use BTreeMap and BTreeSet in order to create determinism
  // when going up and down the tree
  pub parents: BTreeMap<String, BTreeSet<NodeParent>>,
  pub children: BTreeMap<String, NpmPackageId>,
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
pub struct Graph {
  package_reqs: HashMap<String, NpmPackageId>,
  packages_by_name: HashMap<String, Vec<NpmPackageId>>,
  // Ideally this value would be Rc<RefCell<Node>>, but we need to use a Mutex
  // because the lsp requires Send and this code is executed in the lsp.
  // Would be nice if the lsp wasn't Send.
  packages: HashMap<NpmPackageId, Arc<Mutex<Node>>>,
  // This will be set when creating from a snapshot, then
  // inform the final snapshot creation.
  packages_to_copy_index: HashMap<NpmPackageId, usize>,
}

impl Graph {
  pub fn from_snapshot(snapshot: NpmResolutionSnapshot) -> Self {
    fn fill_for_id(
      graph: &mut Graph,
      id: &NpmPackageId,
      packages: &HashMap<NpmPackageId, NpmResolutionPackage>,
    ) -> Arc<Mutex<Node>> {
      let resolution = packages.get(id).unwrap();
      let (created, node) = graph.get_or_create_for_id(id);
      if created {
        for (name, child_id) in &resolution.dependencies {
          let child_node = fill_for_id(graph, child_id, packages);
          graph.set_child_parent_node(name, &child_node, id);
        }
      }
      node
    }

    let mut graph = Self {
      // Note: It might be more correct to store the copy index
      // from past resolutions with the node somehow, but maybe not.
      packages_to_copy_index: snapshot
        .packages
        .iter()
        .map(|(id, p)| (id.clone(), p.copy_index))
        .collect(),
      ..Default::default()
    };
    for (package_req, id) in &snapshot.package_reqs {
      let node = fill_for_id(&mut graph, id, &snapshot.packages);
      let package_req_text = package_req.to_string();
      (*node)
        .lock()
        .add_parent(package_req_text.clone(), NodeParent::Req);
      graph.package_reqs.insert(package_req_text, id.clone());
    }
    graph
  }

  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.package_reqs.contains_key(&req.to_string())
  }

  fn get_or_create_for_id(
    &mut self,
    id: &NpmPackageId,
  ) -> (bool, Arc<Mutex<Node>>) {
    if let Some(node) = self.packages.get(id) {
      (false, node.clone())
    } else {
      let node = Arc::new(Mutex::new(Node {
        id: id.clone(),
        forgotten: false,
        parents: Default::default(),
        children: Default::default(),
        deps: Default::default(),
        no_peers: false,
      }));
      self
        .packages_by_name
        .entry(id.name.clone())
        .or_default()
        .push(id.clone());
      self.packages.insert(id.clone(), node.clone());
      (true, node)
    }
  }

  fn borrow_node(&self, id: &NpmPackageId) -> MutexGuard<Node> {
    (**self.packages.get(id).unwrap_or_else(|| {
      panic!("could not find id {} in the tree", id.as_serialized())
    }))
    .lock()
  }

  fn forget_orphan(&mut self, node_id: &NpmPackageId) {
    if let Some(node) = self.packages.remove(node_id) {
      let mut node = (*node).lock();
      node.forgotten = true;
      assert_eq!(node.parents.len(), 0);

      // Remove the id from the list of packages by name.
      let packages_with_name =
        self.packages_by_name.get_mut(&node.id.name).unwrap();
      let remove_index = packages_with_name
        .iter()
        .position(|id| id == &node.id)
        .unwrap();
      packages_with_name.remove(remove_index);

      let parent = NodeParent::Node(node.id.clone());
      for (specifier, child_id) in &node.children {
        let mut child = self.borrow_node(child_id);
        child.remove_parent(specifier, &parent);
        if child.parents.is_empty() {
          drop(child); // stop borrowing from self
          self.forget_orphan(child_id);
        }
      }
    }
  }

  fn set_child_parent(
    &mut self,
    specifier: &str,
    child: &Mutex<Node>,
    parent: &NodeParent,
  ) {
    match parent {
      NodeParent::Node(parent_id) => {
        self.set_child_parent_node(specifier, child, parent_id);
      }
      NodeParent::Req => {
        let mut node = (*child).lock();
        node.add_parent(specifier.to_string(), parent.clone());
        self
          .package_reqs
          .insert(specifier.to_string(), node.id.clone());
      }
    }
  }

  fn set_child_parent_node(
    &mut self,
    specifier: &str,
    child: &Mutex<Node>,
    parent_id: &NpmPackageId,
  ) {
    let mut child = (*child).lock();
    let mut parent = (**self.packages.get(parent_id).unwrap_or_else(|| {
      panic!(
        "could not find {} in list of packages when setting child {}",
        parent_id.as_serialized(),
        child.id.as_serialized()
      )
    }))
    .lock();
    assert_ne!(parent.id, child.id);
    parent
      .children
      .insert(specifier.to_string(), child.id.clone());
    child
      .add_parent(specifier.to_string(), NodeParent::Node(parent.id.clone()));
  }

  fn remove_child_parent(
    &mut self,
    specifier: &str,
    child_id: &NpmPackageId,
    parent: &NodeParent,
  ) {
    match parent {
      NodeParent::Node(parent_id) => {
        let mut node = self.borrow_node(parent_id);
        if let Some(removed_child_id) = node.children.remove(specifier) {
          assert_eq!(removed_child_id, *child_id);
        }
      }
      NodeParent::Req => {
        if let Some(removed_child_id) = self.package_reqs.remove(specifier) {
          assert_eq!(removed_child_id, *child_id);
        }
      }
    }
    self.borrow_node(child_id).remove_parent(specifier, parent);
  }

  pub async fn into_snapshot(
    self,
    api: &impl NpmRegistryApi,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::from_map_with_capacity(
        self.packages_to_copy_index,
        self.packages.len(),
      );

    // Iterate through the packages vector in each packages_by_name in order
    // to set the copy index as this will be deterministic rather than
    // iterating over the hashmap below.
    for packages in self.packages_by_name.values() {
      if packages.len() > 1 {
        for id in packages {
          copy_index_resolver.resolve(id);
        }
      }
    }

    let mut packages = HashMap::with_capacity(self.packages.len());
    for (id, node) in self.packages {
      let dist = api
        .package_version_info(&id.name, &id.version)
        .await?
        .unwrap()
        .dist;
      let node = node.lock();
      packages.insert(
        id.clone(),
        NpmResolutionPackage {
          copy_index: copy_index_resolver.resolve(&id),
          id,
          dist,
          dependencies: node
            .children
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        },
      );
    }

    Ok(NpmResolutionSnapshot {
      package_reqs: self
        .package_reqs
        .into_iter()
        .map(|(specifier, id)| {
          (NpmPackageReq::from_str(&specifier).unwrap(), id)
        })
        .collect(),
      packages_by_name: self.packages_by_name,
      packages,
    })
  }
}

pub struct GraphDependencyResolver<'a, TNpmRegistryApi: NpmRegistryApi> {
  graph: &'a mut Graph,
  api: &'a TNpmRegistryApi,
  pending_unresolved_nodes:
    VecDeque<(Arc<VisitedVersionsPath>, Arc<Mutex<Node>>)>,
}

impl<'a, TNpmRegistryApi: NpmRegistryApi>
  GraphDependencyResolver<'a, TNpmRegistryApi>
{
  pub fn new(graph: &'a mut Graph, api: &'a TNpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_unresolved_nodes: Default::default(),
    }
  }

  fn resolve_best_package_version_and_info<'info>(
    &self,
    version_matcher: &impl NpmVersionMatcher,
    package_info: &'info NpmPackageInfo,
  ) -> Result<VersionAndInfo<'info>, AnyError> {
    if let Some(version) =
      self.resolve_best_package_version(package_info, version_matcher)?
    {
      match package_info.versions.get(&version.to_string()) {
        Some(version_info) => Ok(VersionAndInfo {
          version,
          info: version_info,
        }),
        None => {
          bail!(
            "could not find version '{}' for '{}'",
            version,
            &package_info.name
          )
        }
      }
    } else {
      // get the information
      get_resolved_package_version_and_info(version_matcher, package_info, None)
    }
  }

  fn resolve_best_package_version(
    &self,
    package_info: &NpmPackageInfo,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Result<Option<NpmVersion>, AnyError> {
    let mut maybe_best_version: Option<&NpmVersion> = None;
    if let Some(ids) = self.graph.packages_by_name.get(&package_info.name) {
      for version in ids.iter().map(|id| &id.version) {
        if version_req_satisfies(version_matcher, version, package_info, None)?
        {
          let is_best_version = maybe_best_version
            .as_ref()
            .map(|best_version| (*best_version).cmp(version).is_lt())
            .unwrap_or(true);
          if is_best_version {
            maybe_best_version = Some(version);
          }
        }
      }
    }
    Ok(maybe_best_version.cloned())
  }

  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.graph.has_package_req(req)
  }

  pub fn add_package_req(
    &mut self,
    package_req: &NpmPackageReq,
    package_info: &NpmPackageInfo,
  ) -> Result<(), AnyError> {
    let node = self.resolve_node_from_info(
      &package_req.name,
      package_req,
      package_info,
      None,
    )?;
    self.graph.set_child_parent(
      &package_req.to_string(),
      &node,
      &NodeParent::Req,
    );
    self.try_add_pending_unresolved_node(None, &node);
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: &NpmPackageInfo,
    parent_id: &NpmPackageId,
    visited_versions: &Arc<VisitedVersionsPath>,
  ) -> Result<Arc<Mutex<Node>>, AnyError> {
    let node = self.resolve_node_from_info(
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
    self.graph.set_child_parent(
      &entry.bare_specifier,
      &node,
      &NodeParent::Node(parent_id.clone()),
    );
    self.try_add_pending_unresolved_node(Some(visited_versions), &node);
    Ok(node)
  }

  fn try_add_pending_unresolved_node(
    &mut self,
    maybe_previous_visited_versions: Option<&Arc<VisitedVersionsPath>>,
    node: &Arc<Mutex<Node>>,
  ) {
    let node_id = {
      let node = node.lock();
      if node.no_peers {
        return; // skip, no need to analyze this again
      }
      node.id.clone()
    };
    let visited_versions = match maybe_previous_visited_versions {
      Some(previous_visited_versions) => {
        match previous_visited_versions.with_id(&node_id) {
          Some(visited_versions) => visited_versions,
          None => return, // circular, don't visit this node
        }
      }
      None => VisitedVersionsPath::new(&node_id),
    };
    self
      .pending_unresolved_nodes
      .push_back((visited_versions, node.clone()));
  }

  fn resolve_node_from_info(
    &mut self,
    pkg_req_name: &str,
    version_matcher: &impl NpmVersionMatcher,
    package_info: &NpmPackageInfo,
    parent_id: Option<&NpmPackageId>,
  ) -> Result<Arc<Mutex<Node>>, AnyError> {
    let version_and_info = self
      .resolve_best_package_version_and_info(version_matcher, package_info)?;
    let id = NpmPackageId {
      name: package_info.name.to_string(),
      version: version_and_info.version.clone(),
      peer_dependencies: Vec::new(),
    };
    debug!(
      "{} - Resolved {}@{} to {}",
      match parent_id {
        Some(id) => id.as_serialized(),
        None => "<package-req>".to_string(),
      },
      pkg_req_name,
      version_matcher.version_text(),
      id.as_serialized(),
    );
    let (created, node) = self.graph.get_or_create_for_id(&id);
    if created {
      let mut node = (*node).lock();
      let mut deps = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| format!("npm package: {}", id.display()))?;
      // Ensure name alphabetical and then version descending
      // so these are resolved in that order
      deps.sort();
      node.deps = Arc::new(deps);
      node.no_peers = node.deps.is_empty();
    }

    Ok(node)
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some((visited_versions, parent_node)) =
        self.pending_unresolved_nodes.pop_front()
      {
        let (mut parent_id, deps, existing_children) = {
          let parent_node = parent_node.lock();
          if parent_node.forgotten || parent_node.no_peers {
            // todo(dsherret): we should try to reproduce this forgotten scenario and write a test
            continue;
          }

          (
            parent_node.id.clone(),
            parent_node.deps.clone(),
            parent_node.children.clone(),
          )
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
              let node = self.analyze_dependency(
                dep,
                &package_info,
                &parent_id,
                &visited_versions,
              )?;
              if !found_peer {
                found_peer = !node.lock().no_peers;
              }
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              found_peer = true;
              let maybe_new_parent_id = self.resolve_peer_dep(
                &dep.bare_specifier,
                &parent_id,
                dep,
                &package_info,
                &visited_versions,
                existing_children.get(&dep.bare_specifier),
              )?;
              if let Some(new_parent_id) = maybe_new_parent_id {
                assert_eq!(
                  (&new_parent_id.name, &new_parent_id.version),
                  (&parent_id.name, &parent_id.version)
                );
                parent_id = new_parent_id;
              }
            }
          }
        }

        if !found_peer {
          self.graph.borrow_node(&parent_id).no_peers = true;
        }
      }
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    specifier: &str,
    parent_id: &NpmPackageId,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: &NpmPackageInfo,
    visited_ancestor_versions: &Arc<VisitedVersionsPath>,
    existing_dep_id: Option<&NpmPackageId>,
  ) -> Result<Option<NpmPackageId>, AnyError> {
    fn find_matching_child<'a>(
      peer_dep: &NpmDependencyEntry,
      peer_package_info: &NpmPackageInfo,
      children: impl Iterator<Item = &'a NpmPackageId>,
    ) -> Result<Option<NpmPackageId>, AnyError> {
      for child_id in children {
        if child_id.name == peer_dep.name
          && version_req_satisfies(
            &peer_dep.version_req,
            &child_id.version,
            peer_package_info,
            None,
          )?
        {
          return Ok(Some(child_id.clone()));
        }
      }
      Ok(None)
    }

    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut pending_ancestors = VecDeque::new(); // go up the tree by depth
    let path = GraphSpecifierPath::new(specifier.to_string());
    let visited_versions = VisitedVersionsPath::new(parent_id);

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
          let maybe_peer_dep_id = if ancestor_node_id.name == peer_dep.name
            && version_req_satisfies(
              &peer_dep.version_req,
              &ancestor_node_id.version,
              peer_package_info,
              None,
            )? {
            Some(ancestor_node_id.clone())
          } else {
            let ancestor = self.graph.borrow_node(ancestor_node_id);
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
              ancestor.children.values(),
            )?
          };
          if let Some(peer_dep_id) = maybe_peer_dep_id {
            if existing_dep_id == Some(&peer_dep_id) {
              return Ok(None); // do nothing, there's already an existing child dep id for this
            }

            // handle optional dependency that's never been set
            if existing_dep_id.is_none() && peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                &peer_dep_id,
                parent_id,
                peer_dep,
                visited_ancestor_versions,
              );
              return Ok(None);
            }

            let parents =
              self.graph.borrow_node(ancestor_node_id).parents.clone();
            return Ok(Some(self.set_new_peer_dep(
              parents,
              ancestor_node_id,
              &peer_dep_id,
              &path,
              visited_ancestor_versions,
            )));
          }
        }
        NodeParent::Req => {
          // in this case, the parent is the root so the children are all the package requirements
          if let Some(child_id) = find_matching_child(
            peer_dep,
            peer_package_info,
            self.graph.package_reqs.values(),
          )? {
            if existing_dep_id == Some(&child_id) {
              return Ok(None); // do nothing, there's already an existing child dep id for this
            }

            // handle optional dependency that's never been set
            if existing_dep_id.is_none() && peer_dep.kind.is_optional() {
              self.set_previously_unresolved_optional_dependency(
                &child_id,
                parent_id,
                peer_dep,
                visited_ancestor_versions,
              );
              return Ok(None);
            }

            let specifier = path.specifier.to_string();
            let path = path.pop().unwrap(); // go back down one level from the package requirement
            let old_id =
              self.graph.package_reqs.get(&specifier).unwrap().clone();
            return Ok(Some(self.set_new_peer_dep(
              BTreeMap::from([(specifier, BTreeSet::from([NodeParent::Req]))]),
              &old_id,
              &child_id,
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
    peer_dep_id: &NpmPackageId,
    parent_id: &NpmPackageId,
    peer_dep: &NpmDependencyEntry,
    visited_ancestor_versions: &Arc<VisitedVersionsPath>,
  ) {
    let (_, node) = self.graph.get_or_create_for_id(peer_dep_id);
    self.graph.set_child_parent(
      &peer_dep.bare_specifier,
      &node,
      &NodeParent::Node(parent_id.clone()),
    );
    self
      .try_add_pending_unresolved_node(Some(visited_ancestor_versions), &node);
  }

  fn set_new_peer_dep(
    &mut self,
    previous_parents: BTreeMap<String, BTreeSet<NodeParent>>,
    node_id: &NpmPackageId,
    peer_dep_id: &NpmPackageId,
    path: &Arc<GraphSpecifierPath>,
    visited_ancestor_versions: &Arc<VisitedVersionsPath>,
  ) -> NpmPackageId {
    let peer_dep_id = Cow::Borrowed(peer_dep_id);
    let old_id = node_id;
    let (new_id, mut old_node_children) =
      if old_id.peer_dependencies.contains(&peer_dep_id)
        || *old_id == *peer_dep_id
      {
        // the parent has already resolved to using this peer dependency
        // via some other path or the parent is the peer dependency,
        // so we don't need to update its ids, but instead only make a link to it
        (
          old_id.clone(),
          self.graph.borrow_node(old_id).children.clone(),
        )
      } else {
        let mut new_id = old_id.clone();
        new_id.peer_dependencies.push(peer_dep_id.as_ref().clone());

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

        let (_, new_node) = self.graph.get_or_create_for_id(&new_id);

        // update the previous parent to point to the new node
        // and this node to point at those parents
        for (specifier, parents) in previous_parents {
          for parent in parents {
            self.graph.set_child_parent(&specifier, &new_node, &parent);
          }
        }

        // now add the previous children to this node
        let new_id_as_parent = NodeParent::Node(new_id.clone());
        for (specifier, child_id) in &old_node_children {
          let child = self.graph.packages.get(child_id).unwrap().clone();
          self
            .graph
            .set_child_parent(specifier, &child, &new_id_as_parent);
        }
        (new_id, old_node_children)
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
        next_node_id,
        &peer_dep_id,
        path,
        visited_ancestor_versions,
      );
    } else {
      // this means we're at the peer dependency now
      debug!(
        "Resolved peer dependency for {} in {} to {}",
        next_specifier,
        &new_id.as_serialized(),
        &peer_dep_id.as_serialized(),
      );

      // handle this node having a previous child due to another peer dependency
      if let Some(child_id) = old_node_children.remove(next_specifier) {
        if let Some(node) = self.graph.packages.get(&child_id) {
          let is_orphan = {
            let mut node = node.lock();
            node
              .remove_parent(next_specifier, &NodeParent::Node(new_id.clone()));
            node.parents.is_empty()
          };
          if is_orphan {
            self.graph.forget_orphan(&child_id);
          }
        }
      }

      let node = self.graph.get_or_create_for_id(&peer_dep_id).1;
      self.try_add_pending_unresolved_node(
        Some(visited_ancestor_versions),
        &node,
      );
      self
        .graph
        .set_child_parent_node(next_specifier, &node, &new_id);
    }

    // forget the old node at this point if it has no parents
    if new_id != *old_id {
      let old_node = self.graph.borrow_node(old_id);
      if old_node.parents.is_empty() {
        drop(old_node); // stop borrowing
        self.graph.forget_orphan(old_id);
      }
    }

    bottom_parent_id
  }
}

#[derive(Clone)]
struct VersionAndInfo<'a> {
  version: NpmVersion,
  info: &'a NpmPackageVersionInfo,
}

fn get_resolved_package_version_and_info<'a>(
  version_matcher: &impl NpmVersionMatcher,
  info: &'a NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo<'a>, AnyError> {
  if let Some(tag) = version_matcher.tag() {
    tag_to_version_info(info, tag, parent)
  } else {
    let mut maybe_best_version: Option<VersionAndInfo> = None;
    for version_info in info.versions.values() {
      let version = NpmVersion::parse(&version_info.version)?;
      if version_matcher.matches(&version) {
        let is_best_version = maybe_best_version
          .as_ref()
          .map(|best_version| best_version.version.cmp(&version).is_lt())
          .unwrap_or(true);
        if is_best_version {
          maybe_best_version = Some(VersionAndInfo {
            version,
            info: version_info,
          });
        }
      }
    }

    match maybe_best_version {
      Some(v) => Ok(v),
      // If the package isn't found, it likely means that the user needs to use
      // `--reload` to get the latest npm package information. Although it seems
      // like we could make this smart by fetching the latest information for
      // this package here, we really need a full restart. There could be very
      // interesting bugs that occur if this package's version was resolved by
      // something previous using the old information, then now being smart here
      // causes a new fetch of the package information, meaning this time the
      // previous resolution of this package's version resolved to an older
      // version, but next time to a different version because it has new information.
      None => bail!(
        concat!(
          "Could not find npm package '{}' matching {}{}. ",
          "Try retrieving the latest npm package information by running with --reload",
        ),
        info.name,
        version_matcher.version_text(),
        match parent {
          Some(id) => format!(" as specified in {}", id.display()),
          None => String::new(),
        }
      ),
    }
  }
}

fn version_req_satisfies(
  matcher: &impl NpmVersionMatcher,
  version: &NpmVersion,
  package_info: &NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<bool, AnyError> {
  match matcher.tag() {
    Some(tag) => {
      let tag_version = tag_to_version_info(package_info, tag, parent)?.version;
      Ok(tag_version == *version)
    }
    None => Ok(matcher.matches(version)),
  }
}

fn tag_to_version_info<'a>(
  info: &'a NpmPackageInfo,
  tag: &str,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo<'a>, AnyError> {
  // For when someone just specifies @types/node, we want to pull in a
  // "known good" version of @types/node that works well with Deno and
  // not necessarily the latest version. For example, we might only be
  // compatible with Node vX, but then Node vY is published so we wouldn't
  // want to pull that in.
  // Note: If the user doesn't want this behavior, then they can specify an
  // explicit version.
  if tag == "latest" && info.name == "@types/node" {
    return get_resolved_package_version_and_info(
      &NpmVersionReq::parse("18.0.0 - 18.8.2").unwrap(),
      info,
      parent,
    );
  }

  if let Some(version) = info.dist_tags.get(tag) {
    match info.versions.get(version) {
      Some(info) => Ok(VersionAndInfo {
        version: NpmVersion::parse(version)?,
        info,
      }),
      None => {
        bail!(
          "Could not find version '{}' referenced in dist-tag '{}'.",
          version,
          tag,
        )
      }
    }
  } else {
    bail!("Could not find dist-tag '{}'.", tag)
  }
}

#[cfg(test)]
mod test {
  use pretty_assertions::assert_eq;

  use crate::npm::registry::TestNpmRegistryApi;
  use crate::npm::NpmPackageReference;

  use super::*;

  #[test]
  fn test_get_resolved_package_version_and_info() {
    // dist tag where version doesn't exist
    let package_ref = NpmPackageReference::from_str("npm:test").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".to_string(),
      versions: HashMap::new(),
      dist_tags: HashMap::from([(
        "latest".to_string(),
        "1.0.0-alpha".to_string(),
      )]),
    };
    let result = get_resolved_package_version_and_info(
      &package_ref.req,
      &package_info,
      None,
    );
    assert_eq!(
      result.err().unwrap().to_string(),
      "Could not find version '1.0.0-alpha' referenced in dist-tag 'latest'."
    );

    // dist tag where version is a pre-release
    let package_ref = NpmPackageReference::from_str("npm:test").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".to_string(),
      versions: HashMap::from([
        ("0.1.0".to_string(), NpmPackageVersionInfo::default()),
        (
          "1.0.0-alpha".to_string(),
          NpmPackageVersionInfo {
            version: "0.1.0-alpha".to_string(),
            ..Default::default()
          },
        ),
      ]),
      dist_tags: HashMap::from([(
        "latest".to_string(),
        "1.0.0-alpha".to_string(),
      )]),
    };
    let result = get_resolved_package_version_and_info(
      &package_ref.req,
      &package_info,
      None,
    );
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-alpha");
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
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@0.1.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageId::from_serialized("package-d@3.2.1").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-d@3.2.1").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
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
      vec!["npm:package-a@1", "npm:package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
      run_resolver_and_get_output(api, vec!["npm:package-0@1.1.1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-0@1.1.1").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized("package-a@1.0.0_package-peer@4.0.0")
              .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@3.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@4.1.0").unwrap(),
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
      run_resolver_and_get_output(api, vec!["npm:package-a@1"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@3.0.0").unwrap(),
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
      vec!["npm:package-a@1", "npm:package-peer@4.0.0"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@3.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized("package-b@1.0.0").unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-b@1.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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

    let (packages, package_reqs) = run_resolver_and_get_output(
      api,
      vec!["npm:package-a@1", "npm:package-b@1"],
    )
    .await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-b@1.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-a".to_string(),
              NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
            )
          ]),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@2.0.0").unwrap(),
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
      run_resolver_and_get_output(api, vec!["npm:package-0@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-0@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::from_serialized("package-peer-a@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer-a@2.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::from_serialized("package-peer-b@3.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer-b@3.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized(
            "package-0@1.0.0_package-peer-a@2.0.0_package-peer-b@3.0.0"
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
              NpmPackageId::from_serialized("package-peer-b@3.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized("package-peer-b@3.0.0").unwrap(),
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
          id: NpmPackageId::from_serialized("package-0@1.1.1").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::from_serialized(
              "package-a@1.0.0_package-peer-a@4.0.0"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
            "package-a@1.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::from_serialized(
                "package-b@2.0.0_package-peer-a@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@3.0.0_package-peer-a@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageId::from_serialized("package-d@3.5.0").unwrap(),
            ),
            (
              "package-peer-a".to_string(),
              NpmPackageId::from_serialized("package-peer-a@4.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
            "package-b@2.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageId::from_serialized("package-peer-a@4.0.0").unwrap(),
            ),
            (
              "package-peer-c".to_string(),
              NpmPackageId::from_serialized("package-peer-c@6.2.0").unwrap(),
            )
          ])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
            "package-c@3.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::from_serialized("package-peer-a@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-d@3.5.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-e@3.6.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer-a@4.0.0").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::from_serialized("package-peer-b@5.4.1").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer-b@5.4.1").unwrap(),
          copy_index: 0,
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer-c@6.2.0").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@2.0.0_package-a@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-b@2.0.0_package-a@1.0.0")
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

      let (packages, package_reqs) = run_resolver_and_get_output(
        api,
        vec!["npm:package-a@1", "npm:package-b@2"],
      )
      .await;
      assert_eq!(
        packages,
        vec![
          NpmResolutionPackage {
            id: NpmPackageId::from_serialized(
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
            id: NpmPackageId::from_serialized(
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
            id: NpmPackageId::from_serialized(
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
            id: NpmPackageId::from_serialized(
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
            id: NpmPackageId::from_serialized("package-peer@4.0.0").unwrap(),
            copy_index: 0,
            dependencies: HashMap::new(),
            dist: Default::default(),
          },
          NpmResolutionPackage {
            id: NpmPackageId::from_serialized("package-peer@5.0.0").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0_package-b@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized("package-c@1.0.0_package-b@1.0.0")
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
          id: NpmPackageId::from_serialized("package-b@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@1.0.0_package-b@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::from_serialized("package-b@1.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@1.0.0").unwrap(),
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
    let api = TestNpmRegistryApi::default();
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
          id: NpmPackageId::from_serialized("package-a@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::from_serialized("package-peer@1.2.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized(
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
          id: NpmPackageId::from_serialized(
            "package-b@1.0.0_package-a@1.0.0_package-peer@1.1.0"
          )
          .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([
            (
              "package-c".to_string(),
              NpmPackageId::from_serialized(
                "package-c@1.0.0_package-a@1.0.0_package-peer@1.1.0"
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
          id: NpmPackageId::from_serialized(
            "package-c@1.0.0_package-a@1.0.0_package-peer@1.1.0"
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
          id: NpmPackageId::from_serialized("package-peer@1.1.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-peer@1.2.0").unwrap(),
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
      run_resolver_and_get_output(api, vec!["npm:package-a@1.0"]).await;
    assert_eq!(
      packages,
      vec![
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-a@1.0.0_package-d@1.0.0")
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
          id: NpmPackageId::from_serialized("package-b@2.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-c@1.0.0_package-d@1.0.0")
            .unwrap(),
          copy_index: 0,
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageId::from_serialized("package-d@1.0.0").unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-d@1.0.0").unwrap(),
          copy_index: 0,
          dependencies: HashMap::new(),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::from_serialized("package-e@1.0.0").unwrap(),
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

  async fn run_resolver_and_get_output(
    api: TestNpmRegistryApi,
    reqs: Vec<&str>,
  ) -> (Vec<NpmResolutionPackage>, Vec<(String, String)>) {
    let mut graph = Graph::default();
    let mut resolver = GraphDependencyResolver::new(&mut graph, &api);

    for req in reqs {
      let req = NpmPackageReference::from_str(req).unwrap().req;
      resolver
        .add_package_req(&req, &api.package_info(&req.name).await.unwrap())
        .unwrap();
    }

    resolver.resolve_pending().await.unwrap();
    let snapshot = graph.into_snapshot(&api).await.unwrap();
    let mut packages = snapshot.all_packages();
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut package_reqs = snapshot
      .package_reqs
      .into_iter()
      .map(|(a, b)| (a.to_string(), b.as_serialized()))
      .collect::<Vec<_>>();
    package_reqs.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    (packages, package_reqs)
  }
}
