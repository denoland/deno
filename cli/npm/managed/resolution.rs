// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::NpmPackageVersionResolutionError;
use deno_npm::resolution::NpmPackagesPartitioned;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::NpmResolutionSnapshotPendingResolver;
use deno_npm::resolution::NpmResolutionSnapshotPendingResolverOptions;
use deno_npm::resolution::PackageCacheFolderIdNotFoundError;
use deno_npm::resolution::PackageNotFoundFromReferrerError;
use deno_npm::resolution::PackageNvNotFoundError;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::VersionReq;

use crate::args::Lockfile;
use crate::util::sync::TaskQueue;

use super::CliNpmRegistryApi;

/// Handles updating and storing npm resolution in memory where the underlying
/// snapshot can be updated concurrently. Additionally handles updating the lockfile
/// based on changes to the resolution.
///
/// This does not interact with the file system.
pub struct NpmResolution {
  api: Arc<CliNpmRegistryApi>,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_queue: TaskQueue,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.snapshot.read();
    f.debug_struct("NpmResolution")
      .field("snapshot", &snapshot.as_valid_serialized().as_serialized())
      .finish()
  }
}

impl NpmResolution {
  pub fn from_serialized(
    api: Arc<CliNpmRegistryApi>,
    initial_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let snapshot =
      NpmResolutionSnapshot::new(initial_snapshot.unwrap_or_default());
    Self::new(api, snapshot, maybe_lockfile)
  }

  pub fn new(
    api: Arc<CliNpmRegistryApi>,
    initial_snapshot: NpmResolutionSnapshot,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    Self {
      api,
      snapshot: RwLock::new(initial_snapshot),
      update_queue: Default::default(),
      maybe_lockfile,
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_queue.acquire().await;
    let snapshot = add_package_reqs_to_snapshot(
      &self.api,
      package_reqs,
      self.maybe_lockfile.clone(),
      || self.snapshot.read().clone(),
    )
    .await?;

    *self.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_queue.acquire().await;

    let reqs_set = package_reqs.iter().collect::<HashSet<_>>();
    let snapshot = add_package_reqs_to_snapshot(
      &self.api,
      package_reqs,
      self.maybe_lockfile.clone(),
      || {
        let snapshot = self.snapshot.read().clone();
        let has_removed_package = !snapshot
          .package_reqs()
          .keys()
          .all(|req| reqs_set.contains(req));
        // if any packages were removed, we need to completely recreate the npm resolution snapshot
        if has_removed_package {
          snapshot.into_empty()
        } else {
          snapshot
        }
      },
    )
    .await?;

    *self.snapshot.write() = snapshot;

    Ok(())
  }

  pub async fn resolve_pending(&self) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_queue.acquire().await;

    let snapshot = add_package_reqs_to_snapshot(
      &self.api,
      &Vec::new(),
      self.maybe_lockfile.clone(),
      || self.snapshot.read().clone(),
    )
    .await?;

    *self.snapshot.write() = snapshot;

    Ok(())
  }

  pub fn resolve_pkg_cache_folder_id_from_pkg_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .snapshot
      .read()
      .package_from_id(id)
      .map(|p| p.get_package_cache_folder_id())
  }

  pub fn resolve_pkg_id_from_pkg_cache_folder_id(
    &self,
    id: &NpmPackageCacheFolderId,
  ) -> Result<NpmPackageId, PackageCacheFolderIdNotFoundError> {
    self
      .snapshot
      .read()
      .resolve_pkg_from_pkg_cache_folder_id(id)
      .map(|pkg| pkg.id.clone())
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageCacheFolderId,
  ) -> Result<NpmResolutionPackage, Box<PackageNotFoundFromReferrerError>> {
    self
      .snapshot
      .read()
      .resolve_package_from_package(name, referrer)
      .cloned()
  }

  /// Resolve a node package from a deno module.
  pub fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self
      .snapshot
      .read()
      .resolve_pkg_from_pkg_req(req)
      .map(|pkg| pkg.id.clone())
  }

  pub fn resolve_pkg_reqs_from_pkg_id(
    &self,
    id: &NpmPackageId,
  ) -> Vec<PackageReq> {
    let snapshot = self.snapshot.read();
    let mut pkg_reqs = snapshot
      .package_reqs()
      .iter()
      .filter(|(_, nv)| *nv == &id.nv)
      .map(|(req, _)| req.clone())
      .collect::<Vec<_>>();
    pkg_reqs.sort(); // be deterministic
    pkg_reqs
  }

  pub fn resolve_pkg_id_from_deno_module(
    &self,
    id: &PackageNv,
  ) -> Result<NpmPackageId, PackageNvNotFoundError> {
    self
      .snapshot
      .read()
      .resolve_package_from_deno_module(id)
      .map(|pkg| pkg.id.clone())
  }

  /// Resolves a package requirement for deno graph. This should only be
  /// called by deno_graph's NpmResolver or for resolving packages in
  /// a package.json
  pub fn resolve_pkg_req_as_pending(
    &self,
    pkg_req: &PackageReq,
  ) -> Result<PackageNv, NpmPackageVersionResolutionError> {
    // we should always have this because it should have been cached before here
    let package_info = self.api.get_cached_package_info(&pkg_req.name).unwrap();
    self.resolve_pkg_req_as_pending_with_info(pkg_req, &package_info)
  }

  /// Resolves a package requirement for deno graph. This should only be
  /// called by deno_graph's NpmResolver or for resolving packages in
  /// a package.json
  pub fn resolve_pkg_req_as_pending_with_info(
    &self,
    pkg_req: &PackageReq,
    package_info: &NpmPackageInfo,
  ) -> Result<PackageNv, NpmPackageVersionResolutionError> {
    debug_assert_eq!(pkg_req.name, package_info.name);
    let mut snapshot = self.snapshot.write();
    let pending_resolver = get_npm_pending_resolver(&self.api);
    let nv = pending_resolver.resolve_package_req_as_pending(
      &mut snapshot,
      pkg_req,
      package_info,
    )?;
    Ok(nv)
  }

  pub fn package_reqs(&self) -> HashMap<PackageReq, PackageNv> {
    self.snapshot.read().package_reqs().clone()
  }

  pub fn all_system_packages(
    &self,
    system_info: &NpmSystemInfo,
  ) -> Vec<NpmResolutionPackage> {
    self.snapshot.read().all_system_packages(system_info)
  }

  pub fn all_system_packages_partitioned(
    &self,
    system_info: &NpmSystemInfo,
  ) -> NpmPackagesPartitioned {
    self
      .snapshot
      .read()
      .all_system_packages_partitioned(system_info)
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.snapshot.read().clone()
  }

  pub fn serialized_valid_snapshot(
    &self,
  ) -> ValidSerializedNpmResolutionSnapshot {
    self.snapshot.read().as_valid_serialized()
  }

  pub fn serialized_valid_snapshot_for_system(
    &self,
    system_info: &NpmSystemInfo,
  ) -> ValidSerializedNpmResolutionSnapshot {
    self
      .snapshot
      .read()
      .as_valid_serialized_for_system(system_info)
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.snapshot.read();
    populate_lockfile_from_snapshot(lockfile, &snapshot)
  }
}

