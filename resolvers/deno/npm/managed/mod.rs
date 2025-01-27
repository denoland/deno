// Copyright 2018-2025 the Deno authors. MIT license.

mod common;
mod global;
mod local;
mod resolution;

use std::path::Path;
use std::path::PathBuf;

use deno_npm::resolution::PackageCacheFolderIdNotFoundError;
use deno_npm::resolution::PackageNvNotFoundError;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmSystemInfo;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPathRef;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

use self::common::NpmPackageFsResolver;
use self::global::GlobalNpmPackageResolver;
use self::local::LocalNpmPackageResolver;
pub use self::resolution::NpmResolutionCell;
pub use self::resolution::NpmResolutionCellRc;
use crate::npmrc::ResolvedNpmRcRc;
use crate::NpmCacheDirRc;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolvePkgFolderFromDenoModuleError {
  #[class(inherit)]
  #[error(transparent)]
  PackageNvNotFound(#[from] PackageNvNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromPkgId(#[from] ResolvePkgFolderFromPkgIdError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error(transparent)]
pub enum ResolvePkgFolderFromPkgIdError {
  #[class(inherit)]
  #[error(transparent)]
  NotFound(#[from] NpmManagedPackageFolderNotFoundError),
  #[class(inherit)]
  #[error(transparent)]
  FailedCanonicalizing(#[from] FailedCanonicalizingError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Package folder not found for '{0}'")]
pub struct NpmManagedPackageFolderNotFoundError(deno_semver::StackString);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Failed canonicalizing '{}'", path.display())]
pub struct FailedCanonicalizingError {
  path: PathBuf,
  #[source]
  source: std::io::Error,
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolvePkgIdFromSpecifierError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  NotFound(#[from] PackageCacheFolderIdNotFoundError),
}

pub struct ManagedNpmResolverCreateOptions<
  TSys: FsCanonicalize + FsMetadata + Clone,
> {
  pub npm_cache_dir: NpmCacheDirRc,
  pub sys: TSys,
  pub maybe_node_modules_path: Option<PathBuf>,
  pub npm_system_info: NpmSystemInfo,
  pub npmrc: ResolvedNpmRcRc,
  pub npm_resolution: NpmResolutionCellRc,
}

#[allow(clippy::disallowed_types)]
pub type ManagedNpmResolverRc<TSys> =
  crate::sync::MaybeArc<ManagedNpmResolver<TSys>>;

#[derive(Debug)]
pub struct ManagedNpmResolver<TSys: FsCanonicalize + FsMetadata> {
  fs_resolver: NpmPackageFsResolver<TSys>,
  npm_cache_dir: NpmCacheDirRc,
  resolution: NpmResolutionCellRc,
  sys: TSys,
}

impl<TSys: FsCanonicalize + FsMetadata> ManagedNpmResolver<TSys> {
  pub fn new<TCreateSys: FsCanonicalize + FsMetadata + Clone>(
    options: ManagedNpmResolverCreateOptions<TCreateSys>,
  ) -> ManagedNpmResolver<TCreateSys> {
    let fs_resolver = match options.maybe_node_modules_path {
      Some(node_modules_folder) => {
        NpmPackageFsResolver::Local(LocalNpmPackageResolver::new(
          options.npm_resolution.clone(),
          options.sys.clone(),
          node_modules_folder,
        ))
      }
      None => NpmPackageFsResolver::Global(GlobalNpmPackageResolver::new(
        options.npm_cache_dir.clone(),
        options.npmrc.clone(),
        options.npm_resolution.clone(),
      )),
    };

    ManagedNpmResolver {
      fs_resolver,
      npm_cache_dir: options.npm_cache_dir,
      sys: options.sys,
      resolution: options.npm_resolution,
    }
  }

  #[inline]
  pub fn root_node_modules_path(&self) -> Option<&Path> {
    self.fs_resolver.node_modules_path()
  }

  pub fn global_cache_root_path(&self) -> &Path {
    self.npm_cache_dir.root_dir()
  }

  pub fn global_cache_root_url(&self) -> &Url {
    self.npm_cache_dir.root_dir_url()
  }

  pub fn resolution(&self) -> &NpmResolutionCell {
    self.resolution.as_ref()
  }

  /// Checks if the provided package req's folder is cached.
  pub fn is_pkg_req_folder_cached(&self, req: &PackageReq) -> bool {
    self
      .resolution
      .resolve_pkg_id_from_pkg_req(req)
      .ok()
      .and_then(|id| self.resolve_pkg_folder_from_pkg_id(&id).ok())
      .map(|folder| self.sys.fs_exists_no_err(folder))
      .unwrap_or(false)
  }

  pub fn resolve_pkg_folder_from_pkg_id(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, ResolvePkgFolderFromPkgIdError> {
    let path = self
      .fs_resolver
      .maybe_package_folder(package_id)
      .ok_or_else(|| {
        NpmManagedPackageFolderNotFoundError(package_id.as_serialized())
      })?;
    // todo(dsherret): investigate if this canonicalization is always
    // necessary. For example, maybe it's not necessary for the global cache
    let path = canonicalize_path_maybe_not_exists(&self.sys, &path).map_err(
      |source| FailedCanonicalizingError {
        path: path.to_path_buf(),
        source,
      },
    )?;
    log::debug!(
      "Resolved package folder of {} to {}",
      package_id.as_serialized(),
      path.display()
    );
    Ok(path)
  }

  pub fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoModuleError> {
    let pkg_id = self.resolution.resolve_pkg_id_from_deno_module(nv)?;
    Ok(self.resolve_pkg_folder_from_pkg_id(&pkg_id)?)
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

  /// Resolves the package id from the provided specifier.
  pub fn resolve_pkg_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageId>, ResolvePkgIdFromSpecifierError> {
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
}

impl<TSys: FsCanonicalize + FsMetadata> NpmPackageFolderResolver
  for ManagedNpmResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
    let path = self
      .fs_resolver
      .resolve_package_folder_from_package(specifier, referrer)?;
    log::debug!(
      "Resolved {} from {} to {}",
      specifier,
      referrer.display(),
      path.display()
    );
    Ok(path)
  }
}

#[derive(Debug, Clone)]
pub struct ManagedInNpmPackageChecker {
  root_dir: Url,
}

impl InNpmPackageChecker for ManagedInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.as_ref().starts_with(self.root_dir.as_str())
  }
}

#[derive(Debug)]
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
