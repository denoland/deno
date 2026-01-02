// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_path_util::url_from_directory_path;
use deno_semver::Version;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPathRef;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

use super::common::join_package_name_to_path;
use super::resolution::NpmResolutionCellRc;
use crate::npm::local::get_package_folder_id_folder_name_from_parts;
use crate::npm::local::get_package_folder_id_from_folder_name;

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
#[derive(Debug)]
pub struct LocalNpmPackageResolver<TSys: FsCanonicalize + FsMetadata> {
  resolution: NpmResolutionCellRc,
  sys: TSys,
  root_node_modules_path: PathBuf,
  root_node_modules_url: Url,
}

impl<TSys: FsCanonicalize + FsMetadata> LocalNpmPackageResolver<TSys> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    resolution: NpmResolutionCellRc,
    sys: TSys,
    node_modules_folder: PathBuf,
  ) -> Self {
    Self {
      resolution,
      sys,
      root_node_modules_url: url_from_directory_path(&node_modules_folder)
        .unwrap(),
      root_node_modules_path: node_modules_folder,
    }
  }

  pub fn node_modules_path(&self) -> Option<&Path> {
    Some(self.root_node_modules_path.as_ref())
  }

  pub fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let folder_copy_index = self
      .resolution
      .resolve_pkg_cache_folder_copy_index_from_pkg_id(id)?;
    // package is stored at:
    // node_modules/.deno/<package_cache_folder_id_folder_name>/node_modules/<package_name>
    Some(
      self
        .root_node_modules_path
        .join(".deno")
        .join(get_package_folder_id_folder_name_from_parts(
          &id.nv,
          folder_copy_index,
        ))
        .join("node_modules")
        .join(&id.nv.name),
    )
  }

  pub fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error> {
    let Some(folder_path) =
      self.resolve_package_folder_from_specifier(specifier)?
    else {
      return Ok(None);
    };
    // ex. project/node_modules/.deno/preact@10.24.3/node_modules/preact/
    let Some(node_modules_ancestor) = folder_path
      .ancestors()
      .find(|ancestor| ancestor.ends_with("node_modules"))
    else {
      return Ok(None);
    };
    let Some(folder_name) =
      node_modules_ancestor.parent().and_then(|p| p.file_name())
    else {
      return Ok(None);
    };
    Ok(get_package_folder_id_from_folder_name(
      &folder_name.to_string_lossy(),
    ))
  }

  fn resolve_package_root(&self, path: &Path) -> PathBuf {
    let mut last_found = path;
    loop {
      let parent = last_found.parent().unwrap();
      if parent.file_name().unwrap() == "node_modules" {
        return last_found.to_path_buf();
      } else {
        last_found = parent;
      }
    }
  }

  fn resolve_folder_for_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(relative_url) =
      self.root_node_modules_url.make_relative(specifier)
    else {
      return Ok(None);
    };
    if relative_url.starts_with("../") {
      return Ok(None);
    }
    // it's within the directory, so use it
    let Some(path) = deno_path_util::url_to_file_path(specifier).ok() else {
      return Ok(None);
    };
    // Canonicalize the path so it's not pointing to the symlinked directory
    // in `node_modules` directory of the referrer.
    canonicalize_path_maybe_not_exists(&self.sys, &path).map(Some)
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(local_path) = self.resolve_folder_for_specifier(specifier)? else {
      return Ok(None);
    };
    let package_root_path = self.resolve_package_root(&local_path);
    Ok(Some(package_root_path))
  }
}

impl<TSys: FsCanonicalize + FsMetadata> NpmPackageFolderResolver
  for LocalNpmPackageResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    let maybe_local_path = self
      .resolve_folder_for_specifier(referrer.url()?)
      .map_err(|err| PackageFolderResolveIoError {
      package_name: name.to_string(),
      referrer: referrer.display(),
      source: err,
    })?;
    let Some(local_path) = maybe_local_path else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.display(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    // go from the current path down because it might have bundled dependencies
    for current_folder in local_path.ancestors().skip(1) {
      let node_modules_folder = if current_folder.ends_with("node_modules") {
        Cow::Borrowed(current_folder)
      } else {
        Cow::Owned(current_folder.join("node_modules"))
      };

      let sub_dir = join_package_name_to_path(&node_modules_folder, name);
      if self.sys.fs_is_dir_no_err(&sub_dir) {
        return Ok(self.sys.fs_canonicalize(&sub_dir).map_err(|err| {
          PackageFolderResolveIoError {
            package_name: name.to_string(),
            referrer: referrer.display(),
            source: err,
          }
        })?);
      }

      if current_folder == self.root_node_modules_path {
        break;
      }
    }

    Err(
      PackageNotFoundError {
        package_name: name.to_string(),
        referrer: referrer.display(),
        referrer_extra: None,
      }
      .into(),
    )
  }

  fn resolve_types_package_folder(
    &self,
    types_package_name: &str,
    maybe_package_version: Option<&Version>,
    maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Option<PathBuf> {
    if let Some(referrer) = maybe_referrer
      && let Ok(path) =
        self.resolve_package_folder_from_package(types_package_name, referrer)
    {
      Some(path)
    } else {
      // otherwise, try to find one in the snapshot
      let snapshot = self.resolution.snapshot();
      let pkg_id = super::common::find_definitely_typed_package_from_snapshot(
        types_package_name,
        maybe_package_version,
        &snapshot,
      )?;
      self.maybe_package_folder(pkg_id)
    }
  }
}
