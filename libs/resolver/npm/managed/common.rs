// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_semver::Version;
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

/// Attempt to choose the "best" `@types/*` package
/// if possible. If multiple versions exist, try to match
/// the major and minor versions of the `@types` package with the
/// actual package, falling back to the highest @types version present.
pub fn find_definitely_typed_package_from_snapshot<'a>(
  types_package_name: &str,
  maybe_package_version: Option<&Version>,
  snapshot: &'a NpmResolutionSnapshot,
) -> Option<&'a NpmPackageId> {
  fn is_id_higher_than_id(new: &NpmPackageId, existing: &NpmPackageId) -> bool {
    match new.nv.version.cmp(&existing.nv.version) {
      Ordering::Equal => new.peer_dependencies > existing.peer_dependencies,
      Ordering::Greater => true,
      Ordering::Less => false,
    }
  }

  let mut best_patch = 0;
  let mut highest: Option<&NpmPackageId> = None;
  let mut best: Option<&NpmPackageId> = None;
  let all_ids = snapshot
    .top_level_packages()
    // not exactly correct, but this is fine because @types/ packages
    // won't ever be conditional on a system
    .chain(snapshot.all_packages_for_every_system().map(|pkg| &pkg.id));

  for id in all_ids {
    if id.nv.name != types_package_name {
      continue;
    }
    if let Some(package_version) = maybe_package_version
      && id.nv.version.major == package_version.major
      && id.nv.version.minor == package_version.minor
      && id.nv.version.patch >= best_patch
      && id.nv.version.pre == package_version.pre
    {
      let should_replace = match &best {
        Some(best_id) => is_id_higher_than_id(id, best_id),
        None => true,
      };
      if should_replace {
        best = Some(id);
        best_patch = id.nv.version.patch;
      }
    }

    let should_replace = match &highest {
      Some(highest_id) => is_id_higher_than_id(id, highest_id),
      None => true,
    };
    if should_replace {
      highest = Some(id);
    }
  }

  best.or(highest)
}
