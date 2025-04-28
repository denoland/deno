// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::unsync::sync::AtomicFlag;
use deno_error::JsErrorBox;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::NpmSystemInfo;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_runtime::colors;
use deno_semver::package::PackageReq;
use rustc_hash::FxHashSet;

pub use self::common::NpmPackageFsInstaller;
use self::global::GlobalNpmPackageInstaller;
use self::local::LocalNpmPackageInstaller;
pub use self::resolution::AddPkgReqsResult;
pub use self::resolution::NpmResolutionInstaller;
use super::NpmResolutionInitializer;
use super::WorkspaceNpmPatchPackages;
use crate::args::CliLockfile;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmInstallDepsProvider;
use crate::args::PackageJsonDepValueParseWithLocationError;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmTarballCache;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

mod common;
mod global;
mod local;
mod resolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCaching<'a> {
  Only(Cow<'a, [PackageReq]>),
  All,
}

#[derive(Debug)]
pub struct NpmInstaller {
  fs_installer: Arc<dyn NpmPackageFsInstaller>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  npm_resolution_initializer: Arc<NpmResolutionInitializer>,
  npm_resolution_installer: Arc<NpmResolutionInstaller>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  npm_resolution: Arc<NpmResolutionCell>,
  top_level_install_flag: AtomicFlag,
  cached_reqs: tokio::sync::Mutex<FxHashSet<PackageReq>>,
}

