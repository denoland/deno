// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::Path;
use std::path::PathBuf;

use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_semver::package::PackageReq;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

use self::common::NpmPackageFsResolver;
use self::common::NpmPackageFsResolverRc;
use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;
pub use self::resolution::NpmResolution;
pub use self::resolution::NpmResolutionRc;
use crate::sync::new_rc;
use crate::sync::MaybeSend;
use crate::sync::MaybeSync;
use crate::NpmCacheDirRc;
use crate::ResolvedNpmRcRc;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Package folder not found for '{0}'")]
pub struct NpmManagedResolverPackageFolderError(deno_semver::StackString);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolvePkgFolderFromPkgIdError {
  #[class(inherit)]
  #[error(transparent)]
  NpmManagedResolverPackageFolder(#[from] NpmManagedResolverPackageFolderError),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ManagedResolvePkgFolderFromDenoReqError {
  #[class(inherit)]
  #[error(transparent)]
  PackageReqNotFound(#[from] PackageReqNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromPkgId(#[from] ResolvePkgFolderFromPkgIdError),
}

#[allow(clippy::disallowed_types)]
pub type ManagedNpmResolverRc<TSys> =
  crate::sync::MaybeArc<ManagedNpmResolver<TSys>>;

#[derive(Debug)]
pub struct ManagedNpmResolver<TSys: FsCanonicalize> {
  fs_resolver: NpmPackageFsResolverRc,
  resolution: NpmResolutionRc,
  sys: TSys,
}

impl<TSys: FsCanonicalize> ManagedNpmResolver<TSys> {
  pub fn new<
    TCreateSys: FsCanonicalize
      + FsMetadata
      + std::fmt::Debug
      + MaybeSend
      + MaybeSync
      + Clone
      + 'static,
  >(
    npm_cache_dir: &NpmCacheDirRc,
    npm_rc: &ResolvedNpmRcRc,
    resolution: NpmResolutionRc,
    sys: TCreateSys,
    maybe_node_modules_path: Option<PathBuf>,
  ) -> ManagedNpmResolver<TCreateSys> {
    let fs_resolver: NpmPackageFsResolverRc = match maybe_node_modules_path {
      Some(node_modules_folder) => new_rc(LocalNpmPackageResolver::new(
        resolution.clone(),
        sys.clone(),
        node_modules_folder,
      )),
      None => new_rc(GlobalNpmPackageResolver::new(
        npm_cache_dir.clone(),
        npm_rc.clone(),
        resolution.clone(),
      )),
    };

    ManagedNpmResolver {
      fs_resolver,
      sys,
      resolution,
    }
  }

  #[inline]
  pub fn node_modules_path(&self) -> Option<&Path> {
    self.fs_resolver.node_modules_path()
  }

  #[inline]
  pub fn maybe_package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Option<PathBuf> {
    self.fs_resolver.maybe_package_folder(package_id)
  }

  pub fn package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, NpmManagedResolverPackageFolderError> {
    self.maybe_package_folder(package_id).ok_or_else(|| {
      NpmManagedResolverPackageFolderError(package_id.as_serialized())
    })
  }

  pub fn resolve_pkg_folder_from_pkg_id(
    &self,
    pkg_id: &NpmPackageId,
  ) -> Result<PathBuf, ResolvePkgFolderFromPkgIdError> {
    let path = self.package_folder(pkg_id)?;
    let path = canonicalize_path_maybe_not_exists(&self.sys, &path)?;
    log::debug!(
      "Resolved package folder of {} to {}",
      pkg_id.as_serialized(),
      path.display()
    );
    Ok(path)
  }

  pub fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    _referrer: &Url,
  ) -> Result<PathBuf, ManagedResolvePkgFolderFromDenoReqError> {
    let pkg_id = self.resolution.resolve_pkg_id_from_pkg_req(req)?;
    Ok(self.resolve_pkg_folder_from_pkg_id(&pkg_id)?)
  }

  #[inline]
  pub fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error> {
    self
      .fs_resolver
      .resolve_package_cache_folder_id_from_specifier(specifier)
  }
}

impl<TSys: FsCanonicalize + std::fmt::Debug + MaybeSend + MaybeSync>
  NpmPackageFolderResolver for ManagedNpmResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &Url,
  ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
    self
      .fs_resolver
      .resolve_package_folder_from_package(specifier, referrer)
  }
}

#[derive(Debug)]
pub struct ManagedInNpmPackageChecker {
  root_dir: Url,
}

impl InNpmPackageChecker for ManagedInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.as_ref().starts_with(self.root_dir.as_str())
  }
}

pub struct ManagedInNpmPkgCheckerCreateOptions<'a> {
  pub root_cache_dir_url: &'a Url,
  pub maybe_node_modules_path: Option<&'a Path>,
}

pub fn create_managed_in_npm_pkg_checker(
  options: ManagedInNpmPkgCheckerCreateOptions,
) -> ManagedInNpmPackageChecker {
  let root_dir = match options.maybe_node_modules_path {
    Some(node_modules_folder) => {
      deno_path_util::url_from_directory_path(node_modules_folder).unwrap()
    }
    None => options.root_cache_dir_url.clone(),
  };
  debug_assert!(root_dir.as_str().ends_with('/'));
  ManagedInNpmPackageChecker { root_dir }
}
