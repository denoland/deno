// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_npm::NpmPackageCacheFolderId;
use deno_runtime::deno_fs;
use deno_semver::package::PackageNv;

use crate::args::CacheSetting;
use crate::http_util::HttpClient;
use crate::npm::NpmCacheDir;
use crate::util::fs::hard_link_dir_recursive;
use crate::util::progress_bar::ProgressBar;

use super::tarball::verify_and_extract_tarball;

/// Stores a single copy of npm packages in a cache.
#[derive(Debug)]
pub struct NpmCache {
  cache_dir: NpmCacheDir,
  cache_setting: CacheSetting,
  fs: Arc<dyn deno_fs::FileSystem>,
  http_client: Arc<HttpClient>,
  progress_bar: ProgressBar,
  /// ensures a package is only downloaded once per run
  previously_reloaded_packages: Mutex<HashSet<PackageNv>>,
}

impl NpmCache {
  pub fn new(
    cache_dir: NpmCacheDir,
    cache_setting: CacheSetting,
    fs: Arc<dyn deno_fs::FileSystem>,
    http_client: Arc<HttpClient>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      cache_dir,
      cache_setting,
      fs,
      http_client,
      progress_bar,
      previously_reloaded_packages: Default::default(),
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
  fn should_use_global_cache_for_package(&self, package: &PackageNv) -> bool {
    self.cache_setting.should_use_for_npm_package(&package.name)
      || !self
        .previously_reloaded_packages
        .lock()
        .insert(package.clone())
  }

  pub async fn ensure_package(
    &self,
    package: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    self
      .ensure_package_inner(package, dist, registry_url)
      .await
      .with_context(|| format!("Failed caching npm package '{package}'."))
  }

  async fn ensure_package_inner(
    &self,
    package: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    let package_folder = self
      .cache_dir
      .package_folder_for_name_and_version(package, registry_url);
    if self.should_use_global_cache_for_package(package)
      && self.fs.exists_sync(&package_folder)
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !self.fs.exists_sync(&package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME))
    {
      return Ok(());
    } else if self.cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{}\", --cached-only is specified.",
          &package.name
        )
      )
      );
    }

    if dist.tarball.is_empty() {
      bail!("Tarball URL was empty.");
    }

    let guard = self.progress_bar.update(&dist.tarball);
    let maybe_bytes = self
      .http_client
      .download_with_progress(&dist.tarball, &guard)
      .await?;
    match maybe_bytes {
      Some(bytes) => {
        verify_and_extract_tarball(package, &bytes, dist, &package_folder)
      }
      None => {
        bail!("Could not find npm package tarball at: {}", dist.tarball);
      }
    }
  }

  /// Ensures a copy of the package exists in the global cache.
  ///
  /// This assumes that the original package folder being hard linked
  /// from exists before this is called.
  pub fn ensure_copy_package(
    &self,
    folder_id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    assert_ne!(folder_id.copy_index, 0);
    let package_folder = self
      .cache_dir
      .package_folder_for_id(folder_id, registry_url);

    if package_folder.exists()
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
      && self.cache_setting.should_use_for_npm_package(&folder_id.nv.name)
    {
      return Ok(());
    }

    let original_package_folder = self
      .cache_dir
      .package_folder_for_name_and_version(&folder_id.nv, registry_url);
    with_folder_sync_lock(&folder_id.nv, &package_folder, || {
      hard_link_dir_recursive(&original_package_folder, &package_folder)
    })?;
    Ok(())
  }

  pub fn package_folder_for_id(
    &self,
    id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> PathBuf {
    self.cache_dir.package_folder_for_id(id, registry_url)
  }

  pub fn package_folder_for_name_and_version(
    &self,
    package: &PackageNv,
    registry_url: &Url,
  ) -> PathBuf {
    self
      .cache_dir
      .package_folder_for_name_and_version(package, registry_url)
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    self.cache_dir.package_name_folder(name, registry_url)
  }

  pub fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self.cache_dir.registry_folder(registry_url)
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .cache_dir
      .resolve_package_folder_id_from_specifier(specifier, registry_url)
  }
}

const NPM_PACKAGE_SYNC_LOCK_FILENAME: &str = ".deno_sync_lock";

pub fn with_folder_sync_lock(
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
