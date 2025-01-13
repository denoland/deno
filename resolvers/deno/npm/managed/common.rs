// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use node_resolver::NpmPackageFolderResolver;
use url::Url;

use crate::sync::MaybeSend;
use crate::sync::MaybeSync;

#[allow(clippy::disallowed_types)]
pub type NpmPackageFsResolverRc =
  crate::sync::MaybeArc<dyn NpmPackageFsResolver>;

/// Part of the resolution that interacts with the file system.
pub trait NpmPackageFsResolver:
  NpmPackageFolderResolver + MaybeSend + MaybeSync
{
  /// The local node_modules folder if it is applicable to the implementation.
  fn node_modules_path(&self) -> Option<&Path>;

  fn maybe_package_folder(&self, package_id: &NpmPackageId) -> Option<PathBuf>;

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error>;
}
