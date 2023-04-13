// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::TaskQueue;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::resolution::NpmPackageVersionResolutionError;
use deno_npm::resolution::NpmPackagesPartitioned;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::NpmResolutionSnapshotCreateOptions;
use deno_npm::resolution::PackageNotFoundFromReferrerError;
use deno_npm::resolution::PackageNvNotFoundError;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_semver::npm::NpmPackageNv;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReq;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::VersionReq;

use crate::args::Lockfile;

use super::registry::CliNpmRegistryApi;

/// Handles updating and storing npm resolution in memory where the underlying
/// snapshot can be updated concurrently. Additionally handles updating the lockfile
/// based on changes to the resolution.
///
/// This does not interact with the file system.
#[derive(Clone)]
pub struct NpmResolution(Arc<NpmResolutionInner>);

struct NpmResolutionInner {
  api: CliNpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_queue: TaskQueue,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.0.snapshot.read();
    f.debug_struct("NpmResolution")
      .field("snapshot", &snapshot.as_serialized())
      .finish()
  }
}

impl NpmResolution {
  pub fn from_serialized(
    api: CliNpmRegistryApi,
    initial_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let snapshot =
      NpmResolutionSnapshot::new(NpmResolutionSnapshotCreateOptions {
        api: Arc::new(api.clone()),
        snapshot: initial_snapshot.unwrap_or_default(),
        // WARNING: When bumping this version, check if anything needs to be
        // updated in the `setNodeOnlyGlobalNames` call in 99_main_compiler.js
        types_node_version_req: Some(
          VersionReq::parse_from_npm("18.0.0 - 18.11.18").unwrap(),
        ),
      });
    Self::new(api, snapshot, maybe_lockfile)
  }