impl NpmInstaller {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    npm_cache: Arc<CliNpmCache>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    npm_registry_info_provider: Arc<
      dyn deno_npm::registry::NpmRegistryApi + Send + Sync,
    >,
    npm_resolution: Arc<NpmResolutionCell>,
    npm_resolution_initializer: Arc<NpmResolutionInitializer>,
    npm_resolution_installer: Arc<NpmResolutionInstaller>,
    progress_bar: &ProgressBar,
    sys: CliSys,
    tarball_cache: Arc<CliNpmTarballCache>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    maybe_node_modules_path: Option<PathBuf>,
    lifecycle_scripts: LifecycleScriptsConfig,
    system_info: NpmSystemInfo,
    workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
  ) -> Self {
    let fs_installer: Arc<dyn NpmPackageFsInstaller> =
      match maybe_node_modules_path {
        Some(node_modules_folder) => Arc::new(LocalNpmPackageInstaller::new(
          npm_cache,
          npm_install_deps_provider.clone(),
          progress_bar.clone(),
          npm_resolution.clone(),
          sys,
          tarball_cache,
          node_modules_folder,
          lifecycle_scripts,
          system_info,
          npm_registry_info_provider,
          workspace_patch_packages,
        )),
        None => Arc::new(GlobalNpmPackageInstaller::new(
          npm_cache,
          tarball_cache,
          npm_resolution.clone(),
          lifecycle_scripts,
          system_info,
        )),
      };
    Self {
      fs_installer,
      npm_install_deps_provider,
      npm_resolution,
      npm_resolution_initializer,
      npm_resolution_installer,
      maybe_lockfile,
      top_level_install_flag: Default::default(),
      cached_reqs: Default::default(),
    }
  }

  /// Adds package requirements to the resolver and ensures everything is setup.
  /// This includes setting up the `node_modules` directory, if applicable.
  pub async fn add_and_cache_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .add_package_reqs_raw(
        packages,
        Some(PackageCaching::Only(packages.into())),
      )
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs_no_cache(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .add_package_reqs_raw(packages, None)
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs(
    &self,
    packages: &[PackageReq],
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    self
      .add_package_reqs_raw(packages, Some(caching))
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs_raw(
    &self,
    packages: &[PackageReq],
    caching: Option<PackageCaching<'_>>,
  ) -> AddPkgReqsResult {
    if packages.is_empty() {
      return AddPkgReqsResult {
        dependencies_result: Ok(()),
        results: vec![],
      };
    }

    #[cfg(debug_assertions)]
    self.npm_resolution_initializer.debug_assert_initialized();

    let mut result = self
      .npm_resolution_installer
      .add_package_reqs(packages)
      .await;

    if result.dependencies_result.is_ok() {
      if let Some(lockfile) = self.maybe_lockfile.as_ref() {
        result.dependencies_result = lockfile.error_if_changed();
      }
    }
    if result.dependencies_result.is_ok() {
      if let Some(caching) = caching {
        // the async mutex is unfortunate, but needed to handle the edge case where two workers
        // try to cache the same package at the same time. we need to hold the lock while we cache
        // and since that crosses an await point, we need the async mutex.
        //
        // should have a negligible perf impact because acquiring the lock is still in the order of nanoseconds
        // while caching typically takes micro or milli seconds.
        let mut cached_reqs = self.cached_reqs.lock().await;
        let uncached = {
          packages
            .iter()
            .filter(|req| !cached_reqs.contains(req))
            .collect::<Vec<_>>()
        };

        if !uncached.is_empty() {
          result.dependencies_result = self.cache_packages(caching).await;
          if result.dependencies_result.is_ok() {
            for req in uncached {
              cached_reqs.insert(req.clone());
            }
          }
        }
      }
    }

    result
  }

  /// Sets package requirements to the resolver, removing old requirements and adding new ones.
  ///
  /// This will retrieve and resolve package information, but not cache any package files.
  pub async fn set_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), AnyError> {
    self
      .npm_resolution_installer
      .set_package_reqs(packages)
      .await
  }

  pub async fn inject_synthetic_types_node_package(
    &self,
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    let reqs = &[PackageReq::from_str("@types/node").unwrap()];
    // add and ensure this isn't added to the lockfile
    self
      .add_package_reqs(reqs, PackageCaching::Only(reqs.into()))
      .await?;

    Ok(())
  }

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    self
      .npm_resolution_installer
      .cache_package_info(package_name)
      .await
  }

  pub async fn cache_packages(
    &self,
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self.fs_installer.cache_packages(caching).await
  }

  pub fn ensure_no_pkg_json_dep_errors(
    &self,
  ) -> Result<(), Box<PackageJsonDepValueParseWithLocationError>> {
    for err in self.npm_install_deps_provider.pkg_json_dep_errors() {
      match err.source.as_kind() {
        deno_package_json::PackageJsonDepValueParseErrorKind::VersionReq(_) => {
          return Err(Box::new(err.clone()));
        }
        deno_package_json::PackageJsonDepValueParseErrorKind::Unsupported {
          ..
        } => {
          // only warn for this one
          log::warn!(
            "{} {}\n    at {}",
            colors::yellow("Warning"),
            err.source,
            err.location,
          )
        }
      }
    }
    Ok(())
  }

  /// Ensures that the top level `package.json` dependencies are installed.
  /// This may set up the `node_modules` directory.
  ///
  /// Returns `true` if the top level packages are already installed. A
  /// return value of `false` means that new packages were added to the NPM resolution.
  pub async fn ensure_top_level_package_json_install(
    &self,
  ) -> Result<bool, JsErrorBox> {
    if !self.top_level_install_flag.raise() {
      return Ok(true); // already did this
    }

    self.npm_resolution_initializer.ensure_initialized().await?;

    let pkg_json_remote_pkgs = self.npm_install_deps_provider.remote_pkgs();
    if pkg_json_remote_pkgs.is_empty() {
      return Ok(true);
    }

    // check if something needs resolving before bothering to load all
    // the package information (which is slow)
    if pkg_json_remote_pkgs.iter().all(|pkg| {
      self
        .npm_resolution
        .resolve_pkg_id_from_pkg_req(&pkg.req)
        .is_ok()
    }) {
      log::debug!(
        "All package.json deps resolvable. Skipping top level install."
      );
      return Ok(true); // everything is already resolvable
    }

    let pkg_reqs = pkg_json_remote_pkgs
      .iter()
      .map(|pkg| pkg.req.clone())
      .collect::<Vec<_>>();
    self.add_package_reqs_no_cache(&pkg_reqs).await?;

    Ok(false)
  }
}
