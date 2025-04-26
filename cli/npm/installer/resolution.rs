// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use capacity_builder::StringBuilder;
use deno_core::error::AnyError;
use deno_error::JsErrorBox;
use deno_lockfile::NpmPackageDependencyLockfileInfo;
use deno_lockfile::NpmPackageLockfileInfo;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::DefaultTarballUrlProvider;
use deno_npm::resolution::NpmResolutionError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmResolutionPackage;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_runtime::colors;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::VersionReq;

use crate::args::CliLockfile;
use crate::npm::CliNpmRegistryInfoProvider;
use crate::npm::WorkspaceNpmPatchPackages;
use crate::util::display::DisplayTreeNode;
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
#[derive(Debug)]
pub struct NpmResolutionInstaller {
  registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
  resolution: Arc<NpmResolutionCell>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  patch_packages: Arc<WorkspaceNpmPatchPackages>,
  update_queue: TaskQueue,
}

impl NpmResolutionInstaller {
  pub fn new(
    registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
    resolution: Arc<NpmResolutionCell>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    patch_packages: Arc<WorkspaceNpmPatchPackages>,
  ) -> Self {
    Self {
      registry_info_provider,
      resolution,
      maybe_lockfile,
      patch_packages,
      update_queue: Default::default(),
    }
  }

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    // this will internally cache the package information
    self.registry_info_provider.package_info(package_name).await
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
      &self.patch_packages,
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
      &self.patch_packages,
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
  patch_packages: &WorkspaceNpmPatchPackages,
  get_new_snapshot: impl Fn() -> NpmResolutionSnapshot,
) -> deno_npm::resolution::AddPkgReqsResult {
  fn get_types_node_version() -> VersionReq {
    // WARNING: When bumping this version, check if anything needs to be
    // updated in the `setNodeOnlyGlobalNames` call in 99_main_compiler.js
    VersionReq::parse_from_npm("22.9.0 - 22.12.0").unwrap()
  }

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
      unmet_peer_diagnostics: Default::default(),
    };
  }
  log::debug!(
    /* this string is used in tests */
    "Running npm resolution."
  );
  let npm_registry_api = registry_info_provider.as_npm_registry_api();
  let result = snapshot
    .add_pkg_reqs(
      &npm_registry_api,
      AddPkgReqsOptions {
        package_reqs,
        types_node_version_req: Some(get_types_node_version()),
        patch_packages: &patch_packages.0,
      },
    )
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
        .add_pkg_reqs(
          &npm_registry_api,
          AddPkgReqsOptions {
            package_reqs,
            types_node_version_req: Some(get_types_node_version()),
            patch_packages: &patch_packages.0,
          },
        )
        .await
    }
    _ => result,
  };

  registry_info_provider.clear_memory_cache();

  if !result.unmet_peer_diagnostics.is_empty()
    && log::log_enabled!(log::Level::Warn)
  {
    let root_node = DisplayTreeNode {
      text: format!(
        "{} The following peer dependency issues were found:",
        colors::yellow("Warning")
      ),
      children: result
        .unmet_peer_diagnostics
        .iter()
        .map(|diagnostic| {
          let mut node = DisplayTreeNode {
            text: format!(
              "peer {}: resolved to {}",
              diagnostic.dependency, diagnostic.resolved
            ),
            children: Vec::new(),
          };
          for ancestor in &diagnostic.ancestors {
            node = DisplayTreeNode {
              text: ancestor.to_string(),
              children: vec![node],
            };
          }
          node
        })
        .collect(),
    };
    let mut text = String::new();
    _ = root_node.print(&mut text);
    log::warn!("{}", text);
  }

  if let Ok(snapshot) = &result.dep_graph_result {
    if let Some(lockfile) = maybe_lockfile {
      populate_lockfile_from_snapshot(&lockfile, snapshot);
    }
  }

  result
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
      .filter_map(|(name, id)| {
        if pkg.optional_dependencies.contains(name) {
          None
        } else {
          Some(NpmPackageDependencyLockfileInfo {
            name: name.clone(),
            id: id.as_serialized(),
          })
        }
      })
      .collect();

    let optional_dependencies = pkg
      .optional_dependencies
      .iter()
      .filter_map(|name| {
        let id = pkg.dependencies.get(name)?;
        Some(NpmPackageDependencyLockfileInfo {
          name: name.clone(),
          id: id.as_serialized(),
        })
      })
      .collect();

    let optional_peers = pkg
      .optional_peer_dependencies
      .iter()
      .filter_map(|name| {
        let id = pkg.dependencies.get(name)?;
        Some(NpmPackageDependencyLockfileInfo {
          name: name.clone(),
          id: id.as_serialized(),
        })
      })
      .collect();
    NpmPackageLockfileInfo {
      serialized_id: pkg.id.as_serialized(),
      integrity: pkg.dist.as_ref().and_then(|dist| {
        dist.integrity().for_lockfile().map(|s| s.into_owned())
      }),
      dependencies,
      optional_dependencies,
      os: pkg.system.os.clone(),
      cpu: pkg.system.cpu.clone(),
      tarball: pkg.dist.as_ref().and_then(|dist| {
        // Omit the tarball URL if it's the standard NPM registry URL
        if dist.tarball
          == crate::npm::managed::DefaultTarballUrl::default_tarball_url(
            &crate::npm::managed::DefaultTarballUrl,
            &pkg.id.nv,
          )
        {
          None
        } else {
          Some(StackString::from_str(&dist.tarball))
        }
      }),
      deprecated: pkg.is_deprecated,
      has_bin: pkg.has_bin,
      has_scripts: pkg.has_scripts,
      optional_peers,
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
