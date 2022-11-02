// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::borrow::BorrowMut;
use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::rc::Rc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

use crate::lockfile::Lockfile;
use crate::npm::registry::NpmDependencyEntry;
use crate::npm::registry::NpmDependencyEntryKind;

use super::cache::should_sync_download;
use super::registry::NpmPackageInfo;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmPackageVersionInfo;
use super::registry::NpmRegistryApi;
use super::semver::NpmVersion;
use super::semver::NpmVersionReq;
use super::semver::SpecifierVersionReq;

/// The version matcher used for npm schemed urls is more strict than
/// the one used by npm packages and so we represent either via a trait.
pub trait NpmVersionMatcher {
  fn tag(&self) -> Option<&str>;
  fn matches(&self, version: &NpmVersion) -> bool;
  fn version_text(&self) -> String;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpmPackageReference {
  pub req: NpmPackageReq,
  pub sub_path: Option<String>,
}

impl NpmPackageReference {
  pub fn from_specifier(
    specifier: &ModuleSpecifier,
  ) -> Result<NpmPackageReference, AnyError> {
    Self::from_str(specifier.as_str())
  }

  pub fn from_str(specifier: &str) -> Result<NpmPackageReference, AnyError> {
    let specifier = match specifier.strip_prefix("npm:") {
      Some(s) => s,
      None => {
        bail!("Not an npm specifier: {}", specifier);
      }
    };
    let parts = specifier.split('/').collect::<Vec<_>>();
    let name_part_len = if specifier.starts_with('@') { 2 } else { 1 };
    if parts.len() < name_part_len {
      return Err(generic_error(format!("Not a valid package: {}", specifier)));
    }
    let name_parts = &parts[0..name_part_len];
    let last_name_part = &name_parts[name_part_len - 1];
    let (name, version_req) = if let Some(at_index) = last_name_part.rfind('@')
    {
      let version = &last_name_part[at_index + 1..];
      let last_name_part = &last_name_part[..at_index];
      let version_req = SpecifierVersionReq::parse(version)
        .with_context(|| "Invalid version requirement.")?;
      let name = if name_part_len == 1 {
        last_name_part.to_string()
      } else {
        format!("{}/{}", name_parts[0], last_name_part)
      };
      (name, Some(version_req))
    } else {
      (name_parts.join("/"), None)
    };
    let sub_path = if parts.len() == name_parts.len() {
      None
    } else {
      Some(parts[name_part_len..].join("/"))
    };

    if let Some(sub_path) = &sub_path {
      if let Some(at_index) = sub_path.rfind('@') {
        let (new_sub_path, version) = sub_path.split_at(at_index);
        let msg = format!(
          "Invalid package specifier 'npm:{}/{}'. Did you mean to write 'npm:{}{}/{}'?",
          name, sub_path, name, version, new_sub_path
        );
        return Err(generic_error(msg));
      }
    }

    Ok(NpmPackageReference {
      req: NpmPackageReq { name, version_req },
      sub_path,
    })
  }
}

impl std::fmt::Display for NpmPackageReference {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(sub_path) = &self.sub_path {
      write!(f, "{}/{}", self.req, sub_path)
    } else {
      write!(f, "{}", self.req)
    }
  }
}

#[derive(
  Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageReq {
  pub name: String,
  pub version_req: Option<SpecifierVersionReq>,
}

impl std::fmt::Display for NpmPackageReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.version_req {
      Some(req) => write!(f, "{}@{}", self.name, req),
      None => write!(f, "{}", self.name),
    }
  }
}

impl NpmVersionMatcher for NpmPackageReq {
  fn tag(&self) -> Option<&str> {
    match &self.version_req {
      Some(version_req) => version_req.tag(),
      None => Some("latest"),
    }
  }

  fn matches(&self, version: &NpmVersion) -> bool {
    match self.version_req.as_ref() {
      Some(req) => {
        assert_eq!(self.tag(), None);
        match req.range() {
          Some(range) => range.satisfies(version),
          None => false,
        }
      }
      None => version.pre.is_empty(),
    }
  }

  fn version_text(&self) -> String {
    self
      .version_req
      .as_ref()
      .map(|v| format!("{}", v))
      .unwrap_or_else(|| "non-prerelease".to_string())
  }
}

#[derive(
  Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageId {
  pub name: String,
  pub version: NpmVersion,
  pub peer_dependencies: Vec<NpmPackageId>,
}

