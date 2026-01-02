// Copyright 2018-2025 the Deno authors. MIT license.

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_npm::resolution::NpmPackagesPartitioned;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageCacheFolderIdNotFoundError;
use deno_npm::resolution::PackageNotFoundFromReferrerError;
use deno_npm::resolution::PackageNvNotFoundError;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_unsync::sync::AtomicFlag;
use parking_lot::RwLock;

#[allow(clippy::disallowed_types)]
pub type NpmResolutionCellRc = deno_maybe_sync::MaybeArc<NpmResolutionCell>;

/// Handles updating and storing npm resolution in memory.
///
/// This does not interact with the file system.
#[derive(Default)]
pub struct NpmResolutionCell {
  snapshot: RwLock<NpmResolutionSnapshot>,
  is_pending: deno_unsync::sync::AtomicFlag,
}

impl std::fmt::Debug for NpmResolutionCell {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.snapshot.read();
    f.debug_struct("NpmResolution")
      .field("snapshot", &snapshot.as_valid_serialized().as_serialized())
      .finish()
  }
}

impl NpmResolutionCell {
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
      is_pending: AtomicFlag::lowered(),
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

  pub fn package_reqs(&self) -> Vec<(PackageReq, PackageNv)> {
    self
      .snapshot
      .read()
      .package_reqs()
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect()
  }

  pub fn top_level_packages(&self) -> Vec<NpmPackageId> {
    self
      .snapshot
      .read()
      .top_level_packages()
      .cloned()
      .collect::<Vec<_>>()
  }

  pub fn any_top_level_package(
    &self,
    check: impl Fn(&NpmPackageId) -> bool,
  ) -> bool {
    self.snapshot.read().top_level_packages().any(check)
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

  /// Checks if the resolution is "pending" meaning that its
  /// current state requires an npm install to get it up
  /// to date. This occurs when the workspace config changes
  /// and deno_lockfile has incompletely updated the npm
  /// snapshot.
  pub fn is_pending(&self) -> bool {
    self.is_pending.is_raised()
  }

  pub fn mark_pending(&self) {
    self.is_pending.raise();
  }

  pub fn mark_not_pending(&self) {
    self.is_pending.lower();
  }
}
