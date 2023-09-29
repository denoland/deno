// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::NpmPackageReqResolution;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NpmResolver;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageNvReference;
use deno_semver::package::PackageReq;
use serde::Deserialize;
use serde::Serialize;

use crate::args::Lockfile;
use crate::util::fs::canonicalize_path_maybe_not_exists_with_fs;

use super::CliNpmRegistryApi;
use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;

pub use self::installer::PackageJsonDepsInstaller;
pub use self::resolution::NpmResolution;
pub use self::resolvers::create_npm_fs_resolver;
pub use self::resolvers::NpmPackageFsResolver;

mod installer;
mod resolution;
mod resolvers;

/// State provided to the process via an environment variable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub snapshot: SerializedNpmResolutionSnapshot,
  pub local_node_modules_path: Option<String>,
}

/// An npm resolver where the resolution is managed by Deno rather than
/// the user bringing their own node_modules (BYONM) on the file system.
pub struct ManagedCliNpmResolver {
  api: Arc<CliNpmRegistryApi>,
  fs: Arc<dyn FileSystem>,
  fs_resolver: Arc<dyn NpmPackageFsResolver>,
  resolution: Arc<NpmResolution>,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  package_json_deps_installer: Arc<PackageJsonDepsInstaller>,
}

impl std::fmt::Debug for ManagedCliNpmResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ManagedNpmResolver")
      .field("api", &"<omitted>")
      .field("fs", &"<omitted>")
      .field("fs_resolver", &"<omitted>")
      .field("resolution", &"<omitted>")
      .field("maybe_lockfile", &"<omitted>")
      .field("package_json_deps_installer", &"<omitted>")
      .finish()
  }
}

impl ManagedCliNpmResolver {
  pub fn new(
    api: Arc<CliNpmRegistryApi>,
    fs: Arc<dyn FileSystem>,
    resolution: Arc<NpmResolution>,
    fs_resolver: Arc<dyn NpmPackageFsResolver>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
    package_json_deps_installer: Arc<PackageJsonDepsInstaller>,
  ) -> Self {
    Self {
      api,
      fs,
      fs_resolver,
      resolution,
      maybe_lockfile,
      package_json_deps_installer,
    }
  }

  pub fn resolve_pkg_folder_from_pkg_id(
    &self,
    pkg_id: &NpmPackageId,
  ) -> Result<PathBuf, AnyError> {
    let path = self.fs_resolver.package_folder(pkg_id)?;
    let path = canonicalize_path_maybe_not_exists_with_fs(&path, |path| {
      self
        .fs
        .realpath_sync(path)
        .map_err(|err| err.into_io_error())
    })?;
    log::debug!(
      "Resolved package folder of {} to {}",
      pkg_id.as_serialized(),
      path.display()
    );
    Ok(path)
  }

  /// Resolves the package nv from the provided specifier.
  pub fn resolve_pkg_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageId>, AnyError> {
    let Some(cache_folder_id) = self
      .fs_resolver
      .resolve_package_cache_folder_id_from_specifier(specifier)?
    else {
      return Ok(None);
    };
    Ok(Some(
      self
        .resolution
        .resolve_pkg_id_from_pkg_cache_folder_id(&cache_folder_id)?,
    ))
  }

  /// Attempts to get the package size in bytes.
  pub fn package_size(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<u64, AnyError> {
    let package_folder = self.fs_resolver.package_folder(package_id)?;
    Ok(crate::util::fs::dir_size(&package_folder)?)
  }

  pub fn all_system_packages(
    &self,
    system_info: &NpmSystemInfo,
  ) -> Vec<NpmResolutionPackage> {
    self.resolution.all_system_packages(system_info)
  }

  /// Adds package requirements to the resolver and ensures everything is setup.
  pub async fn add_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), AnyError> {
    if packages.is_empty() {
      return Ok(());
    }

    self.resolution.add_package_reqs(packages).await?;
    self.fs_resolver.cache_packages().await?;

    // If there's a lock file, update it with all discovered npm packages
    if let Some(lockfile_mutex) = &self.maybe_lockfile {
      let mut lockfile = lockfile_mutex.lock();
      self.lock(&mut lockfile)?;
    }

    Ok(())
  }