  pub fn new(
    api: CliNpmRegistryApi,
    initial_snapshot: NpmResolutionSnapshot,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    Self(Arc::new(NpmResolutionInner {
      api,
      snapshot: RwLock::new(initial_snapshot),
      update_queue: Default::default(),
      maybe_lockfile,
    }))
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    let inner = &self.0;

    // only allow one thread in here at a time
    let _permit = inner.update_queue.acquire().await;
    let snapshot = add_package_reqs_to_snapshot(
      &inner.api,
      package_reqs,
      self.0.maybe_lockfile.clone(),
      || inner.snapshot.read().clone(),
    )
    .await?;

    *inner.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    let inner = &self.0;
    // only allow one thread in here at a time
    let _permit = inner.update_queue.acquire().await;

    let reqs_set = package_reqs.iter().cloned().collect::<HashSet<_>>();
    let snapshot = add_package_reqs_to_snapshot(
      &inner.api,
      package_reqs,
      self.0.maybe_lockfile.clone(),
      || {
        let snapshot = inner.snapshot.read().clone();
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

    *inner.snapshot.write() = snapshot;

    Ok(())
  }

  pub async fn resolve_pending(&self) -> Result<(), AnyError> {
    let inner = &self.0;
    // only allow one thread in here at a time
    let _permit = inner.update_queue.acquire().await;

    let snapshot = add_package_reqs_to_snapshot(
      &inner.api,
      Vec::new(),
      self.0.maybe_lockfile.clone(),
      || inner.snapshot.read().clone(),
    )
    .await?;

    *inner.snapshot.write() = snapshot;

    Ok(())
  }

  pub fn pkg_req_ref_to_nv_ref(
    &self,
    req_ref: NpmPackageReqReference,
  ) -> Result<NpmPackageNvReference, PackageReqNotFoundError> {
    let node_id = self.resolve_pkg_id_from_pkg_req(&req_ref.req)?;
    Ok(NpmPackageNvReference {
      nv: node_id.nv,
      sub_path: req_ref.sub_path,
    })
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
  ) -> Result<NpmResolutionPackage, Box<PackageNotFoundFromReferrerError>> {
    self
      .0
      .snapshot
      .read()
      .resolve_package_from_package(name, referrer)
      .cloned()
  }

  /// Resolve a node package from a deno module.
  pub fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &NpmPackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self
      .0
      .snapshot
      .read()
      .resolve_pkg_from_pkg_req(req)
      .map(|pkg| pkg.pkg_id.clone())
  }

  pub fn resolve_pkg_id_from_deno_module(
    &self,
    id: &NpmPackageNv,
  ) -> Result<NpmPackageId, PackageNvNotFoundError> {
    self
      .0
      .snapshot
      .read()
      .resolve_package_from_deno_module(id)
      .map(|pkg| pkg.pkg_id.clone())
  }

  /// Resolves a package requirement for deno graph. This should only be
  /// called by deno_graph's NpmResolver or for resolving packages in
  /// a package.json
  pub fn resolve_package_req_as_pending(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<NpmPackageNv, NpmPackageVersionResolutionError> {
    // we should always have this because it should have been cached before here
    let package_info =
      self.0.api.get_cached_package_info(&pkg_req.name).unwrap();
    self.resolve_package_req_as_pending_with_info(pkg_req, &package_info)
  }

  /// Resolves a package requirement for deno graph. This should only be
  /// called by deno_graph's NpmResolver or for resolving packages in
  /// a package.json
  pub fn resolve_package_req_as_pending_with_info(
    &self,
    pkg_req: &NpmPackageReq,
    package_info: &NpmPackageInfo,
  ) -> Result<NpmPackageNv, NpmPackageVersionResolutionError> {
    debug_assert_eq!(pkg_req.name, package_info.name);
    let inner = &self.0;
    let mut snapshot = inner.snapshot.write();
    let nv = snapshot.resolve_package_req_as_pending(pkg_req, package_info)?;
    Ok(nv)
  }

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    self.0.snapshot.read().all_packages_partitioned()
  }

  pub fn has_packages(&self) -> bool {
    !self.0.snapshot.read().is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.0.snapshot.read().clone()
  }

  pub fn serialized_snapshot(&self) -> SerializedNpmResolutionSnapshot {
    self.0.snapshot.read().as_serialized()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.0.snapshot.read();
    populate_lockfile_from_snapshot(lockfile, &snapshot)
  }
}

async fn add_package_reqs_to_snapshot(
  api: &CliNpmRegistryApi,
  // todo(18079): it should be possible to pass &[NpmPackageReq] in here
  // and avoid all these clones, but the LSP complains because of its
  // `Send` requirement
  package_reqs: Vec<NpmPackageReq>,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  get_new_snapshot: impl Fn() -> NpmResolutionSnapshot,
) -> Result<NpmResolutionSnapshot, AnyError> {
  let snapshot = get_new_snapshot();
  if !snapshot.has_pending()
    && package_reqs
      .iter()
      .all(|req| snapshot.package_reqs().contains_key(req))
  {
    return Ok(snapshot); // already up to date
  }

  let result = snapshot.resolve_pending(package_reqs.clone()).await;
  api.clear_memory_cache();
  let snapshot = match result {
    Ok(snapshot) => snapshot,
    Err(NpmResolutionError::Resolution(err)) if api.mark_force_reload() => {
      log::debug!("{err:#}");
      log::debug!("npm resolution failed. Trying again...");

      // try again
      let snapshot = get_new_snapshot();
      let result = snapshot.resolve_pending(package_reqs).await;
      api.clear_memory_cache();
      // now surface the result after clearing the cache
      result?
    }
    Err(err) => return Err(err.into()),
  };

  if let Some(lockfile_mutex) = maybe_lockfile {
    let mut lockfile = lockfile_mutex.lock();
    populate_lockfile_from_snapshot(&mut lockfile, &snapshot)?;
    Ok(snapshot)
  } else {
    Ok(snapshot)
  }
}

fn populate_lockfile_from_snapshot(
  lockfile: &mut Lockfile,
  snapshot: &NpmResolutionSnapshot,
) -> Result<(), AnyError> {
  for (package_req, nv) in snapshot.package_reqs() {
    lockfile.insert_npm_specifier(
      package_req.to_string(),
      snapshot
        .resolve_package_from_deno_module(nv)
        .unwrap()
        .pkg_id
        .as_serialized(),
    );
  }
  for package in snapshot.all_packages() {
    lockfile
      .check_or_insert_npm_package(npm_package_to_lockfile_info(package))?;
  }
  Ok(())
}

fn npm_package_to_lockfile_info(
  pkg: NpmResolutionPackage,
) -> NpmPackageLockfileInfo {
  let dependencies = pkg
    .dependencies
    .into_iter()
    .map(|(name, id)| NpmPackageDependencyLockfileInfo {
      name,
      id: id.as_serialized(),
    })
    .collect();

  NpmPackageLockfileInfo {
    display_id: pkg.pkg_id.nv.to_string(),
    serialized_id: pkg.pkg_id.as_serialized(),
    integrity: pkg.dist.integrity().to_string(),
    dependencies,
  }
}
