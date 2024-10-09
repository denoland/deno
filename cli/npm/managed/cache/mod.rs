// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::npm::NpmCacheDir;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::NpmPackageCacheFolderId;
use deno_semver::package::PackageNv;
use deno_semver::Version;

use crate::args::CacheSetting;
use crate::cache::CACHE_PERM;
use crate::util::fs::atomic_write_file_with_retries;
use crate::util::fs::hard_link_dir_recursive;

mod registry_info;
mod tarball;
mod tarball_extract;

pub use registry_info::RegistryInfoDownloader;
pub use tarball::TarballCache;

/// Stores a single copy of npm packages in a cache.
#[derive(Debug)]
pub struct NpmCache {
  cache_dir: NpmCacheDir,
  cache_setting: CacheSetting,
  npmrc: Arc<ResolvedNpmRc>,
  /// ensures a package is only downloaded once per run
  previously_reloaded_packages: Mutex<HashSet<PackageNv>>,
}

impl NpmCache {
  pub fn new(
    cache_dir: NpmCacheDir,
    cache_setting: CacheSetting,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache_dir,
      cache_setting,
      previously_reloaded_packages: Default::default(),
      npmrc,
    }
  }

  pub fn cache_setting(&self) -> &CacheSetting {
    &self.cache_setting
  }

  pub fn root_dir_url(&self) -> &Url {
    self.cache_dir.root_dir_url()
  }

  /// Checks if the cache should be used for the provided name and version.
  /// NOTE: Subsequent calls for the same package will always return `true`
  /// to ensure a package is only downloaded once per run of the CLI. This
  /// prevents downloads from re-occurring when someone has `--reload` and
  /// and imports a dynamic import that imports the same package again for example.
  pub fn should_use_cache_for_package(&self, package: &PackageNv) -> bool {
    self.cache_setting.should_use_for_npm_package(&package.name)
      || !self
        .previously_reloaded_packages
        .lock()
        .insert(package.clone())
  }

  /// Ensures a copy of the package exists in the global cache.
  ///
  /// This assumes that the original package folder being hard linked
  /// from exists before this is called.
  pub fn ensure_copy_package(
    &self,
    folder_id: &NpmPackageCacheFolderId,
  ) -> Result<(), AnyError> {
    let registry_url = self.npmrc.get_registry_url(&folder_id.nv.name);
    assert_ne!(folder_id.copy_index, 0);
    let package_folder = self.cache_dir.package_folder_for_id(
      &folder_id.nv.name,
      &folder_id.nv.version.to_string(),
      folder_id.copy_index,
      registry_url,
    );

    if package_folder.exists()
      // if this file exists, then the package didn't successfully initialize
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
      && self.cache_setting.should_use_for_npm_package(&folder_id.nv.name)
    {
      return Ok(());
    }

    let original_package_folder = self.cache_dir.package_folder_for_id(
      &folder_id.nv.name,
      &folder_id.nv.version.to_string(),
      0, // original copy index
      registry_url,
    );

    // it seems Windows does an "AccessDenied" error when moving a
    // directory with hard links, so that's why this solution is done
    with_folder_sync_lock(&folder_id.nv, &package_folder, || {
      hard_link_dir_recursive(&original_package_folder, &package_folder)
    })?;
    Ok(())
  }

  pub fn package_folder_for_id(&self, id: &NpmPackageCacheFolderId) -> PathBuf {
    let registry_url = self.npmrc.get_registry_url(&id.nv.name);
    self.cache_dir.package_folder_for_id(
      &id.nv.name,
      &id.nv.version.to_string(),
      id.copy_index,
      registry_url,
    )
  }

  pub fn package_folder_for_nv(&self, package: &PackageNv) -> PathBuf {
    let registry_url = self.npmrc.get_registry_url(&package.name);
    self.package_folder_for_nv_and_url(package, registry_url)
  }

  pub fn package_folder_for_nv_and_url(
    &self,
    package: &PackageNv,
    registry_url: &Url,
  ) -> PathBuf {
    self.cache_dir.package_folder_for_id(
      &package.name,
      &package.version.to_string(),
      0, // original copy_index
      registry_url,
    )
  }

  pub fn package_name_folder(&self, name: &str) -> PathBuf {
    let registry_url = self.npmrc.get_registry_url(name);
    self.cache_dir.package_name_folder(name, registry_url)
  }

  pub fn root_folder(&self) -> PathBuf {
    self.cache_dir.root_dir().to_owned()
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .cache_dir
      .resolve_package_folder_id_from_specifier(specifier)
      .and_then(|cache_id| {
        Some(NpmPackageCacheFolderId {
          nv: PackageNv {
            name: cache_id.name,
            version: Version::parse_from_npm(&cache_id.version).ok()?,
          },
          copy_index: cache_id.copy_index,
        })
      })
  }

  pub fn load_package_info(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, AnyError> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);

    let file_text = match fs::read_to_string(file_cache_path) {
      Ok(file_text) => file_text,
      Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
      Err(err) => return Err(err.into()),
    };
    Ok(serde_json::from_str(&file_text)?)
  }

  pub fn save_package_info(
    &self,
    name: &str,
    package_info: &NpmPackageInfo,
  ) -> Result<(), AnyError> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);
    let file_text = serde_json::to_string(&package_info)?;
    atomic_write_file_with_retries(&file_cache_path, file_text, CACHE_PERM)?;
    Ok(())
  }

  fn get_registry_package_info_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.package_name_folder(name);
    name_folder_path.join("registry.json")
  }
}

const NPM_PACKAGE_SYNC_LOCK_FILENAME: &str = ".deno_sync_lock";

fn with_folder_sync_lock(
  package: &PackageNv,
  output_folder: &Path,
  action: impl FnOnce() -> Result<(), AnyError>,
) -> Result<(), AnyError> {
  fn inner(
    output_folder: &Path,
    action: impl FnOnce() -> Result<(), AnyError>,
  ) -> Result<(), AnyError> {
    fs::create_dir_all(output_folder).with_context(|| {
      format!("Error creating '{}'.", output_folder.display())
    })?;

    // This sync lock file is a way to ensure that partially created
    // npm package directories aren't considered valid. This could maybe
    // be a bit smarter in the future to not bother extracting here
    // if another process has taken the lock in the past X seconds and
    // wait for the other process to finish (it could try to create the
    // file with `create_new(true)` then if it exists, check the metadata
    // then wait until the other process finishes with a timeout), but
    // for now this is good enough.
    let sync_lock_path = output_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME);
    match fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(false)
      .open(&sync_lock_path)
    {
      Ok(_) => {
        action()?;
        // extraction succeeded, so only now delete this file
        let _ignore = std::fs::remove_file(&sync_lock_path);
        Ok(())
      }
      Err(err) => {
        bail!(
          concat!(
            "Error creating package sync lock file at '{}'. ",
            "Maybe try manually deleting this folder.\n\n{:#}",
          ),
          output_folder.display(),
          err
        );
      }
    }
  }

  match inner(output_folder, action) {
    Ok(()) => Ok(()),
    Err(err) => {
      if let Err(remove_err) = fs::remove_dir_all(output_folder) {
        if remove_err.kind() != std::io::ErrorKind::NotFound {
          bail!(
            concat!(
              "Failed setting up package cache directory for {}, then ",
              "failed cleaning it up.\n\nOriginal error:\n\n{}\n\n",
              "Remove error:\n\n{}\n\nPlease manually ",
              "delete this folder or you will run into issues using this ",
              "package in the future:\n\n{}"
            ),
            package,
            err,
            remove_err,
            output_folder.display(),
          );
        }
      }
      Err(err)
    }
  }
}