async fn add_package_reqs_to_snapshot(
  api: &CliNpmRegistryApi,
  package_reqs: &[PackageReq],
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  get_new_snapshot: impl Fn() -> NpmResolutionSnapshot,
) -> Result<NpmResolutionSnapshot, AnyError> {
  let snapshot = get_new_snapshot();
  let snapshot = if !snapshot.has_pending()
    && package_reqs
      .iter()
      .all(|req| snapshot.package_reqs().contains_key(req))
  {
    log::debug!("Snapshot already up to date. Skipping pending resolution.");
    snapshot
  } else {
    let pending_resolver = get_npm_pending_resolver(api);
    let result = pending_resolver
      .resolve_pending(snapshot, package_reqs)
      .await;
    api.clear_memory_cache();
    match result {
      Ok(snapshot) => snapshot,
      Err(NpmResolutionError::Resolution(err)) if api.mark_force_reload() => {
        log::debug!("{err:#}");
        log::debug!("npm resolution failed. Trying again...");

        // try again
        let snapshot = get_new_snapshot();
        let result = pending_resolver
          .resolve_pending(snapshot, package_reqs)
          .await;
        api.clear_memory_cache();
        // now surface the result after clearing the cache
        result?
      }
      Err(err) => return Err(err.into()),
    }
  };

  if let Some(lockfile_mutex) = maybe_lockfile {
    let mut lockfile = lockfile_mutex.lock();
    populate_lockfile_from_snapshot(&mut lockfile, &snapshot)?;
  }

  Ok(snapshot)
}

fn get_npm_pending_resolver(
  api: &CliNpmRegistryApi,
) -> NpmResolutionSnapshotPendingResolver<CliNpmRegistryApi> {
  NpmResolutionSnapshotPendingResolver::new(
    NpmResolutionSnapshotPendingResolverOptions {
      api,
      // WARNING: When bumping this version, check if anything needs to be
      // updated in the `setNodeOnlyGlobalNames` call in 99_main_compiler.js
      types_node_version_req: Some(
        VersionReq::parse_from_npm("18.0.0 - 18.16.19").unwrap(),
      ),
    },
  )
}

fn populate_lockfile_from_snapshot(
  lockfile: &mut Lockfile,
  snapshot: &NpmResolutionSnapshot,
) -> Result<(), AnyError> {
  for (package_req, nv) in snapshot.package_reqs() {
    lockfile.insert_package_specifier(
      format!("npm:{}", package_req),
      format!(
        "npm:{}",
        snapshot
          .resolve_package_from_deno_module(nv)
          .unwrap()
          .id
          .as_serialized()
      ),
    );
  }
  for package in snapshot.all_packages_for_every_system() {
    lockfile
      .check_or_insert_npm_package(npm_package_to_lockfile_info(package))?;
  }
  Ok(())
}

fn npm_package_to_lockfile_info(
  pkg: &NpmResolutionPackage,
) -> NpmPackageLockfileInfo {
  let dependencies = pkg
    .dependencies
    .iter()
    .map(|(name, id)| NpmPackageDependencyLockfileInfo {
      name: name.clone(),
      id: id.as_serialized(),
    })
    .collect();

  NpmPackageLockfileInfo {
    display_id: pkg.id.nv.to_string(),
    serialized_id: pkg.id.as_serialized(),
    integrity: pkg.dist.integrity().for_lockfile(),
    dependencies,
  }
}
