// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;

use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::RwLock;
use deno_graph::npm::NpmPackageId;
use deno_graph::npm::NpmPackageReq;
use serde::Deserialize;
use serde::Serialize;

use crate::args::Lockfile;

use self::graph::GraphDependencyResolver;
use self::snapshot::NpmPackagesPartitioned;

use super::cache::should_sync_download;
use super::cache::NpmPackageCacheFolderId;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::RealNpmRegistryApi;
use super::NpmRegistryApi;

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

pub struct NpmResolution {
  api: RealNpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_semaphore: tokio::sync::Semaphore,
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
    api: RealNpmRegistryApi,
    initial_snapshot: Option<NpmResolutionSnapshot>,
  ) -> Self {
    Self {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_semaphore: tokio::sync::Semaphore::new(1),
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_semaphore.acquire().await?;
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
    let _permit = self.update_semaphore.acquire().await?;
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
    package_reqs: Vec<NpmPackageReq>,
    snapshot: NpmResolutionSnapshot,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    // convert the snapshot to a traversable graph
    let mut graph = Graph::from_snapshot(snapshot);

    // go over the top level package names first, then down the
    // tree one level at a time through all the branches
    let mut unresolved_tasks = Vec::with_capacity(package_reqs.len());
    let mut resolving_package_names =
      HashSet::with_capacity(package_reqs.len());
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
        self.api.package_info(&package_req.name).await?;
      } else {
        let api = self.api.clone();
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

    let mut resolver = GraphDependencyResolver::new(&mut graph, &self.api);

    // These package_reqs should already be sorted in the order they should
    // be resolved in.
    for package_req in package_reqs {
      // avoid loading the info if this is already in the graph
      if !resolver.has_package_req(&package_req) {
        let info = self.api.package_info(&package_req.name).await?;
        resolver.add_package_req(&package_req, &info)?;
      }
    }

    resolver.resolve_pending().await?;

    let result = graph.into_snapshot(&self.api).await;
    self.api.clear_memory_cache();
    result
  }

  pub fn resolve_package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmResolutionPackage> {
    self.snapshot.read().package_from_id(id).cloned()
  }

  pub fn resolve_package_cache_folder_id_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmPackageCacheFolderId> {
    self
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

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    self.snapshot.read().all_packages_partitioned()
  }

  pub fn has_packages(&self) -> bool {
    !self.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.snapshot.read().clone()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.snapshot.read();
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