  /// Sets package requirements to the resolver, removing old requirements and adding new ones.
  ///
  /// This will retrieve and resolve package information, but not cache any package files.
  pub async fn set_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), AnyError> {
    self.resolution.set_package_reqs(packages).await
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.resolution.snapshot()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    self.resolution.lock(lockfile)
  }

  pub async fn inject_synthetic_types_node_package(
    &self,
  ) -> Result<(), AnyError> {
    // add and ensure this isn't added to the lockfile
    let package_reqs = vec![PackageReq::from_str("@types/node").unwrap()];
    self.resolution.add_package_reqs(&package_reqs).await?;
    self.fs_resolver.cache_packages().await?;

    Ok(())
  }

  pub async fn resolve_pending(&self) -> Result<(), AnyError> {
    self.resolution.resolve_pending().await?;
    self.fs_resolver.cache_packages().await?;
    Ok(())
  }

  fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self.resolution.resolve_pkg_id_from_pkg_req(req)
  }

  pub async fn ensure_top_level_package_json_install(
    &self,
  ) -> Result<(), AnyError> {
    self
      .package_json_deps_installer
      .ensure_top_level_install()
      .await
  }

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<(), AnyError> {
    // this will internally cache the package information
    self
      .api
      .package_info(&package_name)
      .await
      .map(|_| ())
      .map_err(|err| err.into())
  }
}

impl NpmResolver for ManagedCliNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError> {
    let path = self
      .fs_resolver
      .resolve_package_folder_from_package(name, referrer, mode)?;
    log::debug!("Resolved {} from {} to {}", name, referrer, path.display());
    Ok(path)
  }

  fn resolve_package_folder_from_path(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    self.resolve_pkg_folder_from_specifier(specifier)
  }

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    let root_dir_url = self.fs_resolver.root_dir_url();
    debug_assert!(root_dir_url.as_str().ends_with('/'));
    specifier.as_ref().starts_with(root_dir_url.as_str())
  }

  fn ensure_read_permission(
    &self,
    permissions: &dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    self.fs_resolver.ensure_read_permission(permissions, path)
  }
}

impl CliNpmResolver for ManagedCliNpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver> {
    self
  }

  fn root_dir_url(&self) -> &Url {
    self.fs_resolver.root_dir_url()
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Managed(self)
  }

  fn node_modules_path(&self) -> Option<PathBuf> {
    self.fs_resolver.node_modules_path()
  }

  /// Checks if the provided package req's folder is cached.
  fn is_pkg_req_folder_cached(&self, req: &PackageReq) -> bool {
    self
      .resolve_pkg_id_from_pkg_req(req)
      .ok()
      .and_then(|id| self.fs_resolver.package_folder(&id).ok())
      .map(|folder| folder.exists())
      .unwrap_or(false)
  }

  fn resolve_npm_for_deno_graph(
    &self,
    pkg_req: &PackageReq,
  ) -> NpmPackageReqResolution {
    let result = self.resolution.resolve_pkg_req_as_pending(&pkg_req);
    match result {
      Ok(nv) => NpmPackageReqResolution::Ok(nv),
      Err(err) => {
        if self.api.mark_force_reload() {
          log::debug!("Restarting npm specifier resolution to check for new registry information. Error: {:#}", err);
          NpmPackageReqResolution::ReloadRegistryInfo(err.into())
        } else {
          NpmPackageReqResolution::Err(err.into())
        }
      }
    }
  }

  fn resolve_pkg_nv_ref_from_pkg_req_ref(
    &self,
    req_ref: &NpmPackageReqReference,
  ) -> Result<NpmPackageNvReference, PackageReqNotFoundError> {
    let pkg_nv = self
      .resolve_pkg_id_from_pkg_req(req_ref.req())
      .map(|id| id.nv)?;
    Ok(NpmPackageNvReference::new(PackageNvReference {
      nv: pkg_nv,
      sub_path: req_ref.sub_path().map(|s| s.to_string()),
    }))
  }

  /// Resolve the root folder of the package the provided specifier is in.
  ///
  /// This will error when the provided specifier is not in an npm package.
  fn resolve_pkg_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    let Some(path) = self
      .fs_resolver
      .resolve_package_folder_from_specifier(specifier)?
    else {
      return Ok(None);
    };
    log::debug!(
      "Resolved package folder of {} to {}",
      specifier,
      path.display()
    );
    Ok(Some(path))
  }

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
  ) -> Result<PathBuf, AnyError> {
    let pkg_id = self.resolve_pkg_id_from_pkg_req(req)?;
    self.resolve_pkg_folder_from_pkg_id(&pkg_id)
  }

  fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, AnyError> {
    let pkg_id = self.resolution.resolve_pkg_id_from_deno_module(nv)?;
    self.resolve_pkg_folder_from_pkg_id(&pkg_id)
  }

  /// Gets the state of npm for the process.
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      snapshot: self
        .resolution
        .serialized_valid_snapshot()
        .into_serialized(),
      local_node_modules_path: self
        .fs_resolver
        .node_modules_path()
        .map(|p| p.to_string_lossy().to_string()),
    })
    .unwrap()
  }

  fn package_reqs(&self) -> HashMap<PackageReq, PackageNv> {
    self.resolution.package_reqs()
  }
}
