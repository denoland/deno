// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod common;
mod global;
mod local;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NpmResolver;
use deno_runtime::deno_node::PathClean;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use global::GlobalNpmPackageResolver;
use serde::Deserialize;
use serde::Serialize;

use crate::args::Lockfile;
use crate::util::fs::canonicalize_path_maybe_not_exists_with_fs;
use crate::util::progress_bar::ProgressBar;

use self::local::LocalNpmPackageResolver;
use super::resolution::NpmResolution;
use super::NpmCache;

pub use self::common::NpmPackageFsResolver;

/// State provided to the process via an environment variable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpmProcessState {
  pub snapshot: SerializedNpmResolutionSnapshot,
  pub local_node_modules_path: Option<String>,
}

/// Brings together the npm resolution with the file system.
pub struct CliNpmResolver {
  fs: Arc<dyn FileSystem>,
  fs_resolver: Arc<dyn NpmPackageFsResolver>,
  resolution: Arc<NpmResolution>,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
}

impl std::fmt::Debug for CliNpmResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NpmPackageResolver")
      .field("fs", &"<omitted>")
      .field("fs_resolver", &"<omitted>")
      .field("resolution", &"<omitted>")
      .field("maybe_lockfile", &"<omitted>")
      .finish()
  }
}

impl CliNpmResolver {
  pub fn new(
    fs: Arc<dyn FileSystem>,
    resolution: Arc<NpmResolution>,
    fs_resolver: Arc<dyn NpmPackageFsResolver>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    Self {
      fs,
      fs_resolver,
      resolution,
      maybe_lockfile,
    }
  }

  pub fn root_dir_url(&self) -> &Url {
    self.fs_resolver.root_dir_url()
  }

  pub fn node_modules_path(&self) -> Option<PathBuf> {
    self.fs_resolver.node_modules_path()
  }

  /// Checks if the provided package req's folder is cached.
  pub fn is_pkg_req_folder_cached(&self, req: &PackageReq) -> bool {
    self
      .resolve_pkg_id_from_pkg_req(req)
      .ok()
      .and_then(|id| self.fs_resolver.package_folder(&id).ok())
      .map(|folder| folder.exists())
      .unwrap_or(false)
  }

  pub fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self.resolution.resolve_pkg_id_from_pkg_req(req)
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

  /// Resolve the root folder of the package the provided specifier is in.
  ///
  /// This will error when the provided specifier is not in an npm package.
  pub fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    let Some(path) = self
      .fs_resolver
      .resolve_package_folder_from_specifier(specifier)? else {
        return Ok(None);
      };
    log::debug!(
      "Resolved package folder of {} to {}",
      specifier,
      path.display()
    );
    Ok(Some(path))
  }

  /// Resolves the package nv from the provided specifier.
  pub fn resolve_package_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageId>, AnyError> {
    let Some(cache_folder_id) = self
      .fs_resolver
      .resolve_package_cache_folder_id_from_specifier(specifier)? else {
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

  /// Gets if the provided specifier is in an npm package.
  pub fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    let root_dir_url = self.fs_resolver.root_dir_url();
    debug_assert!(root_dir_url.as_str().ends_with('/'));
    specifier.as_ref().starts_with(root_dir_url.as_str())
  }

  /// If the resolver has resolved any npm packages.
  pub fn has_packages(&self) -> bool {
    self.resolution.has_packages()
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

  /// Gets the state of npm for the process.
  pub fn get_npm_process_state(&self) -> String {
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

  pub fn package_reqs(&self) -> HashMap<PackageReq, PackageNv> {
    self.resolution.package_reqs()
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
}

impl NpmResolver for CliNpmResolver {
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
    path: &Path,
  ) -> Result<Option<PathBuf>, AnyError> {
    let specifier = path_to_specifier(path)?;
    self.resolve_package_folder_from_specifier(&specifier)
  }

  fn resolve_package_folder_from_deno_module(
    &self,
    pkg_nv: &PackageNv,
  ) -> Result<PathBuf, AnyError> {
    let pkg_id = self.resolution.resolve_pkg_id_from_deno_module(pkg_nv)?;
    self.resolve_pkg_folder_from_pkg_id(&pkg_id)
  }

  fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self.resolution.resolve_pkg_id_from_pkg_req(req)
  }

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .resolve_package_folder_from_specifier(specifier)
      .map(|p| p.is_some())
      .unwrap_or(false)
  }

  fn ensure_read_permission(
    &self,
    permissions: &dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    self.fs_resolver.ensure_read_permission(permissions, path)
  }
}

pub fn create_npm_fs_resolver(
  fs: Arc<dyn FileSystem>,
  cache: Arc<NpmCache>,
  progress_bar: &ProgressBar,
  registry_url: Url,
  resolution: Arc<NpmResolution>,
  maybe_node_modules_path: Option<PathBuf>,
  system_info: NpmSystemInfo,
) -> Arc<dyn NpmPackageFsResolver> {
  match maybe_node_modules_path {
    Some(node_modules_folder) => Arc::new(LocalNpmPackageResolver::new(
      fs,
      cache,
      progress_bar.clone(),
      registry_url,
      node_modules_folder,
      resolution,
      system_info,
    )),
    None => Arc::new(GlobalNpmPackageResolver::new(
      fs,
      cache,
      registry_url,
      resolution,
      system_info,
    )),
  }
}

fn path_to_specifier(path: &Path) -> Result<ModuleSpecifier, AnyError> {
  match ModuleSpecifier::from_file_path(path.to_path_buf().clean()) {
    Ok(specifier) => Ok(specifier),
    Err(()) => bail!("Could not convert '{}' to url.", path.display()),
  }
}
