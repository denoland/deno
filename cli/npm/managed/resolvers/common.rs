// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod bin_entries;
pub mod lifecycle_scripts;

use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use node_resolver::errors::PackageFolderResolveError;

use super::super::PackageCaching;

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsResolver: Send + Sync {
  /// The local node_modules folder if it is applicable to the implementation.
  fn node_modules_path(&self) -> Option<&Path>;

  fn maybe_package_folder(&self, package_id: &NpmPackageId) -> Option<PathBuf>;

  fn package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<PathBuf, AnyError> {
    self.maybe_package_folder(package_id).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "Package folder not found for '{}'",
        package_id.as_serialized()
      )
    })
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError>;

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageCacheFolderId>, AnyError>;

  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), AnyError>;
}
