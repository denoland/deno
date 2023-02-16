// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::RwLock;
use deno_graph::npm::NpmPackageId;
use deno_graph::npm::NpmPackageReq;
use log::debug;
use serde::Deserialize;
use serde::Serialize;

use crate::args::Lockfile;
use crate::npm::resolution::graph::LATEST_VERSION_REQ;

use self::common::resolve_best_package_version_and_info;
use self::graph::GraphDependencyResolver;
use self::snapshot::NpmPackagesPartitioned;

use super::cache::should_sync_download;
use super::cache::NpmPackageCacheFolderId;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmRegistryApi;

mod common;
mod graph;
mod snapshot;
mod specifier;

use graph::Graph;
pub use snapshot::NpmResolutionSnapshot;
pub use specifier::resolve_graph_npm_info;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmResolutionPackage {
  pub id: NpmPackageId,
  /// The peer dependency resolution can differ for the same
  /// package (name and version) depending on where it is in
  /// the resolution tree. This copy index indicates which
  /// copy of the package this is.
  pub copy_index: usize,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageId>,
}

impl NpmResolutionPackage {
  pub fn get_package_cache_folder_id(&self) -> NpmPackageCacheFolderId {
    NpmPackageCacheFolderId {
      name: self.id.name.clone(),
      version: self.id.version.clone(),
      copy_index: self.copy_index,
    }
  }
}

#[derive(Clone)]
pub struct NpmResolution(Arc<NpmResolutionInner>);

struct NpmResolutionInner {
  api: NpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_semaphore: tokio::sync::Semaphore,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.0.snapshot.read();
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
    Self(Arc::new(NpmResolutionInner {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_semaphore: tokio::sync::Semaphore::new(1),
    }))
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let inner = &self.0;
    let _permit = inner.update_semaphore.acquire().await?;
    let snapshot = inner.snapshot.read().clone();

    let snapshot = inner
      .add_package_reqs_to_snapshot(package_reqs, snapshot)
      .await?;

    *inner.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    let inner = &self.0;
    // only allow one thread in here at a time
    let _permit = inner.update_semaphore.acquire().await?;
    let snapshot = inner.snapshot.read().clone();

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
    let snapshot = add_package_reqs_to_snapshot(
      &inner.api,
      package_reqs.into_iter().collect(),
      snapshot,
    )
    .await?;

    *inner.snapshot.write() = snapshot;

    Ok(())
  }

  pub fn resolve_package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmResolutionPackage> {
    self.0.snapshot.read().package_from_id(id).cloned()
  }

  pub fn resolve_package_cache_folder_id_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .0
      .snapshot
      .read()
      .package_from_id(id)
      .map(|p| p.get_package_cache_folder_id())
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageCacheFolderId,
  ) -> Result<NpmResolutionPackage, AnyError> {
    self
      .0
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
      .0
      .snapshot
      .read()
      .resolve_package_from_deno_module(package)
      .cloned()
  }

  pub fn resolve_deno_graph_package_req(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<NpmPackageId, AnyError> {
    let inner = &self.0;
    let package_info = match inner.api.get_cached_package_info(&pkg_req.name) {
      Some(package_info) => package_info,
      // should never happen because we should have cached before
      None => bail!(
        "Deno bug. Please report: Could not find '{}' in npm package info cache.",
        pkg_req.name
      ),
    };

    let snapshot = inner.snapshot.write();
    let version_req =
      pkg_req.version_req.as_ref().unwrap_or(&*LATEST_VERSION_REQ);
    let version_and_info = resolve_best_package_version_and_info(
      version_req,
      &package_info,
      &snapshot.packages_by_name,
    )?;
    let id = NpmPackageId {
      name: package_info.name.to_string(),
      version: version_and_info.version.clone(),
      peer_dependencies: Vec::new(),
    };
    debug!(
      "Resolved {}@{} to {}",
      pkg_req.name,
      version_req.version_text(),
      id.as_serialized(),
    );
    snapshot.package_reqs.insert(pkg_req.clone(), id.clone());
    let packages = snapshot
      .packages_by_name
      .entry(package_info.name.clone())
      .or_default();
    if !packages.contains(&id) {
      packages.push(id);
    }
    snapshot.pending_unresolved_packages.push(id.clone());
    Ok(id)
  }

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    self.0.snapshot.read().all_packages_partitioned()
  }

  pub fn has_packages(&self) -> bool {
    !self.0.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.0.snapshot.read().clone()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.0.snapshot.read();
    for (package_req, package_id) in snapshot.package_reqs.iter() {
      lockfile.insert_npm_specifier(
        package_req.to_string(),
        package_id.as_serialized(),
      );
    }
    for package in snapshot.all_packages() {
      lockfile.check_or_insert_npm_package(package.into())?;
    }
    Ok(())
  }
}

async fn add_package_reqs_to_snapshot(
  api: &NpmRegistryApi,
  package_reqs: Vec<NpmPackageReq>,
  snapshot: NpmResolutionSnapshot,
) -> Result<NpmResolutionSnapshot, AnyError> {
  // convert the snapshot to a traversable graph
  let mut graph = Graph::from_snapshot(snapshot);

  // go over the top level package names first, then down the
  // tree one level at a time through all the branches
  let mut unresolved_tasks = Vec::with_capacity(package_reqs.len());
  let mut resolving_package_names = HashSet::with_capacity(package_reqs.len());
  for package_req in &package_reqs {
    if graph.has_package_req(package_req) {
      // skip analyzing this package, as there's already a matching top level package
      continue;
    }
    if !resolving_package_names.insert(package_req.name.clone()) {
      continue; // already resolving
    }

    // cache the package info up front in parallel
    if should_sync_download() {
      // for deterministic test output
      api.package_info(&package_req.name).await?;
    } else {
      let api = api.clone();
      let package_name = package_req.name.clone();
      unresolved_tasks.push(tokio::task::spawn(async move {
        // This is ok to call because api will internally cache
        // the package information in memory.
        api.package_info(&package_name).await
      }));
    };
  }

  for result in futures::future::join_all(unresolved_tasks).await {
    result??; // surface the first error
  }

  let mut resolver = GraphDependencyResolver::new(&mut graph, &api);

  // These package_reqs should already be sorted in the order they should
  // be resolved in.
  for package_req in package_reqs {
    // avoid loading the info if this is already in the graph
    if !resolver.has_package_req(&package_req) {
      let info = api.package_info(&package_req.name).await?;
      resolver.add_package_req(&package_req, &info)?;
    }
  }

  resolver.resolve_pending().await?;

  let result = graph.into_snapshot(&api).await;
  api.clear_memory_cache();
  result
}
