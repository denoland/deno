// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for local node_modules resolution.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_ast::ModuleSpecifier;
use deno_cache_dir::npm::mixed_case_package_name_decode;
use deno_core::url::Url;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_resolver::npm::normalize_pkg_name_for_node_modules_deno_folder;
use deno_semver::package::PackageNv;
use deno_semver::StackString;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;
use sys_traits::FsMetadata;

use super::super::resolution::NpmResolution;
use super::common::NpmPackageFsResolver;
use crate::sys::CliSys;

/// Resolver that creates a local node_modules directory
/// and resolves packages from it.
#[derive(Debug)]
pub struct LocalNpmPackageResolver {
  resolution: Arc<NpmResolution>,
  sys: CliSys,
  root_node_modules_path: PathBuf,
  root_node_modules_url: Url,
}

impl LocalNpmPackageResolver {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    resolution: Arc<NpmResolution>,
    sys: CliSys,
    node_modules_folder: PathBuf,
  ) -> Self {
    Self {
      resolution,
      sys,
      root_node_modules_url: Url::from_directory_path(&node_modules_folder)
        .unwrap(),
      root_node_modules_path: node_modules_folder,
    }
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
    specifier: &ModuleSpecifier,
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
    let Some(path) = specifier.to_file_path().ok() else {
      return Ok(None);
    };
    // Canonicalize the path so it's not pointing to the symlinked directory
    // in `node_modules` directory of the referrer.
    canonicalize_path_maybe_not_exists(&self.sys, &path).map(Some)
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, std::io::Error> {
    let Some(local_path) = self.resolve_folder_for_specifier(specifier)? else {
      return Ok(None);
    };
    let package_root_path = self.resolve_package_root(&local_path);
    Ok(Some(package_root_path))
  }
}

#[async_trait(?Send)]
impl NpmPackageFsResolver for LocalNpmPackageResolver {
  fn node_modules_path(&self) -> Option<&Path> {
    Some(self.root_node_modules_path.as_ref())
  }

  fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let cache_folder_id = self
      .resolution
      .resolve_pkg_cache_folder_id_from_pkg_id(id)?;
    // package is stored at:
    // node_modules/.deno/<package_cache_folder_id_folder_name>/node_modules/<package_name>
    Some(
      self
        .root_node_modules_path
        .join(".deno")
        .join(get_package_folder_id_folder_name(&cache_folder_id))
        .join("node_modules")
        .join(&cache_folder_id.nv.name),
    )
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    let maybe_local_path = self
      .resolve_folder_for_specifier(referrer)
      .map_err(|err| PackageFolderResolveIoError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        source: err,
      })?;
    let Some(local_path) = maybe_local_path else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.clone(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    let package_root_path = self.resolve_package_root(&local_path);
    let mut current_folder = package_root_path.as_path();
    while let Some(parent_folder) = current_folder.parent() {
      current_folder = parent_folder;
      let node_modules_folder = if current_folder.ends_with("node_modules") {
        Cow::Borrowed(current_folder)
      } else {
        Cow::Owned(current_folder.join("node_modules"))
      };

      let sub_dir = join_package_name(&node_modules_folder, name);
      if self.sys.fs_is_dir_no_err(&sub_dir) {
        return Ok(sub_dir);
      }

      if current_folder == self.root_node_modules_path {
        break;
      }
    }

    Err(
      PackageNotFoundError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        referrer_extra: None,
      }
      .into(),
    )
  }

  fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
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
}

pub fn get_package_folder_id_folder_name(
  folder_id: &NpmPackageCacheFolderId,
) -> String {
  let copy_str = if folder_id.copy_index == 0 {
    Cow::Borrowed("")
  } else {
    Cow::Owned(format!("_{}", folder_id.copy_index))
  };
  let nv = &folder_id.nv;
  let name = normalize_pkg_name_for_node_modules_deno_folder(&nv.name);
  format!("{}@{}{}", name, nv.version, copy_str)
}

fn get_package_folder_id_from_folder_name(
  folder_name: &str,
) -> Option<NpmPackageCacheFolderId> {
  let folder_name = folder_name.replace('+', "/");
  let (name, ending) = folder_name.rsplit_once('@')?;
  let name: StackString = if let Some(encoded_name) = name.strip_prefix('_') {
    StackString::from_string(mixed_case_package_name_decode(encoded_name)?)
  } else {
    name.into()
  };
  let (raw_version, copy_index) = match ending.split_once('_') {
    Some((raw_version, copy_index)) => {
      let copy_index = copy_index.parse::<u8>().ok()?;
      (raw_version, copy_index)
    }
    None => (ending, 0),
  };
  let version = deno_semver::Version::parse_from_npm(raw_version).ok()?;
  Some(NpmPackageCacheFolderId {
    nv: PackageNv { name, version },
    copy_index,
  })
}

fn join_package_name(path: &Path, package_name: &str) -> PathBuf {
  let mut path = path.to_path_buf();
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = path.join(part);
  }
  path
}

#[cfg(test)]
mod test {
  use deno_npm::NpmPackageCacheFolderId;
  use deno_semver::package::PackageNv;

  use super::*;

  #[test]
  fn test_get_package_folder_id_folder_name() {
    let cases = vec![
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("@types/foo@1.2.3").unwrap(),
          copy_index: 1,
        },
        "@types+foo@1.2.3_1".to_string(),
      ),
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("JSON@3.2.1").unwrap(),
          copy_index: 0,
        },
        "_jjju6tq@3.2.1".to_string(),
      ),
    ];
    for (input, output) in cases {
      assert_eq!(get_package_folder_id_folder_name(&input), output);
      let folder_id = get_package_folder_id_from_folder_name(&output).unwrap();
      assert_eq!(folder_id, input);
    }
  }
}
