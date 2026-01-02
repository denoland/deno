// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_semver::Version;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPathRef;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

#[derive(Debug)]
pub enum NpmPackageFsResolver<TSys: FsCanonicalize + FsMetadata> {
  Local(super::local::LocalNpmPackageResolver<TSys>),
  Global(super::global::GlobalNpmPackageResolver<TSys>),
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

  fn resolve_types_package_folder(
    &self,
    types_package_name: &str,
    maybe_package_version: Option<&Version>,
    maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Option<PathBuf> {
    match self {
      NpmPackageFsResolver::Local(r) => r.resolve_types_package_folder(
        types_package_name,
        maybe_package_version,
        maybe_referrer,
      ),
      NpmPackageFsResolver::Global(r) => r.resolve_types_package_folder(
        types_package_name,
        maybe_package_version,
        maybe_referrer,
      ),
    }
  }
}

pub fn join_package_name_to_path(path: &Path, package_name: &str) -> PathBuf {
  let mut path = Cow::Borrowed(path);
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = Cow::Owned(path.join(part));
  }
  path.into_owned()
}

pub fn find_definitely_typed_package_from_snapshot<'a>(
  types_package_name: &str,
  maybe_package_version: Option<&Version>,
  snapshot: &'a NpmResolutionSnapshot,
) -> Option<&'a NpmPackageId> {
  let (_, nv) = find_definitely_typed_package(
    types_package_name,
    maybe_package_version,
    snapshot.package_reqs().iter(),
  )?;
  snapshot.resolve_package_id_from_deno_module(nv).ok()
}

/// Attempt to choose the "best" `@types/*` package
/// if possible. If multiple versions exist, try to match
/// the major and minor versions of the `@types` package with the
/// actual package, falling back to the highest @types version present.
pub fn find_definitely_typed_package<'a>(
  types_package_name: &str,
  maybe_package_version: Option<&Version>,
  packages: impl IntoIterator<Item = (&'a PackageReq, &'a PackageNv)>,
) -> Option<(&'a PackageReq, &'a PackageNv)> {
  let mut best_patch = 0;
  let mut highest: Option<(&PackageReq, &PackageNv)> = None;
  let mut best: Option<(&PackageReq, &PackageNv)> = None;

  for (req, type_nv) in packages {
    if type_nv.name != types_package_name {
      continue;
    }
    if let Some(package_version) = maybe_package_version
      && type_nv.version.major == package_version.major
      && type_nv.version.minor == package_version.minor
      && type_nv.version.patch >= best_patch
      && type_nv.version.pre == package_version.pre
    {
      let should_replace = match &best {
        Some((_, best_nv)) => type_nv.version > best_nv.version,
        None => true,
      };
      if should_replace {
        best = Some((req, type_nv));
        best_patch = type_nv.version.patch;
      }
    }

    if let Some((_, highest_nv)) = highest {
      if type_nv.version > highest_nv.version {
        highest = Some((req, type_nv));
      }
    } else {
      highest = Some((req, type_nv));
    }
  }

  best.or(highest)
}
