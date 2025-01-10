// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_npm::resolution::NpmPackagesPartitioned;
use deno_npm::resolution::NpmResolutionSnapshot;
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
use parking_lot::RwLock;

#[allow(clippy::disallowed_types)]
pub type NpmResolutionRc = crate::sync::MaybeArc<NpmResolution>;

/// Handles updating and storing npm resolution in memory.
///
/// This does not interact with the file system.
pub struct NpmResolution {
  snapshot: RwLock<NpmResolutionSnapshot>,
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
    initial_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  ) -> Self {
    let snapshot =
      NpmResolutionSnapshot::new(initial_snapshot.unwrap_or_default());
    Self::new(snapshot)
  }

  pub fn new(initial_snapshot: NpmResolutionSnapshot) -> Self {
    Self {
      snapshot: RwLock::new(initial_snapshot),
    }
  }

  pub fn resolve_pkg_cache_folder_copy_index_from_pkg_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<u8> {
    self
      .snapshot
      .read()
      .package_from_id(id)
      .map(|p| p.copy_index)
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

  pub fn subset(&self, package_reqs: &[PackageReq]) -> NpmResolutionSnapshot {
    self.snapshot.read().subset(package_reqs)
  }

  pub fn set_snapshot(&self, snapshot: NpmResolutionSnapshot) {
    *self.snapshot.write() = snapshot;
  }
}
