// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use node_resolver::errors::PackageFolderResolveError;
use url::Url;

#[allow(clippy::disallowed_types)]
pub(super) type NpmPackageFsResolverRc =
  crate::sync::MaybeArc<dyn NpmPackageFsResolver>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Package folder not found for '{0}'")]
pub struct NpmPackageFsResolverPackageFolderError(deno_semver::StackString);

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsResolver: Send + Sync {
  /// The local node_modules folder if it is applicable to the implementation.
  fn node_modules_path(&self) -> Option<&Path>;

  fn maybe_package_folder(&self, package_id: &NpmPackageId) -> Option<PathBuf>;

  fn package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, NpmPackageFsResolverPackageFolderError> {
    self.maybe_package_folder(package_id).ok_or_else(|| {
      NpmPackageFsResolverPackageFolderError(package_id.as_serialized())
    })
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &Url,
  ) -> Result<PathBuf, PackageFolderResolveError>;

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error>;
}
