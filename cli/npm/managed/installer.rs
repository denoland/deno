// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use capacity_builder::StringBuilder;
use deno_core::error::AnyError;
use deno_error::JsErrorBox;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmResolutionPackage;
use deno_resolver::npm::managed::NpmResolution;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::SmallStackString;
use deno_semver::VersionReq;

use crate::args::CliLockfile;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::util::sync::TaskQueue;

pub struct AddPkgReqsResult {
  /// Results from adding the individual packages.
  ///
  /// The indexes of the results correspond to the indexes of the provided
  /// package requirements.
  pub results: Vec<Result<PackageNv, NpmResolutionError>>,
  /// The final result of resolving and caching all the package requirements.
  pub dependencies_result: Result<(), JsErrorBox>,
}

/// Updates the npm resolution with the provided package requirements.
pub struct NpmResolutionInstaller {
  registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
  resolution: Arc<NpmResolution>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  update_queue: TaskQueue,
}

impl NpmResolutionInstaller {
  pub fn new(
    registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
    resolution: Arc<NpmResolution>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
  ) -> Self {
    Self {
      registry_info_provider,
      resolution,
      maybe_lockfile,
      update_queue: Default::default(),
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> AddPkgReqsResult {
    // only allow one thread in here at a time
    let _snapshot_lock = self.update_queue.acquire().await;
    let result = add_package_reqs_to_snapshot(
      &self.registry_info_provider,
      package_reqs,
      self.maybe_lockfile.clone(),
      || self.resolution.snapshot(),
    )
    .await;

    AddPkgReqsResult {
      results: result.results,
      dependencies_result: match result.dep_graph_result {
        Ok(snapshot) => {
          self.resolution.set_snapshot(snapshot);
          Ok(())
        }
        Err(err) => Err(JsErrorBox::from_err(err)),
      },
    }
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: &[PackageReq],
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _snapshot_lock = self.update_queue.acquire().await;

    let reqs_set = package_reqs.iter().collect::<HashSet<_>>();
    let snapshot = add_package_reqs_to_snapshot(
      &self.registry_info_provider,
      package_reqs,
      self.maybe_lockfile.clone(),
      || {
        let snapshot = self.resolution.snapshot();
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
    .await
    .into_result()?;

    self.resolution.set_snapshot(snapshot);

    Ok(())
  }
}

async fn add_package_reqs_to_snapshot(
  registry_info_provider: &Arc<CliNpmRegistryInfoProvider>,
  package_reqs: &[PackageReq],
  maybe_lockfile: Option<Arc<CliLockfile>>,
  get_new_snapshot: impl Fn() -> NpmResolutionSnapshot,
) -> deno_npm::resolution::AddPkgReqsResult {
  let snapshot = get_new_snapshot();
  if package_reqs
    .iter()
    .all(|req| snapshot.package_reqs().contains_key(req))
  {
    log::debug!("Snapshot already up to date. Skipping npm resolution.");
    return deno_npm::resolution::AddPkgReqsResult {
      results: package_reqs
        .iter()
        .map(|req| Ok(snapshot.package_reqs().get(req).unwrap().clone()))
        .collect(),
      dep_graph_result: Ok(snapshot),
    };
  }
  log::debug!(
    /* this string is used in tests */
    "Running npm resolution."
  );
  let npm_registry_api = registry_info_provider.as_npm_registry_api();
  let result = snapshot
    .add_pkg_reqs(&npm_registry_api, get_add_pkg_reqs_options(package_reqs))
    .await;
  let result = match &result.dep_graph_result {
    Err(NpmResolutionError::Resolution(err))
      if npm_registry_api.mark_force_reload() =>
    {
      log::debug!("{err:#}");
      log::debug!("npm resolution failed. Trying again...");

      // try again with forced reloading
      let snapshot = get_new_snapshot();
      snapshot
        .add_pkg_reqs(&npm_registry_api, get_add_pkg_reqs_options(package_reqs))
        .await
    }
    _ => result,
  };

  registry_info_provider.clear_memory_cache();

  if let Ok(snapshot) = &result.dep_graph_result {
    if let Some(lockfile) = maybe_lockfile {
      populate_lockfile_from_snapshot(&lockfile, snapshot);
    }
  }

  result
}

fn get_add_pkg_reqs_options(package_reqs: &[PackageReq]) -> AddPkgReqsOptions {
  AddPkgReqsOptions {
    package_reqs,
    // WARNING: When bumping this version, check if anything needs to be
    // updated in the `setNodeOnlyGlobalNames` call in 99_main_compiler.js
    types_node_version_req: Some(
      VersionReq::parse_from_npm("22.0.0 - 22.5.4").unwrap(),
    ),
  }
}

fn populate_lockfile_from_snapshot(
  lockfile: &CliLockfile,
  snapshot: &NpmResolutionSnapshot,
) {
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
      serialized_id: pkg.id.as_serialized(),
      integrity: pkg.dist.integrity().for_lockfile(),
      dependencies,
    }
  }

  let mut lockfile = lockfile.lock();
  for (package_req, nv) in snapshot.package_reqs() {
    let id = &snapshot.resolve_package_from_deno_module(nv).unwrap().id;
    lockfile.insert_package_specifier(
      JsrDepPackageReq::npm(package_req.clone()),
      {
        StringBuilder::<SmallStackString>::build(|builder| {
          builder.append(&id.nv.version);
          builder.append(&id.peer_dependencies);
        })
        .unwrap()
      },
    );
  }
  for package in snapshot.all_packages_for_every_system() {
    lockfile.insert_npm_package(npm_package_to_lockfile_info(package));
  }
}
