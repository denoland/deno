// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPathRef;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

#[derive(Debug)]
pub enum NpmPackageFsResolver<TSys: FsCanonicalize + FsMetadata> {
  Local(super::local::LocalNpmPackageResolver<TSys>),
  Global(super::global::GlobalNpmPackageResolver),
}

impl<TSys: FsCanonicalize + FsMetadata> NpmPackageFsResolver<TSys> {
  /// The local node_modules folder (only for the local resolver).
  pub fn node_modules_path(&self) -> Option<&Path> {
    match self {
      NpmPackageFsResolver::Local(resolver) => resolver.node_modules_path(),
      NpmPackageFsResolver::Global(_) => None,
    }
  }

  pub fn maybe_package_folder(
    &self,
    package_id: &NpmPackageId,
  ) -> Option<PathBuf> {
    match self {
      NpmPackageFsResolver::Local(resolver) => {
        resolver.maybe_package_folder(package_id)
      }
      NpmPackageFsResolver::Global(resolver) => {
        resolver.maybe_package_folder(package_id)
      }
    }
  }

  pub fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error> {
    match self {
      NpmPackageFsResolver::Local(resolver) => {
        resolver.resolve_package_cache_folder_id_from_specifier(specifier)
      }
      NpmPackageFsResolver::Global(resolver) => {
        resolver.resolve_package_cache_folder_id_from_specifier(specifier)
      }
    }
  }
}

impl<TSys: FsCanonicalize + FsMetadata> NpmPackageFolderResolver
  for NpmPackageFsResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
    match self {
      NpmPackageFsResolver::Local(r) => {
        r.resolve_package_folder_from_package(specifier, referrer)
      }
      NpmPackageFsResolver::Global(r) => {
        r.resolve_package_folder_from_package(specifier, referrer)
      }
    }
  }
}
