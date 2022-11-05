// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::MutexGuard;

use crate::npm::cache::should_sync_download;
use crate::npm::registry::NpmDependencyEntry;
use crate::npm::registry::NpmDependencyEntryKind;
use crate::npm::registry::NpmPackageInfo;
use crate::npm::registry::NpmPackageVersionInfo;
use crate::npm::semver::NpmVersion;
use crate::npm::semver::NpmVersionReq;
use crate::npm::NpmRegistryApi;

use super::snapshot::NpmResolutionSnapshot;
use super::NpmPackageId;
use super::NpmPackageReq;
use super::NpmResolutionPackage;
use super::NpmVersionMatcher;

#[derive(Default, Clone)]
pub struct VisitedVersions(HashSet<String>);

impl VisitedVersions {
  pub fn add(&mut self, id: &NpmPackageId) -> bool {
    self.0.insert(Self::id_as_key(id))
  }

  pub fn has_visited(&self, id: &NpmPackageId) -> bool {
    self.0.contains(&Self::id_as_key(id))
  }

  fn id_as_key(id: &NpmPackageId) -> String {
    // we only key on name and version in the id and not peer dependencies
    // because the peer dependencies could change above and below us,
    // but the names and versions won't
    format!("{}@{}", id.name, id.version)
  }
}

#[derive(Default, Clone)]
pub struct GraphPath {
  visited_versions: VisitedVersions,
  specifiers: Vec<String>,
}

impl GraphPath {
  pub fn with_step(&self, specifier: &str, id: &NpmPackageId) -> GraphPath {
    let mut copy = self.clone();
    assert!(copy.visited_versions.add(id));
    copy.specifiers.push(specifier.to_string());
    copy
  }

  pub fn with_specifier(&self, specifier: String) -> GraphPath {
    let mut copy = self.clone();
    copy.specifiers.push(specifier);
    copy
  }

