// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::npm::NpmCacheDir;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::NpmPackageCacheFolderId;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_semver::package::PackageNv;
use deno_semver::StackString;
use deno_semver::Version;
use http::HeaderName;
use http::HeaderValue;
use http::StatusCode;
use parking_lot::Mutex;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;
use url::Url;

mod fs_util;
mod registry_info;
mod remote;
mod tarball;
mod tarball_extract;

pub use fs_util::hard_link_dir_recursive;
pub use fs_util::hard_link_file;
pub use fs_util::HardLinkDirRecursiveError;
pub use fs_util::HardLinkFileError;
// todo(#27198): make both of these private and get the rest of the code
// using RegistryInfoProvider.
pub use registry_info::get_package_url;
pub use registry_info::RegistryInfoProvider;
pub use remote::maybe_auth_header_for_npm_registry;
pub use tarball::EnsurePackageError;
pub use tarball::TarballCache;

#[derive(Debug, deno_error::JsError)]
#[class(generic)]
pub struct DownloadError {
  pub status_code: Option<StatusCode>,
  pub error: JsErrorBox,
}

impl std::error::Error for DownloadError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    self.error.source()
  }
}

impl std::fmt::Display for DownloadError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self.error.fmt(f)
  }
}

#[async_trait::async_trait(?Send)]
pub trait NpmCacheHttpClient: Send + Sync + 'static {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth_header: Option<(HeaderName, HeaderValue)>,
  ) -> Result<Option<Vec<u8>>, DownloadError>;
}

/// Indicates how cached source files should be handled.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NpmCacheSetting {
  /// Only the cached files should be used. Any files not in the cache will
  /// error. This is the equivalent of `--cached-only` in the CLI.
  Only,
  /// No cached source files should be used, and all files should be reloaded.
  /// This is the equivalent of `--reload` in the CLI.
  ReloadAll,
  /// Only some cached resources should be used. This is the equivalent of
  /// `--reload=npm:chalk`
  ReloadSome { npm_package_names: Vec<String> },
  /// The cached source files should be used for local modules. This is the
  /// default behavior of the CLI.
  Use,
}

impl NpmCacheSetting {
  pub fn from_cache_setting(cache_setting: &CacheSetting) -> NpmCacheSetting {
    match cache_setting {
      CacheSetting::Only => NpmCacheSetting::Only,
      CacheSetting::ReloadAll => NpmCacheSetting::ReloadAll,
      CacheSetting::ReloadSome(values) => {
        if values.iter().any(|v| v == "npm:") {
          NpmCacheSetting::ReloadAll
        } else {
          NpmCacheSetting::ReloadSome {
            npm_package_names: values
              .iter()
              .filter_map(|v| v.strip_prefix("npm:"))
              .map(|n| n.to_string())
              .collect(),
          }
        }
      }
      CacheSetting::RespectHeaders => panic!("not supported"),
      CacheSetting::Use => NpmCacheSetting::Use,
    }
  }
  pub fn should_use_for_npm_package(&self, package_name: &str) -> bool {
    match self {
      NpmCacheSetting::ReloadAll => false,
      NpmCacheSetting::ReloadSome { npm_package_names } => {
        !npm_package_names.iter().any(|n| n == package_name)
      }
      _ => true,
    }
  }
}

/// Stores a single copy of npm packages in a cache.
#[derive(Debug)]
pub struct NpmCache<
  TSys: FsCreateDirAll
    + FsHardLink
    + FsMetadata
    + FsOpen
    + FsReadDir
    + FsRemoveFile
    + FsRename
    + ThreadSleep
    + SystemRandom,
> {
  cache_dir: Arc<NpmCacheDir>,
  sys: TSys,
  cache_setting: NpmCacheSetting,
  npmrc: Arc<ResolvedNpmRc>,
  previously_reloaded_packages: Mutex<HashSet<PackageNv>>,
}

