// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for global npm cache resolution.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm_cache::NpmCache;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;
use url::Url;

use super::NpmPackageFsResolver;
use super::NpmResolution;

/// Resolves packages from the global npm cache.
#[derive(Debug)]
pub struct GlobalNpmPackageResolver<
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom
    + Send
    + Sync,
> {
  cache: Arc<NpmCache<TSys>>,
  resolution: Arc<NpmResolution>,
}

impl<
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom
      + Send
      + Sync,
  > GlobalNpmPackageResolver<TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    resolution: Arc<NpmResolution>,
  ) -> Self {
    Self { cache, resolution }
  }
}

#[async_trait(?Send)]
impl<
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom
      + Send
      + Sync,
  > NpmPackageFsResolver for GlobalNpmPackageResolver<TSys>
{
  fn node_modules_path(&self) -> Option<&Path> {
    None
  }

  fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let folder_id = self
      .resolution
      .resolve_pkg_cache_folder_id_from_pkg_id(id)?;
    Some(self.cache.package_folder_for_id(&folder_id))
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &Url,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    use deno_npm::resolution::PackageNotFoundFromReferrerError;
    let Some(referrer_cache_folder_id) = self
      .cache
      .resolve_package_folder_id_from_specifier(referrer)
    else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.clone(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    let resolve_result = self
      .resolution
      .resolve_package_from_package(name, &referrer_cache_folder_id);
    match resolve_result {
      Ok(pkg) => match self.maybe_package_folder(&pkg.id) {
        Some(folder) => Ok(folder),
        None => Err(
          PackageNotFoundError {
            package_name: name.to_string(),
            referrer: referrer.clone(),
            referrer_extra: Some(format!(
              "{} -> {}",
              referrer_cache_folder_id,
              pkg.id.as_serialized()
            )),
          }
          .into(),
        ),
      },
      Err(err) => match *err {
        PackageNotFoundFromReferrerError::Referrer(cache_folder_id) => Err(
          ReferrerNotFoundError {
            referrer: referrer.clone(),
            referrer_extra: Some(cache_folder_id.to_string()),
          }
          .into(),
        ),
        PackageNotFoundFromReferrerError::Package {
          name,
          referrer: cache_folder_id_referrer,
        } => Err(
          PackageNotFoundError {
            package_name: name,
            referrer: referrer.clone(),
            referrer_extra: Some(cache_folder_id_referrer.to_string()),
          }
          .into(),
        ),
      },
    }
  }

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error> {
    Ok(
      self
        .cache
        .resolve_package_folder_id_from_specifier(specifier),
    )
  }
}