impl NpmPackageId {
  #[allow(unused)]
  pub fn scope(&self) -> Option<&str> {
    if self.name.starts_with('@') && self.name.contains('/') {
      self.name.split('/').next()
    } else {
      None
    }
  }

  pub fn serialize_for_lock_file(&self) -> String {
    if !self.peer_dependencies.is_empty() {
      todo!();
    }
    format!("{}@{}", self.name, self.version)
  }

  pub fn deserialize_from_lock_file(id: &str) -> Result<Self, AnyError> {
    let reference = NpmPackageReference::from_str(&format!("npm:{}", id))
      .with_context(|| {
        format!("Unable to deserialize npm package reference: {}", id)
      })?;
    let version =
      NpmVersion::parse(&reference.req.version_req.unwrap().to_string())
        .unwrap();
    Ok(Self {
      name: reference.req.name,
      version,
      peer_dependencies: Vec::new(), // todo
    })
  }
}

impl std::fmt::Display for NpmPackageId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}@{}", self.name, self.version)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmResolutionPackage {
  pub id: NpmPackageId,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NpmResolutionSnapshot {
  #[serde(with = "map_to_vec")]
  package_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  package_versions_by_name: HashMap<String, Vec<NpmVersion>>,
  #[serde(with = "map_to_vec")]
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
}

// This is done so the maps with non-string keys get serialized and deserialized as vectors.
// Adapted from: https://github.com/serde-rs/serde/issues/936#issuecomment-302281792
mod map_to_vec {
  use std::collections::HashMap;

  use serde::de::Deserialize;
  use serde::de::Deserializer;
  use serde::ser::Serializer;
  use serde::Serialize;

  pub fn serialize<S, K: Serialize, V: Serialize>(
    map: &HashMap<K, V>,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.collect_seq(map.iter())
  }

  pub fn deserialize<
    'de,
    D,
    K: Deserialize<'de> + Eq + std::hash::Hash,
    V: Deserialize<'de>,
  >(
    deserializer: D,
  ) -> Result<HashMap<K, V>, D::Error>
  where
    D: Deserializer<'de>,
  {
    let mut map = HashMap::new();
    for (key, value) in Vec::<(K, V)>::deserialize(deserializer)? {
      map.insert(key, value);
    }
    Ok(map)
  }
}

impl NpmResolutionSnapshot {
  /// Resolve a node package from a deno module.
  pub fn resolve_package_from_deno_module(
    &self,
    req: &NpmPackageReq,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    match self.package_reqs.get(req) {
      Some(version) => Ok(
        self
          .packages
          .get(&NpmPackageId {
            name: req.name.clone(),
            version: version.clone(),
          })
          .unwrap(),
      ),
      None => bail!("could not find npm package directory for '{}'", req),
    }
  }

  pub fn top_level_packages(&self) -> Vec<NpmPackageId> {
    self
      .package_reqs
      .iter()
      .map(|(req, version)| NpmPackageId {
        name: req.name.clone(),
        version: version.clone(),
      })
      .collect::<HashSet<_>>()
      .into_iter()
      .collect::<Vec<_>>()
  }