impl<
    TSys: FsCreateDirAll
      + FsHardLink
      + FsMetadata
      + FsOpen
      + FsReadDir
      + FsRemoveFile
      + FsRename
      + ThreadSleep
      + SystemRandom,
  > NpmCache<TSys>
{
  pub fn new(
    cache_dir: Arc<NpmCacheDir>,
    sys: TSys,
    cache_setting: NpmCacheSetting,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      cache_dir,
      sys,
      cache_setting,
      npmrc,
      previously_reloaded_packages: Default::default(),
    }
  }

  pub fn cache_setting(&self) -> &NpmCacheSetting {
    &self.cache_setting
  }

  pub fn root_dir_path(&self) -> &Path {
    self.cache_dir.root_dir()
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
  ) -> Result<(), WithFolderSyncLockError> {
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
      hard_link_dir_recursive(
        &self.sys,
        &original_package_folder,
        &package_folder,
      )
      .map_err(JsErrorBox::from_err)
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

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .cache_dir
      .resolve_package_folder_id_from_specifier(specifier)
      .and_then(|cache_id| {
        Some(NpmPackageCacheFolderId {
          nv: PackageNv {
            name: StackString::from_string(cache_id.name),
            version: Version::parse_from_npm(&cache_id.version).ok()?,
          },
          copy_index: cache_id.copy_index,
        })
      })
  }

  pub fn load_package_info(
    &self,
    name: &str,
  ) -> Result<Option<NpmPackageInfo>, serde_json::Error> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);

    let file_text = match std::fs::read_to_string(file_cache_path) {
      Ok(file_text) => file_text,
      Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
      Err(err) => return Err(serde_json::Error::io(err)),
    };
    serde_json::from_str(&file_text)
  }

  pub fn save_package_info(
    &self,
    name: &str,
    package_info: &NpmPackageInfo,
  ) -> Result<(), JsErrorBox> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);
    let file_text =
      serde_json::to_string(&package_info).map_err(JsErrorBox::from_err)?;
    atomic_write_file_with_retries(
      &self.sys,
      &file_cache_path,
      file_text.as_bytes(),
      0o644,
    )
    .map_err(JsErrorBox::from_err)?;
    Ok(())
  }

  fn get_registry_package_info_file_cache_path(&self, name: &str) -> PathBuf {
    let name_folder_path = self.package_name_folder(name);
    name_folder_path.join("registry.json")
  }
}

const NPM_PACKAGE_SYNC_LOCK_FILENAME: &str = ".deno_sync_lock";

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WithFolderSyncLockError {
  #[class(inherit)]
  #[error("Error creating '{path}'")]
  CreateDir {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Error creating package sync lock file at '{path}'. Maybe try manually deleting this folder.")]
  CreateLockFile {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  Action(#[from] JsErrorBox),
  #[class(generic)]
  #[error("Failed setting up package cache directory for {package}, then failed cleaning it up.\n\nOriginal error:\n\n{error}\n\nRemove error:\n\n{remove_error}\n\nPlease manually delete this folder or you will run into issues using this package in the future:\n\n{output_folder}")]
  SetUpPackageCacheDir {
    package: Box<PackageNv>,
    error: Box<WithFolderSyncLockError>,
    remove_error: std::io::Error,
    output_folder: PathBuf,
  },
}

// todo(dsherret): use `sys` here instead of `std::fs`.
fn with_folder_sync_lock(
  package: &PackageNv,
  output_folder: &Path,
  action: impl FnOnce() -> Result<(), JsErrorBox>,
) -> Result<(), WithFolderSyncLockError> {
  fn inner(
    output_folder: &Path,
    action: impl FnOnce() -> Result<(), JsErrorBox>,
  ) -> Result<(), WithFolderSyncLockError> {
    std::fs::create_dir_all(output_folder).map_err(|source| {
      WithFolderSyncLockError::CreateDir {
        path: output_folder.to_path_buf(),
        source,
      }
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
    match std::fs::OpenOptions::new()
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
      Err(err) => Err(WithFolderSyncLockError::CreateLockFile {
        path: output_folder.to_path_buf(),
        source: err,
      }),
    }
  }

  match inner(output_folder, action) {
    Ok(()) => Ok(()),
    Err(err) => {
      if let Err(remove_err) = std::fs::remove_dir_all(output_folder) {
        if remove_err.kind() != std::io::ErrorKind::NotFound {
          return Err(WithFolderSyncLockError::SetUpPackageCacheDir {
            package: Box::new(package.clone()),
            error: Box::new(err),
            remove_error: remove_err,
            output_folder: output_folder.to_path_buf(),
          });
        }
      }
      Err(err)
    }
  }
}