  pub fn has_visited_version(&self, id: &NpmPackageId) -> bool {
    self.visited_versions.has_visited(id)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NodeParent {
  /// These are top of the graph npm package requirements
  /// as specified in Deno code.
  Req(NpmPackageReq),
  /// A reference to another node, which is a resolved package.
  Node(NpmPackageId),
}

/// A resolved package in the resolution graph.
#[derive(Debug)]
struct Node {
  pub id: NpmPackageId,
  pub parents: HashMap<String, HashSet<NodeParent>>,
  pub children: HashMap<String, NpmPackageId>,
  pub deps: Arc<Vec<NpmDependencyEntry>>,
}

impl Node {
  pub fn add_parent(&mut self, specifier: String, parent: NodeParent) {
    eprintln!(
      "ADDING parent to {}: {} {} {}",
      self.id.as_serializable_name(),
      specifier,
      match &parent {
        NodeParent::Node(n) => n.as_serializable_name(),
        NodeParent::Req(req) => req.to_string(),
      },
      self.parents.entry(specifier.clone()).or_default().len(),
    );
    if self.id.as_serializable_name() == "package-a@1.0.0"
      && match &parent {
        NodeParent::Node(n) => n.as_serializable_name(),
        NodeParent::Req(req) => req.to_string(),
      } == "package-b@2.0.0_package-a@1.0.0"
    {
      panic!("STOP");
    }
    self.parents.entry(specifier).or_default().insert(parent);
  }

  pub fn remove_parent(&mut self, specifier: &str, parent: &NodeParent) {
    eprintln!(
      "REMOVING parent from {}: {} {} {}",
      self.id.as_serializable_name(),
      specifier,
      match parent {
        NodeParent::Node(n) => n.as_serializable_name(),
        NodeParent::Req(req) => req.to_string(),
      },
      self.parents.entry(specifier.to_string()).or_default().len(),
    );
    if let Some(parents) = self.parents.get_mut(specifier) {
      parents.remove(parent);
      if parents.is_empty() {
        drop(parents);
        self.parents.remove(specifier);
      }
    }
  }
}

#[derive(Debug, Default)]
pub struct Graph {
  package_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  packages_by_name: HashMap<String, Vec<NpmPackageId>>,
  // Ideally this would be Rc<RefCell<Node>>, but we need to use a Mutex
  // because the lsp requires Send and this code is executed in the lsp.
  // Would be nice if the lsp wasn't Send.
  packages: HashMap<NpmPackageId, Arc<Mutex<Node>>>,
}

impl Graph {
  pub fn has_package_req(&self, req: &NpmPackageReq) -> bool {
    self.package_reqs.contains_key(req)
  }

  pub fn fill_with_snapshot(&mut self, snapshot: &NpmResolutionSnapshot) {
    for (package_req, id) in &snapshot.package_reqs {
      let node = self.fill_for_id_with_snapshot(id, snapshot);
      (*node).lock().add_parent(
        package_req.to_string(),
        NodeParent::Req(package_req.clone()),
      );
      self.package_reqs.insert(package_req.clone(), id.clone());
    }
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
        parents: Default::default(),
        children: Default::default(),
        deps: Default::default(),
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

  fn fill_for_id_with_snapshot(
    &mut self,
    id: &NpmPackageId,
    snapshot: &NpmResolutionSnapshot,
  ) -> Arc<Mutex<Node>> {
    let resolution = snapshot.packages.get(id).unwrap();
    let node = self.get_or_create_for_id(id).1;
    for (name, child_id) in &resolution.dependencies {
      let child_node = self.fill_for_id_with_snapshot(&child_id, snapshot);
      self.set_child_parent_node(&name, &child_node, &id);
    }
    node
  }

  fn borrow_node(&self, id: &NpmPackageId) -> MutexGuard<Node> {
    (**self.packages.get(id).unwrap_or_else(|| {
      panic!(
        "could not find id {} in the tree",
        id.as_serializable_name()
      )
    }))
    .lock()
  }

  fn forget_orphan(&mut self, node_id: &NpmPackageId) {
    if let Some(node) = self.packages.remove(node_id) {
      eprintln!(
        "REMAINING: {:?}",
        self
          .packages
          .values()
          .map(|n| n.lock().id.as_serializable_name())
          .collect::<Vec<_>>()
      );
      let node = (*node).lock();
      eprintln!("FORGOT: {}", node.id.as_serializable_name());
      assert_eq!(node.parents.len(), 0);
      let parent = NodeParent::Node(node.id.clone());
      for (specifier, child_id) in &node.children {
        let mut child = self.borrow_node(child_id);
        child.remove_parent(specifier, &parent);
        eprintln!("CHILD PARENTS: {:?}", child.parents);
        if child.parents.is_empty() {
          drop(child); // stop borrowing from self
          self.forget_orphan(&child_id);
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
        self.set_child_parent_node(&specifier, &child, &parent_id);
      }
      NodeParent::Req(package_req) => {
        let mut node = (*child).lock();
        node.add_parent(specifier.to_string(), parent.clone());
        self
          .package_reqs
          .insert(package_req.clone(), node.id.clone());
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
    let mut parent = (**self.packages.get(parent_id).unwrap()).lock();
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
        eprintln!("PARENT: {}", parent_id.as_serializable_name());
        eprintln!("SPECIFIER: {}", specifier);
        let mut node = self.borrow_node(parent_id);
        if let Some(removed_child_id) = node.children.remove(specifier) {
          assert_eq!(removed_child_id, *child_id);
        }
      }
      NodeParent::Req(req) => {
        assert_eq!(req.to_string(), specifier);
        if let Some(removed_child_id) = self.package_reqs.remove(req) {
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
    let mut packages = HashMap::with_capacity(self.packages.len());
    for (id, node) in self.packages {
      let dist = api
        .package_version_info(&id.name, &id.version)
        .await?
        .unwrap() // todo(THIS PR): don't unwrap here
        .dist;
      let node = node.lock();
      packages.insert(
        id.clone(),
        NpmResolutionPackage {
          dist,
          dependencies: node.children.clone(),
          id,
        },
      );
    }
    Ok(NpmResolutionSnapshot {
      package_reqs: self.package_reqs,
      packages_by_name: self.packages_by_name,
      packages,
    })
  }
}

pub struct GraphDependencyResolver<'a, TNpmRegistryApi: NpmRegistryApi> {
  graph: &'a mut Graph,
  api: &'a TNpmRegistryApi,
  pending_unresolved_nodes: VecDeque<(VisitedVersions, Arc<Mutex<Node>>)>,
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

  fn resolve_best_package_version_and_info(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
    package_info: NpmPackageInfo,
  ) -> Result<VersionAndInfo, AnyError> {
    if let Some(version) =
      self.resolve_best_package_version(name, version_matcher)
    {
      match package_info.versions.get(&version.to_string()) {
        Some(version_info) => Ok(VersionAndInfo {
          version,
          info: version_info.clone(),
        }),
        None => {
          bail!("could not find version '{}' for '{}'", version, name)
        }
      }
    } else {
      // get the information
      get_resolved_package_version_and_info(
        name,
        version_matcher,
        package_info,
        None,
      )
    }
  }

  fn resolve_best_package_version(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Option<NpmVersion> {
    let mut maybe_best_version: Option<&NpmVersion> = None;
    if let Some(ids) = self.graph.packages_by_name.get(name) {
      for version in ids.iter().map(|id| &id.version) {
        if version_matcher.matches(version) {
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
    maybe_best_version.cloned()
  }

  pub fn add_npm_package_req(
    &mut self,
    package_req: &NpmPackageReq,
    package_info: NpmPackageInfo,
  ) -> Result<(), AnyError> {
    let node = self.resolve_node_from_info(
      &package_req.name,
      package_req,
      package_info,
    )?;
    self.graph.set_child_parent(
      &package_req.to_string(),
      &node,
      &NodeParent::Req(package_req.clone()),
    );
    self
      .pending_unresolved_nodes
      .push_back((VisitedVersions::default(), node));
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: NpmPackageInfo,
    parent_id: &NpmPackageId,
    visited_versions: &VisitedVersions,
  ) -> Result<(), AnyError> {
    let node = self.resolve_node_from_info(
      &entry.name,
      match entry.kind {
        NpmDependencyEntryKind::Dep => &entry.version_req,
        // when resolving a peer dependency as a dependency, it should
        // use the "dependencies" entry version requirement if it exists
        NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer => {
          &entry
            .peer_dep_version_req
            .as_ref()
            .unwrap_or(&entry.version_req)
        }
      },
      package_info,
    )?;
    self.graph.set_child_parent(
      &entry.bare_specifier,
      &node,
      &NodeParent::Node(parent_id.clone()),
    );
    self
      .pending_unresolved_nodes
      .push_back((visited_versions.clone(), node));
    Ok(())
  }

  fn resolve_node_from_info(
    &mut self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
    package_info: NpmPackageInfo,
  ) -> Result<Arc<Mutex<Node>>, AnyError> {
    let version_and_info = self.resolve_best_package_version_and_info(
      name,
      version_matcher,
      package_info,
    )?;
    let id = NpmPackageId {
      name: name.to_string(),
      version: version_and_info.version.clone(),
      peer_dependencies: Vec::new(),
    };
    let (created, node) = self.graph.get_or_create_for_id(&id);
    if created {
      eprintln!("RESOLVED: {}", id.as_serializable_name());
      let mut node = (*node).lock();
      let mut deps = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| format!("npm package: {}", id))?;
      // Ensure name alphabetical and then version descending
      // so these are resolved in that order
      deps.sort();
      node.deps = Arc::new(deps);
    }

    Ok(node)
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_unresolved_nodes.is_empty() {
      // now go down through the dependencies by tree depth
      while let Some((mut visited_versions, parent_node)) =
        self.pending_unresolved_nodes.pop_front()
      {
        let (mut parent_id, deps) = {
          let parent_node = parent_node.lock();
          (parent_node.id.clone(), parent_node.deps.clone())
        };

        if !visited_versions.add(&parent_id) {
          continue; // circular
        }

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
        for dep in deps.iter() {
          let package_info = self.api.package_info(&dep.name).await?;

          eprintln!(
            "-- DEPENDENCY: {} ({})",
            dep.name,
            parent_id.as_serializable_name()
          );
          match dep.kind {
            NpmDependencyEntryKind::Dep => {
              self.analyze_dependency(
                &dep,
                package_info,
                &parent_id,
                &visited_versions,
              )?;
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              eprintln!("ANALYZING PEER DEP: {}", dep.name);
              let maybe_new_parent_id = self.resolve_peer_dep(
                &dep.bare_specifier,
                &parent_id,
                &dep,
                package_info,
                &visited_versions,
              )?;
              if let Some(new_parent_id) = maybe_new_parent_id {
                eprintln!(
                  "NEW PARENT ID: {} -> {}",
                  parent_id.as_serializable_name(),
                  new_parent_id.as_serializable_name()
                );
                assert_eq!(
                  (&new_parent_id.name, &new_parent_id.version),
                  (&parent_id.name, &parent_id.version)
                );
                parent_id = new_parent_id;
              }
            }
          }
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
    peer_package_info: NpmPackageInfo,
    visited_ancestor_versions: &VisitedVersions,
  ) -> Result<Option<NpmPackageId>, AnyError> {
    fn find_matching_child<'a>(
      peer_dep: &NpmDependencyEntry,
      children: impl Iterator<Item = &'a NpmPackageId>,
    ) -> Option<NpmPackageId> {
      for child_id in children {
        if child_id.name == peer_dep.name
          && peer_dep.version_req.satisfies(&child_id.version)
        {
          return Some(child_id.clone());
        }
      }
      None
    }

    eprintln!("[resolve_peer_dep]: Specifier: {}", specifier);
    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional.
    let mut pending_ancestors = VecDeque::new(); // go up the tree by depth
    let path = GraphPath::default().with_step(specifier, parent_id);
    eprintln!("[resolve_peer_dep]: Path: {:?}", path.specifiers);

    // skip over the current node
    for (specifier, grand_parents) in
      self.graph.borrow_node(&parent_id).parents.clone()
    {
      let path = path.with_specifier(specifier);
      for grand_parent in grand_parents {
        pending_ancestors.push_back((grand_parent, path.clone()));
      }
    }

    while let Some((ancestor, path)) = pending_ancestors.pop_front() {
      match &ancestor {
        NodeParent::Node(ancestor_node_id) => {
          // we've gone in a full circle, so don't keep looking
          if path.has_visited_version(ancestor_node_id) {
            continue;
          }

          let maybe_peer_dep_id = if ancestor_node_id.name == peer_dep.name
            && peer_dep.version_req.satisfies(&ancestor_node_id.version)
          {
            Some(ancestor_node_id.clone())
          } else {
            let ancestor = self.graph.borrow_node(ancestor_node_id);
            for (specifier, parents) in &ancestor.parents {
              let new_path = path.with_step(specifier, ancestor_node_id);
              for parent in parents {
                pending_ancestors.push_back((parent.clone(), new_path.clone()));
              }
            }
            find_matching_child(peer_dep, ancestor.children.values())
          };
          if let Some(peer_dep_id) = maybe_peer_dep_id {
            let parents =
              self.graph.borrow_node(ancestor_node_id).parents.clone();
            return Ok(Some(self.set_new_peer_dep(
              parents,
              ancestor_node_id,
              &peer_dep_id,
              path.specifiers,
              visited_ancestor_versions,
            )));
          }
        }
        NodeParent::Req(req) => {
          // in this case, the parent is the root so the children are all the package requirements
          if let Some(child_id) =
            find_matching_child(peer_dep, self.graph.package_reqs.values())
          {
            let old_id = self.graph.package_reqs.get(&req).unwrap().clone();
            let mut path = path.specifiers;
            path.pop(); // go back down one level
            return Ok(Some(self.set_new_peer_dep(
              HashMap::from([(
                req.to_string(),
                HashSet::from([NodeParent::Req(req.clone())]),
              )]),
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
    if !peer_dep.kind.is_optional() {
      self.analyze_dependency(
        &peer_dep,
        peer_package_info,
        &parent_id,
        &visited_ancestor_versions,
      )?;
    }

    Ok(None)
  }

  fn set_new_peer_dep(
    &mut self,
    previous_parents: HashMap<String, HashSet<NodeParent>>,
    node_id: &NpmPackageId,
    peer_dep_id: &NpmPackageId,
    mut path: Vec<String>,
    visited_ancestor_versions: &VisitedVersions,
  ) -> NpmPackageId {
    eprintln!("PREVIOUS PARENTS: {:?}", previous_parents);
    let mut peer_dep_id = Cow::Borrowed(peer_dep_id);
    let old_id = node_id;
    let (new_id, old_node_children) =
      if old_id.peer_dependencies.contains(&peer_dep_id) {
        // the parent has already resolved to using this peer dependency
        // via some other path, so we don't need to update its ids,
        // but instead only make a link to it
        (
          old_id.clone(),
          self.graph.borrow_node(old_id).children.clone(),
        )
      } else {
        let mut new_id = old_id.clone();
        new_id.peer_dependencies.push(peer_dep_id.as_ref().clone());

        // this will happen for circular dependencies
        if *old_id == *peer_dep_id {
          peer_dep_id = Cow::Owned(new_id.clone());
        }

        eprintln!("NEW ID: {}", new_id.as_serializable_name());
        eprintln!("PATH: {:?}", path);
        // remove the previous parents from the old node
        let old_node_children = {
          for (specifier, parents) in &previous_parents {
            for parent in parents {
              self.graph.remove_child_parent(&specifier, old_id, parent);
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
            .set_child_parent(&specifier, &child, &new_id_as_parent);
        }
        (new_id, old_node_children)
      };

    // this is the parent id found at the bottom of the path
    let mut bottom_parent_id = new_id.clone();

    // continue going down the path
    if let Some(next_specifier) = path.pop() {
      eprintln!(
        "Next specifier: {}, peer dep id: {}",
        next_specifier,
        peer_dep_id.as_serializable_name()
      );
      if path.is_empty() {
        // this means we're at the peer dependency now
        assert!(!old_node_children.contains_key(&next_specifier));
        let node = self.graph.get_or_create_for_id(&peer_dep_id).1;
        self
          .pending_unresolved_nodes
          .push_back((visited_ancestor_versions.clone(), node.clone()));
        self
          .graph
          .set_child_parent_node(&next_specifier, &node, &new_id);
      } else {
        let next_node_id = old_node_children.get(&next_specifier).unwrap();
        bottom_parent_id = self.set_new_peer_dep(
          HashMap::from([(
            next_specifier.to_string(),
            HashSet::from([NodeParent::Node(new_id.clone())]),
          )]),
          &next_node_id,
          &peer_dep_id,
          path,
          visited_ancestor_versions,
        );
      }
    }

    // forget the old node at this point if it has no parents
    if new_id != *old_id {
      eprintln!(
        "CHANGING ID: {} -> {}",
        old_id.as_serializable_name(),
        new_id.as_serializable_name()
      );
      let old_node = self.graph.borrow_node(old_id);
      eprintln!("OLD PARENTS: {:?}", old_node.parents);
      if old_node.parents.is_empty() {
        drop(old_node); // stop borrowing
        self.graph.forget_orphan(old_id);
      }
    }

    return bottom_parent_id;
  }
}

#[derive(Clone)]
struct VersionAndInfo {
  version: NpmVersion,
  info: NpmPackageVersionInfo,
}

fn get_resolved_package_version_and_info(
  pkg_name: &str,
  version_matcher: &impl NpmVersionMatcher,
  info: NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo, AnyError> {
  let mut maybe_best_version: Option<VersionAndInfo> = None;
  if let Some(tag) = version_matcher.tag() {
    // For when someone just specifies @types/node, we want to pull in a
    // "known good" version of @types/node that works well with Deno and
    // not necessarily the latest version. For example, we might only be
    // compatible with Node vX, but then Node vY is published so we wouldn't
    // want to pull that in.
    // Note: If the user doesn't want this behavior, then they can specify an
    // explicit version.
    if tag == "latest" && pkg_name == "@types/node" {
      return get_resolved_package_version_and_info(
        pkg_name,
        &NpmVersionReq::parse("18.0.0 - 18.8.2").unwrap(),
        info,
        parent,
      );
    }

    if let Some(version) = info.dist_tags.get(tag) {
      match info.versions.get(version) {
        Some(info) => {
          return Ok(VersionAndInfo {
            version: NpmVersion::parse(version)?,
            info: info.clone(),
          });
        }
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
  } else {
    for (_, version_info) in info.versions.into_iter() {
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
      pkg_name,
      version_matcher.version_text(),
      match parent {
        Some(id) => format!(" as specified in {}", id),
        None => String::new(),
      }
    ),
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
    let result = get_resolved_package_version_and_info(
      "test",
      &package_ref.req,
      NpmPackageInfo {
        name: "test".to_string(),
        versions: HashMap::new(),
        dist_tags: HashMap::from([(
          "latest".to_string(),
          "1.0.0-alpha".to_string(),
        )]),
      },
      None,
    );
    assert_eq!(
      result.err().unwrap().to_string(),
      "Could not find version '1.0.0-alpha' referenced in dist-tag 'latest'."
    );

    // dist tag where version is a pre-release
    let package_ref = NpmPackageReference::from_str("npm:test").unwrap();
    let result = get_resolved_package_version_and_info(
      "test",
      &package_ref.req,
      NpmPackageInfo {
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
      },
      None,
    );
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-alpha");
  }

  #[tokio::test]
  async fn resolve_no_peer_deps() {
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
          id: NpmPackageId::deserialize_name("package-a@1.0.0").unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name("package-c@0.1.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-c@0.1.0").unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-d".to_string(),
            NpmPackageId::deserialize_name("package-d@3.2.1").unwrap(),
          ),])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-d@3.2.1").unwrap(),
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
          id: NpmPackageId::deserialize_name(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-0@1.1.1").unwrap(),
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::deserialize_name(
              "package-a@1.0.0_package-peer@4.0.0"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer".to_string(),
              NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-a@1.0.0").unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-c@3.0.0").unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.1.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer@4.1.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-a@1.0.0").unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name("package-c@3.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-b@2.0.0").unwrap(),
          dist: Default::default(),
          dependencies: HashMap::new(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-c@3.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name(
            "package-a@1.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name(
                "package-b@2.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name(
                "package-c@3.0.0_package-peer@4.0.0"
              )
              .unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-b@2.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-c@3.0.0_package-peer@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer".to_string(),
            NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer@4.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-0@1.0.0").unwrap(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::deserialize_name("package-peer-a@2.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-a@2.0.0").unwrap(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::deserialize_name("package-peer-b@3.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-b@3.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name(
            "package-0@1.0.0_package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageId::deserialize_name(
                "package-peer-a@2.0.0_package-peer-b@3.0.0"
              )
              .unwrap(),
            ),
            (
              "package-peer-b".to_string(),
              NpmPackageId::deserialize_name("package-peer-b@3.0.0").unwrap(),
            )
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-peer-a@2.0.0_package-peer-b@3.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::deserialize_name("package-peer-b@3.0.0").unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-b@3.0.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-0@1.1.1").unwrap(),
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::deserialize_name(
              "package-a@1.0.0_package-peer-a@4.0.0"
            )
            .unwrap(),
          ),]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-a@1.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([
            (
              "package-b".to_string(),
              NpmPackageId::deserialize_name(
                "package-b@2.0.0_package-peer-a@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-c".to_string(),
              NpmPackageId::deserialize_name(
                "package-c@3.0.0_package-peer-a@4.0.0"
              )
              .unwrap(),
            ),
            (
              "package-d".to_string(),
              NpmPackageId::deserialize_name("package-d@3.5.0").unwrap(),
            ),
            (
              "package-peer-a".to_string(),
              NpmPackageId::deserialize_name("package-peer-a@4.0.0").unwrap(),
            ),
          ]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-b@2.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([
            (
              "package-peer-a".to_string(),
              NpmPackageId::deserialize_name("package-peer-a@4.0.0").unwrap(),
            ),
            (
              "package-peer-c".to_string(),
              NpmPackageId::deserialize_name("package-peer-c@6.2.0").unwrap(),
            )
          ])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-c@3.0.0_package-peer-a@4.0.0"
          )
          .unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-a".to_string(),
            NpmPackageId::deserialize_name("package-peer-a@4.0.0").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-d@3.5.0").unwrap(),
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-e@3.6.0").unwrap(),
          dependencies: HashMap::from([]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-a@4.0.0").unwrap(),
          dist: Default::default(),
          dependencies: HashMap::from([(
            "package-peer-b".to_string(),
            NpmPackageId::deserialize_name("package-peer-b@5.4.1").unwrap(),
          )])
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-b@5.4.1").unwrap(),
          dist: Default::default(),
          dependencies: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name("package-peer-c@6.2.0").unwrap(),
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
          id: NpmPackageId::deserialize_name("package-a@1.0.0_package-a@1.0.0")
            .unwrap(),
          dependencies: HashMap::from([(
            "package-b".to_string(),
            NpmPackageId::deserialize_name(
              "package-b@2.0.0_package-a@1.0.0__package-a@1.0.0"
            )
            .unwrap(),
          )]),
          dist: Default::default(),
        },
        NpmResolutionPackage {
          id: NpmPackageId::deserialize_name(
            "package-b@2.0.0_package-a@1.0.0__package-a@1.0.0"
          )
          .unwrap(),
          dependencies: HashMap::from([(
            "package-a".to_string(),
            NpmPackageId::deserialize_name("package-a@1.0.0_package-a@1.0.0")
              .unwrap(),
          )]),
          dist: Default::default(),
        },
      ]
    );
    assert_eq!(
      package_reqs,
      vec![(
        "package-a@1.0".to_string(),
        "package-a@1.0.0_package-a@1.0.0".to_string()
      )]
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
        .add_npm_package_req(&req, api.package_info(&req.name).await.unwrap())
        .unwrap();
    }

    resolver.resolve_pending().await.unwrap();
    let snapshot = graph.into_snapshot(&api).await.unwrap();
    let mut packages = snapshot.all_packages();
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut package_reqs = snapshot
      .package_reqs
      .into_iter()
      .map(|(a, b)| (a.to_string(), b.as_serializable_name()))
      .collect::<Vec<_>>();
    package_reqs.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    (packages, package_reqs)
  }
}