  pub fn package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<&NpmResolutionPackage> {
    self.packages.get(id)
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageId,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    match self.packages.get(referrer) {
      Some(referrer_package) => {
        let name_ = name_without_path(name);
        if let Some(id) = referrer_package.dependencies.get(name_) {
          return Ok(self.packages.get(id).unwrap());
        }

        if referrer_package.id.name == name_ {
          return Ok(referrer_package);
        }

        // TODO(bartlomieju): this should use a reverse lookup table in the
        // snapshot instead of resolving best version again.
        let req = NpmPackageReq {
          name: name_.to_string(),
          version_req: None,
        };

        if let Some(version) = self.resolve_best_package_version(name_, &req) {
          let id = NpmPackageId {
            name: name_.to_string(),
            version,
          };
          if let Some(pkg) = self.packages.get(&id) {
            return Ok(pkg);
          }
        }

        bail!(
          "could not find npm package '{}' referenced by '{}'",
          name,
          referrer
        )
      }
      None => bail!("could not find referrer npm package '{}'", referrer),
    }
  }

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.packages.values().cloned().collect()
  }

  pub fn resolve_best_package_version(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Option<NpmVersion> {
    let mut maybe_best_version: Option<&NpmVersion> = None;
    if let Some(versions) = self.package_versions_by_name.get(name) {
      for version in versions {
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

  pub async fn from_lockfile(
    lockfile: Arc<Mutex<Lockfile>>,
    api: &NpmRegistryApi,
  ) -> Result<Self, AnyError> {
    let mut package_reqs: HashMap<NpmPackageReq, NpmVersion>;
    let mut packages_by_name: HashMap<String, Vec<NpmVersion>>;
    let mut packages: HashMap<NpmPackageId, NpmResolutionPackage>;

    {
      let lockfile = lockfile.lock();

      // pre-allocate collections
      package_reqs =
        HashMap::with_capacity(lockfile.content.npm.specifiers.len());
      packages = HashMap::with_capacity(lockfile.content.npm.packages.len());
      packages_by_name =
        HashMap::with_capacity(lockfile.content.npm.packages.len()); // close enough
      let mut verify_ids =
        HashSet::with_capacity(lockfile.content.npm.packages.len());

      // collect the specifiers to version mappings
      for (key, value) in &lockfile.content.npm.specifiers {
        let reference = NpmPackageReference::from_str(&format!("npm:{}", key))
          .with_context(|| format!("Unable to parse npm specifier: {}", key))?;
        let package_id = NpmPackageId::deserialize_from_lock_file(value)?;
        package_reqs.insert(reference.req, package_id.version.clone());
        verify_ids.insert(package_id.clone());
      }

      // then the packages
      for (key, value) in &lockfile.content.npm.packages {
        let package_id = NpmPackageId::deserialize_from_lock_file(key)?;
        let mut dependencies = HashMap::default();

        packages_by_name
          .entry(package_id.name.to_string())
          .or_default()
          .push(package_id.version.clone());

        for (name, specifier) in &value.dependencies {
          let dep_id = NpmPackageId::deserialize_from_lock_file(specifier)?;
          dependencies.insert(name.to_string(), dep_id.clone());
          verify_ids.insert(dep_id);
        }

        let package = NpmResolutionPackage {
          id: package_id.clone(),
          // temporary dummy value
          dist: NpmPackageVersionDistInfo {
            tarball: "foobar".to_string(),
            shasum: "foobar".to_string(),
            integrity: Some("foobar".to_string()),
          },
          dependencies,
        };

        packages.insert(package_id, package);
      }

      // verify that all these ids exist in packages
      for id in &verify_ids {
        if !packages.contains_key(id) {
          bail!(
            "the lockfile ({}) is corrupt. You can recreate it with --lock-write",
            lockfile.filename.display(),
          );
        }
      }
    }

    let mut unresolved_tasks = Vec::with_capacity(packages_by_name.len());

    // cache the package names in parallel in the registry api
    for package_name in packages_by_name.keys() {
      let package_name = package_name.clone();
      let api = api.clone();
      unresolved_tasks.push(tokio::task::spawn(async move {
        api.package_info(&package_name).await?;
        Result::<_, AnyError>::Ok(())
      }));
    }
    for result in futures::future::join_all(unresolved_tasks).await {
      result??;
    }

    // ensure the dist is set for each package
    for package in packages.values_mut() {
      // this will read from the memory cache now
      let package_info = api.package_info(&package.id.name).await?;
      let version_info = match package_info
        .versions
        .get(&package.id.version.to_string())
      {
        Some(version_info) => version_info,
        None => {
          bail!("could not find '{}' specified in the lockfile. Maybe try again with --reload", package.id);
        }
      };
      package.dist = version_info.dist.clone();
    }

    Ok(Self {
      package_reqs,
      package_versions_by_name: packages_by_name,
      packages,
    })
  }
}

pub struct NpmResolution {
  api: NpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_sempahore: tokio::sync::Semaphore,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.snapshot.read();
    f.debug_struct("NpmResolution")
      .field("snapshot", &snapshot)
      .finish()
  }
}

impl NpmResolution {
  pub fn new(
    api: NpmRegistryApi,
    initial_snapshot: Option<NpmResolutionSnapshot>,
  ) -> Self {
    Self {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_sempahore: tokio::sync::Semaphore::new(1),
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
    let snapshot = self.snapshot.read().clone();

    let snapshot = self
      .add_package_reqs_to_snapshot(package_reqs, snapshot)
      .await?;

    *self.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
    let snapshot = self.snapshot.read().clone();

    let has_removed_package = !snapshot
      .package_reqs
      .keys()
      .all(|req| package_reqs.contains(req));
    // if any packages were removed, we need to completely recreate the npm resolution snapshot
    let snapshot = if has_removed_package {
      NpmResolutionSnapshot::default()
    } else {
      snapshot
    };
    let snapshot = self
      .add_package_reqs_to_snapshot(
        package_reqs.into_iter().collect(),
        snapshot,
      )
      .await?;

    *self.snapshot.write() = snapshot;

    Ok(())
  }

  async fn add_package_reqs_to_snapshot(
    &self,
    mut package_reqs: Vec<NpmPackageReq>,
    mut snapshot: NpmResolutionSnapshot,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    // convert the snapshot to a traversable graph
    let mut graph = Graph::default();
    graph.fill_with_snapshot(&snapshot);

    // multiple packages are resolved in alphabetical order
    package_reqs.sort_by(|a, b| a.name.cmp(&b.name));

    // go over the top level packages first, then down the
    // tree one level at a time through all the branches
    let mut unresolved_tasks = Vec::with_capacity(package_reqs.len());
    for package_req in package_reqs {
      if graph.package_reqs.contains_key(&package_req) {
        // skip analyzing this package, as there's already a matching top level package
        continue;
      }

      // no existing best version, so resolve the current packages
      let api = self.api.clone();
      let maybe_info = if should_sync_download() {
        // for deterministic test output
        Some(api.package_info(&package_req.name).await)
      } else {
        None
      };
      unresolved_tasks.push(tokio::task::spawn(async move {
        let info = match maybe_info {
          Some(info) => info?,
          None => api.package_info(&package_req.name).await?,
        };
        Result::<_, AnyError>::Ok((package_req, info))
      }));
    }

    let mut resolver = GraphDependencyResolver {
      graph: &mut graph,
      api: &self.api,
      pending_dependencies: Default::default(),
      pending_peer_dependencies: Default::default(),
    };

    for result in futures::future::join_all(unresolved_tasks).await {
      let (package_req, info) = result??;
      resolver.resolve_npm_package_req(&package_req, info)?;
    }

    resolver.resolve_pending().await?;

    Ok(snapshot)
  }

  pub fn resolve_package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmResolutionPackage> {
    self.snapshot.read().package_from_id(id).cloned()
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageId,
  ) -> Result<NpmResolutionPackage, AnyError> {
    self
      .snapshot
      .read()
      .resolve_package_from_package(name, referrer)
      .cloned()
  }

  /// Resolve a node package from a deno module.
  pub fn resolve_package_from_deno_module(
    &self,
    package: &NpmPackageReq,
  ) -> Result<NpmResolutionPackage, AnyError> {
    self
      .snapshot
      .read()
      .resolve_package_from_deno_module(package)
      .cloned()
  }

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.snapshot.read().all_packages()
  }

  pub fn has_packages(&self) -> bool {
    !self.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.snapshot.read().clone()
  }

  pub fn lock(
    &self,
    lockfile: &mut Lockfile,
    snapshot: &NpmResolutionSnapshot,
  ) -> Result<(), AnyError> {
    for (package_req, version) in snapshot.package_reqs.iter() {
      lockfile.insert_npm_specifier(package_req, version.to_string());
    }
    for package in self.all_packages() {
      lockfile.check_or_insert_npm_package(&package)?;
    }
    Ok(())
  }
}
#[derive(Clone, PartialEq, Eq, Hash)]
enum NodeParent {
  Req(NpmPackageReq),
  Node(NpmPackageId),
}

struct Node {
  pub id: NpmPackageId,
  pub parents: HashSet<NodeParent>,
  pub children: HashSet<NpmPackageId>,
  pub unresolved_peers: Vec<NpmDependencyEntry>,
}

#[derive(Default)]
struct Graph {
  package_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  packages_by_name: HashMap<String, Vec<NpmPackageId>>,
  packages: HashMap<NpmPackageId, Rc<RefCell<Node>>>,
}

impl Graph {
  pub fn get_or_create_for_id(
    &mut self,
    id: &NpmPackageId,
  ) -> (bool, Rc<RefCell<Node>>) {
    if let Some(node) = self.packages.get(id) {
      (false, node.clone())
    } else {
      let node = Rc::new(RefCell::new(Node {
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

  pub fn fill_with_snapshot(&mut self, snapshot: &NpmResolutionSnapshot) {
    for (package_req, id) in &snapshot.package_reqs {
      let node = self.fill_for_id_with_snapshot(id, snapshot);
      (*node)
        .borrow_mut()
        .parents
        .insert(NodeParent::Req(package_req.clone()));
      self.package_reqs.insert(package_req.clone(), id.clone());
    }
  }

  fn fill_for_id_with_snapshot(
    &mut self,
    id: &NpmPackageId,
    snapshot: &NpmResolutionSnapshot,
  ) -> Rc<RefCell<Node>> {
    let resolution = snapshot.packages.get(id).unwrap();
    let node = self.get_or_create_for_id(id).1;
    for (name, child_id) in resolution.dependencies {
      let child_node = self.fill_for_id_with_snapshot(&child_id, snapshot);
      self.set_child_parent_node(&child_node, &id);
    }
    node
  }

  fn borrow_node(&self, id: &NpmPackageId) -> Ref<Node> {
    (**self.packages.get(id).unwrap()).borrow()
  }

  fn borrow_node_mut(&self, id: &NpmPackageId) -> RefMut<Node> {
    (**self.packages.get(id).unwrap()).borrow_mut()
  }

  fn forget_orphan(&mut self, node: &mut Node) {
    assert_eq!(node.parents.len(), 0);
    self.packages.remove(&node.id);
    let parent = NodeParent::Node(node.id.clone());
    for child_id in &node.children {
      let mut child = (**self.packages.get(child_id).unwrap()).borrow_mut();
      child.parents.remove(&parent);
      if child.parents.is_empty() {
        self.forget_orphan(&mut child);
      }
    }
  }

  pub fn set_child_parent_node(
    &mut self,
    child: &Rc<RefCell<Node>>,
    parent_id: &NpmPackageId,
  ) {
    let mut child = (**child).borrow_mut();
    let mut parent = (**self.packages.get(parent_id).unwrap()).borrow_mut();
    debug_assert_ne!(parent.id, child.id);
    parent.children.insert(child.id.clone());
    child.parents.insert(NodeParent::Node(parent.id.clone()));
  }
}

struct GraphDependencyResolver<'a> {
  graph: &'a mut Graph,
  api: &'a NpmRegistryApi,
  pending_dependencies: VecDeque<(NpmPackageId, Vec<NpmDependencyEntry>)>,
  pending_peer_dependencies:
    VecDeque<(NpmPackageId, NpmDependencyEntry, NpmPackageInfo)>,
}

impl<'a> GraphDependencyResolver<'a> {
  pub fn resolve_best_package_version_and_info(
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

  pub fn resolve_best_package_version(
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
    (*node)
      .borrow_mut()
      .parents
      .insert(NodeParent::Req(package_req.clone()));

    let dependencies = version_and_info
      .info
      .dependencies_as_entries()
      .with_context(|| format!("npm package: {}", id))?;

    self.pending_dependencies.push_back((id, dependencies));
    Ok(())
  }

  fn analyze_dependency(
    &mut self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
    package_info: NpmPackageInfo,
    parent_id: &NpmPackageId,
  ) -> Result<(), AnyError> {
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
    self.graph.set_child_parent_node(&node, &parent_id);

    if created {
      // inspect the dependencies of the package
      let dependencies = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| {
          format!("npm package: {}@{}", name, version_and_info.version)
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

          if matches!(
            dep.kind,
            NpmDependencyEntryKind::Peer | NpmDependencyEntryKind::OptionalPeer
          ) {
            self.pending_peer_dependencies.push_back((
              parent_id.clone(),
              dep,
              package_info,
            ));
          } else {
            self.analyze_dependency(
              &dep.name,
              &dep.version_req,
              package_info,
              &parent_id,
            )?;
          }
        }
      }

      if let Some((parent_id, peer_dep, peer_package_info)) =
        self.pending_peer_dependencies.pop_front()
      {
        self.resolve_peer_dep(
          &parent_id,
          &peer_dep,
          peer_package_info,
          vec![],
        )?;
        // peer_dep.version_req.satisfies(version)
        //parent_node.borrow_mut().
      }
    }
    Ok(())
  }

  fn resolve_peer_dep(
    &mut self,
    child_id: &NpmPackageId,
    peer_dep: &NpmDependencyEntry,
    peer_package_info: NpmPackageInfo,
    mut path: Vec<NpmPackageId>,
  ) -> Result<(), AnyError> {
    // Peer dependencies are resolved based on its ancestors' siblings.
    // If not found, then it resolves based on the version requirement if non-optional
    let parents = self.graph.borrow_node(&child_id).parents.clone();
    path.push(child_id.clone());
    for parent in parents {
      let children_ids = match &parent {
        NodeParent::Node(parent_node_id) => {
          self.graph.borrow_node(parent_node_id).children.clone()
        }
        NodeParent::Req(req) => self
          .graph
          .package_reqs
          .values()
          .cloned()
          .collect::<HashSet<_>>(),
      };
      for child_id in children_ids {
        if child_id.name == peer_dep.name
          && peer_dep.version_req.satisfies(&child_id.version)
        {
          // go down the descendants creating a new path
          match &parent {
            NodeParent::Node(node_id) => {
              let parents = self.graph.borrow_node(node_id).parents.clone();
              self.set_new_peer_dep(parents, node_id, &child_id, path);
              return Ok(());
            }
            NodeParent::Req(req) => {
              let old_id = self.graph.package_reqs.get(&req).unwrap().clone();
              self.set_new_peer_dep(
                HashSet::from([NodeParent::Req(req.clone())]),
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
      self.analyze_dependency(
        &peer_dep.name,
        &peer_dep.version_req,
        peer_package_info,
        &child_id,
      )?;
    }
    Ok(())
  }

  fn set_new_peer_dep(
    &mut self,
    previous_parents: HashSet<NodeParent>,
    node_id: &NpmPackageId,
    peer_dep_id: &NpmPackageId,
    path: Vec<NpmPackageId>,
  ) {
    let old_id = node_id;
    let mut new_id = old_id.clone();
    new_id.peer_dependencies.push(peer_dep_id.clone());
    // remove the previous parents from the old node
    let old_node_children = {
      let old_node = self.graph.borrow_node_mut(old_id);
      for previous_parent in &previous_parents {
        old_node.parents.remove(previous_parent);
      }
      old_node.children.clone()
    };

    let (created, new_node) = self.graph.get_or_create_for_id(&new_id);

    // update the previous parent to point to the new node
    // and this node to point at those parents
    {
      let new_node = (*new_node).borrow_mut();
      for parent in previous_parents {
        match &parent {
          NodeParent::Node(parent_id) => {
            let mut parent =
              (**self.graph.packages.get(parent_id).unwrap()).borrow_mut();
            parent.children.remove(&old_id);
            parent.children.insert(new_id.clone());
          }
          NodeParent::Req(req) => {
            self.graph.package_reqs.insert(req.clone(), new_id.clone());
          }
        }
        new_node.parents.insert(parent);
      }

      // now add the previous children to this node
      new_node.children.extend(old_node_children);
    }

    for child_id in old_node_children {
      self
        .graph
        .borrow_node_mut(&child_id)
        .parents
        .insert(NodeParent::Node(new_id.clone()));
    }

    if created {
      // continue going down the path
      let maybe_next_node = path.pop();
      if let Some(next_node_id) = path.pop() {
        self.set_new_peer_dep(
          HashSet::from([NodeParent::Node(new_id)]),
          &next_node_id,
          peer_dep_id,
          path,
        );
      }
    }

    // forget the old node at this point if it has no parents
    {
      let old_node = self.graph.borrow_node_mut(old_id);
      if old_node.parents.is_empty() {
        self.graph.forget_orphan(&mut old_node);
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

fn name_without_path(name: &str) -> &str {
  let mut search_start_index = 0;
  if name.starts_with('@') {
    if let Some(slash_index) = name.find('/') {
      search_start_index = slash_index + 1;
    }
  }
  if let Some(slash_index) = &name[search_start_index..].find('/') {
    // get the name up until the path slash
    &name[0..search_start_index + slash_index]
  } else {
    name
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_npm_package_ref() {
    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@1").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("1").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@^1.2").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("^1.2").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package")
        .err()
        .unwrap()
        .to_string(),
      "Not a valid package: @package"
    );
  }

  #[test]
  fn test_name_without_path() {
    assert_eq!(name_without_path("foo"), "foo");
    assert_eq!(name_without_path("@foo/bar"), "@foo/bar");
    assert_eq!(name_without_path("@foo/bar/baz"), "@foo/bar");
    assert_eq!(name_without_path("@hello"), "@hello");
  }

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
}
