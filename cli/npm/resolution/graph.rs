// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NodeParent {
  Req(NpmPackageReq),
  Node(NpmPackageId),
}

#[derive(Debug)]
struct Node {
  pub id: NpmPackageId,
  pub parents: HashMap<String, NodeParent>,
  pub children: HashMap<String, NpmPackageId>,
  pub unresolved_peers: Vec<NpmDependencyEntry>,
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
      (*node).lock().parents.insert(
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
        unresolved_peers: Default::default(),
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
    (**self.packages.get(id).unwrap()).lock()
  }

  fn forget_orphan(&mut self, node_id: &NpmPackageId) {
    if let Some(node) = self.packages.remove(node_id) {
      let node = (*node).lock();
      assert_eq!(node.parents.len(), 0);
      for (specifier, child_id) in &node.children {
        let mut child = (**self.packages.get(child_id).unwrap()).lock();
        child.parents.remove(specifier);
        if child.parents.is_empty() {
          drop(child); // stop borrowing from self
          self.forget_orphan(&child_id);
        }
      }
    }
  }

  fn set_child_parent_node(
    &mut self,
    specifier: &str,
    child: &Arc<Mutex<Node>>,
    parent_id: &NpmPackageId,
  ) {
    let mut child = (**child).lock();
    let mut parent = (**self.packages.get(parent_id).unwrap()).lock();
    debug_assert_ne!(parent.id, child.id);
    parent
      .children
      .insert(specifier.to_string(), child.id.clone());
    child
      .parents
      .insert(specifier.to_string(), NodeParent::Node(parent.id.clone()));
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
      assert_eq!(node.unresolved_peers.len(), 0);
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
  pending_dependencies: VecDeque<(NpmPackageId, Vec<NpmDependencyEntry>)>,
  pending_peer_dependencies:
    VecDeque<((String, NpmPackageId), NpmDependencyEntry, NpmPackageInfo)>,
}

impl<'a, TNpmRegistryApi: NpmRegistryApi>
  GraphDependencyResolver<'a, TNpmRegistryApi>
{
  pub fn new(graph: &'a mut Graph, api: &'a TNpmRegistryApi) -> Self {
    Self {
      graph,
      api,
      pending_dependencies: Default::default(),
      pending_peer_dependencies: Default::default(),
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

  pub fn resolve_npm_package_req(
    &mut self,
    package_req: &NpmPackageReq,
    info: NpmPackageInfo,
  ) -> Result<(), AnyError> {
    // inspect if there's a match in the list of current packages and otherwise
    // fall back to looking at the registry
    let version_and_info = self.resolve_best_package_version_and_info(
      &package_req.name,
      package_req,
      info,
    )?;
    let id = NpmPackageId {
      name: package_req.name.clone(),
      version: version_and_info.version.clone(),
      peer_dependencies: Vec::new(),
    };
    let node = self.graph.get_or_create_for_id(&id).1;
    (*node).lock().parents.insert(
      package_req.to_string(),
      NodeParent::Req(package_req.clone()),
    );
    self
      .graph
      .package_reqs
      .insert(package_req.clone(), id.clone());

    let dependencies = version_and_info
      .info
      .dependencies_as_entries()
      .with_context(|| format!("npm package: {}", id))?;

    self.pending_dependencies.push_back((id, dependencies));
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    entry: &NpmDependencyEntry,
    package_info: NpmPackageInfo,
    parent_id: &NpmPackageId,
  ) -> Result<(), AnyError> {
    let version_and_info = self.resolve_best_package_version_and_info(
      &entry.name,
      &entry.version_req,
      package_info,
    )?;

    let id = NpmPackageId {
      name: entry.name.clone(),
      version: version_and_info.version.clone(),
      peer_dependencies: Vec::new(),
    };
    let (created, node) = self.graph.get_or_create_for_id(&id);
    self
      .graph
      .set_child_parent_node(&entry.bare_specifier, &node, &parent_id);

    if created {
      // inspect the dependencies of the package
      let dependencies = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| {
          format!("npm package: {}@{}", &entry.name, version_and_info.version)
        })?;

      self.pending_dependencies.push_back((id, dependencies));
    }
    Ok(())
  }

  pub async fn resolve_pending(&mut self) -> Result<(), AnyError> {
    while !self.pending_dependencies.is_empty()
      || !self.pending_peer_dependencies.is_empty()
    {
      // now go down through the dependencies by tree depth
      while let Some((parent_id, mut deps)) =
        self.pending_dependencies.pop_front()
      {
        // ensure name alphabetical and then version descending
        deps.sort();

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

        // resolve the non-peer dependencies
        for dep in deps {
          let package_info = self.api.package_info(&dep.name).await?;

          match dep.kind {
            NpmDependencyEntryKind::Dep => {
              self.analyze_dependency(&dep, package_info, &parent_id)?;
            }
            NpmDependencyEntryKind::Peer
            | NpmDependencyEntryKind::OptionalPeer => {
              self.pending_peer_dependencies.push_back((
                (dep.bare_specifier.clone(), parent_id.clone()),
                dep,
                package_info,
              ));
            }
          }
        }
      }

      // if a peer dependency was found, resolve one of them then go back
      // to resolving dependenices above before moving on to the next
      // peer dependency
      if let Some(((specifier, parent_id), peer_dep, peer_package_info)) =
        self.pending_peer_dependencies.pop_front()
      {
        self.resolve_peer_dep(
          &specifier,
          &parent_id,
          &peer_dep,
          peer_package_info,
          vec![],
        )?;
      }
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    specifier: &str,
    child_id: &NpmPackageId,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: NpmPackageInfo,
    mut path: Vec<(String, NpmPackageId)>,
  ) -> Result<(), AnyError> {
    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional
    let parents = self.graph.borrow_node(&child_id).parents.clone();
    path.push((specifier.to_string(), child_id.clone()));
    for (specifier, parent) in parents {
      let children = match &parent {
        NodeParent::Node(parent_node_id) => {
          self.graph.borrow_node(parent_node_id).children.clone()
        }
        NodeParent::Req(parent_req) => self
          .graph
          .package_reqs
          .iter()
          .filter(|(req, _)| *req == parent_req)
          .map(|(req, id)| (req.to_string(), id.clone()))
          .collect::<HashMap<_, _>>(),
      };
      // todo(THIS PR): don't we need to use the specifier here?
      for (child_specifier, child_id) in children {
        if child_id.name == peer_dep.name
          && peer_dep.version_req.satisfies(&child_id.version)
        {
          // go down the descendants creating a new path
          match &parent {
            NodeParent::Node(node_id) => {
              let parents = self.graph.borrow_node(node_id).parents.clone();
              self.set_new_peer_dep(
                parents, &specifier, node_id, &child_id, path,
              );
              return Ok(());
            }
            NodeParent::Req(req) => {
              let old_id = self.graph.package_reqs.get(&req).unwrap().clone();
              self.set_new_peer_dep(
                HashMap::from([(
                  req.to_string(),
                  NodeParent::Req(req.clone()),
                )]),
                &specifier,
                &old_id,
                &child_id,
                path,
              );
              return Ok(());
            }
          }
        }
      }
    }

    // at this point it means we didn't find anything by searching the ancestor siblings,
    // so we need to resolve based on the package info
    if !peer_dep.kind.is_optional() {
      self.analyze_dependency(&peer_dep, peer_package_info, &child_id)?;
    }
    Ok(())
  }

  fn set_new_peer_dep(
    &mut self,
    previous_parents: HashMap<String, NodeParent>,
    specifier: &str,
    node_id: &NpmPackageId,
    peer_dep_id: &NpmPackageId,
    mut path: Vec<(String, NpmPackageId)>,
  ) {
    let old_id = node_id;
    let mut new_id = old_id.clone();
    new_id.peer_dependencies.push(peer_dep_id.clone());
    // remove the previous parents from the old node
    let old_node_children = {
      let mut old_node = self.graph.borrow_node(old_id);
      for previous_parent in previous_parents.keys() {
        old_node.parents.remove(previous_parent);
      }
      old_node.children.clone()
    };

    let (created, new_node) = self.graph.get_or_create_for_id(&new_id);

    // update the previous parent to point to the new node
    // and this node to point at those parents
    {
      let mut new_node = (*new_node).lock();
      for (specifier, parent) in previous_parents {
        match &parent {
          NodeParent::Node(parent_id) => {
            let mut parent =
              (**self.graph.packages.get(parent_id).unwrap()).lock();
            parent.children.insert(specifier.clone(), new_id.clone());
          }
          NodeParent::Req(req) => {
            self.graph.package_reqs.insert(req.clone(), new_id.clone());
          }
        }
        new_node.parents.insert(specifier, parent);
      }

      // now add the previous children to this node
      new_node.children.extend(old_node_children.clone());
    }

    for (specifier, child_id) in old_node_children {
      self
        .graph
        .borrow_node(&child_id)
        .parents
        .insert(specifier, NodeParent::Node(new_id.clone()));
    }

    if created {
      // continue going down the path
      if let Some((next_specifier, next_node_id)) = path.pop() {
        self.set_new_peer_dep(
          HashMap::from([(specifier.to_string(), NodeParent::Node(new_id))]),
          &next_specifier,
          &next_node_id,
          peer_dep_id,
          path,
        );
      }
    }

    // forget the old node at this point if it has no parents
    {
      let old_node = self.graph.borrow_node(old_id);
      if old_node.parents.is_empty() {
        drop(old_node); // stop borrowing
        self.graph.forget_orphan(old_id);
      }
    }
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
      bail!("Could not find dist-tag '{}'.", tag,)
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

    let packages =
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
  }

  async fn run_resolver_and_get_output(
    api: TestNpmRegistryApi,
    reqs: Vec<&str>,
  ) -> Vec<NpmResolutionPackage> {
    let mut graph = Graph::default();
    let mut resolver = GraphDependencyResolver::new(&mut graph, &api);

    for req in reqs {
      let req = NpmPackageReference::from_str(req).unwrap().req;
      resolver
        .resolve_npm_package_req(
          &req,
          api.package_info(&req.name).await.unwrap(),
        )
        .unwrap();
    }

    resolver.resolve_pending().await.unwrap();
    let mut packages = graph.into_snapshot(&api).await.unwrap().all_packages();
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    packages
  }
}
