// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::npm::NpmCacheDir;
use deno_error::JsErrorBox;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::fast_registry_json;
use deno_npmrc::RegistryConfig;
use deno_npmrc::ResolvedNpmRc;
use deno_path_util::fs::atomic_write_file_with_retries;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::package::PackageNv;
use parking_lot::Mutex;
use serde_json::value::RawValue;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsHardLink;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveDirAll;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;
use url::Url;

mod fs_util;
mod registry_info;
mod remote;
mod rt;
mod tarball;
pub mod tarball_extract;

pub use fs_util::hard_link_dir_recursive;
pub use fs_util::hard_link_file;
pub use registry_info::RegistryInfoProvider;
pub use registry_info::SerializedCachedPackageInfo;
pub use registry_info::get_package_url;
pub use remote::maybe_auth_header_value_for_npm_registry;
pub use tarball::EnsurePackageError;
pub use tarball::TarballCache;
pub use tarball::TarballCacheReporter;

use self::rt::spawn_blocking;

#[derive(Debug, deno_error::JsError)]
#[class(generic)]
pub struct DownloadError {
  pub status_code: Option<u16>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpmPackumentFormat {
  /// Request the abbreviated install manifest (smaller, but omits `time` and `scripts`).
  Abbreviated,
  /// Request the full packument (needed when `minimumDependencyAge` is configured).
  Full,
}

pub enum NpmCacheHttpClientResponse {
  NotFound,
  NotModified,
  Bytes(NpmCacheHttpClientBytesResponse),
}

pub struct NpmCacheHttpClientBytesResponse {
  pub bytes: Vec<u8>,
  pub etag: Option<String>,
}

#[async_trait::async_trait(?Send)]
pub trait NpmCacheHttpClient: std::fmt::Debug + Send + Sync + 'static {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth: Option<String>,
    maybe_etag: Option<String>,
    maybe_registry_config: Option<&RegistryConfig>,
  ) -> Result<NpmCacheHttpClientResponse, DownloadError>;
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

#[sys_traits::auto_impl]
pub trait NpmCacheSys:
  FsCanonicalize
  + FsCreateDirAll
  + FsHardLink
  + FsMetadata
  + FsOpen
  + FsRead
  + FsReadDir
  + FsRemoveDirAll
  + FsRemoveFile
  + FsRename
  + ThreadSleep
  + SystemRandom
  + Send
  + Sync
  + Clone
  + std::fmt::Debug
  + 'static
{
}

/// Stores a single copy of npm packages in a cache.
#[derive(Debug)]
pub struct NpmCache<TSys: NpmCacheSys> {
  cache_dir: Arc<NpmCacheDir>,
  sys: TSys,
  cache_setting: NpmCacheSetting,
  npmrc: Arc<ResolvedNpmRc>,
  previously_reloaded_packages: Mutex<HashSet<PackageNv>>,
}

impl<TSys: NpmCacheSys> NpmCache<TSys> {
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

    if self.sys.fs_exists_no_err(&package_folder)
      // if this file exists, then the package didn't successfully initialize
      // the first time, or another process is currently extracting the zip file
      && !self.sys.fs_exists_no_err(package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME))
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
    with_folder_sync_lock(&self.sys, &folder_id.nv, &package_folder, || {
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

  pub async fn load_package_info(
    &self,
    name: &str,
    _packument_format: NpmPackumentFormat,
  ) -> Result<Option<SerializedCachedPackageInfo>, serde_json::Error> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);

    let file_bytes = match self.sys.fs_read(&file_cache_path) {
      Ok(file_text) => file_text,
      Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
      Err(err) => return Err(serde_json::Error::io(err)),
    };

    spawn_blocking(move || {
      let (info, cache_metadata) =
        deno_npm::registry::NpmPackageInfo::from_packument_bytes_with_cache_info(
          file_bytes.into_owned(),
        )
        .map_err(|err| serde_json::Error::io(std::io::Error::other(err)))?;
      Ok(Some(SerializedCachedPackageInfo {
        info,
        etag: cache_metadata.etag,
        full_packument: cache_metadata.full_packument,
      }))
    })
    .await
    .unwrap()
  }

  pub fn save_package_info(
    &self,
    name: &str,
    package_info: &SerializedCachedPackageInfo,
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

  pub fn build_package_info_cache_bytes(
    &self,
    package_info_bytes: &[u8],
    etag: Option<&str>,
    packument_format: NpmPackumentFormat,
  ) -> Result<Vec<u8>, JsErrorBox> {
    slim_package_info_bytes(
      package_info_bytes,
      etag,
      packument_format == NpmPackumentFormat::Full,
    )
  }

  pub fn save_package_info_bytes(
    &self,
    name: &str,
    package_info_bytes: &[u8],
  ) -> Result<(), JsErrorBox> {
    let file_cache_path = self.get_registry_package_info_file_cache_path(name);
    atomic_write_file_with_retries(
      &self.sys,
      &file_cache_path,
      package_info_bytes,
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

fn slim_package_info_bytes(
  package_info_bytes: &[u8],
  etag: Option<&str>,
  full_packument: bool,
) -> Result<Vec<u8>, JsErrorBox> {
  let text = std::str::from_utf8(package_info_bytes)
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  let index = fast_registry_json::pluck_packument_index(text)
    .map_err(|err| JsErrorBox::generic(format!("{err:?}")))?;

  let mut output =
    Vec::with_capacity(package_info_bytes.len().min(1024 * 1024));
  output.push(b'{');
  let mut first = true;

  if let Some(name) = index.name {
    write_json_property_name(&mut output, &mut first, "name")
      .map_err(JsErrorBox::from_err)?;
    serde_json::to_writer(&mut output, name).map_err(JsErrorBox::from_err)?;
  }

  write_json_property_name(&mut output, &mut first, "dist-tags")
    .map_err(JsErrorBox::from_err)?;
  write_string_map(&mut output, index.dist_tags.iter())
    .map_err(JsErrorBox::from_err)?;

  write_json_property_name(&mut output, &mut first, "versions")
    .map_err(JsErrorBox::from_err)?;
  output.push(b'{');
  let mut first_version = true;
  for (version, (start, end)) in
    index.versions.iter().zip(index.version_ranges.iter())
  {
    write_json_property_name(&mut output, &mut first_version, version)
      .map_err(JsErrorBox::from_err)?;
    let evidence = index.trust_evidence.get(version).copied();
    write_slim_version(
      &mut output,
      version,
      &text[*start as usize..*end as usize],
      evidence,
    )?;
  }
  output.push(b'}');

  write_json_property_name(&mut output, &mut first, "time")
    .map_err(JsErrorBox::from_err)?;
  write_string_map(&mut output, index.time.iter())
    .map_err(JsErrorBox::from_err)?;

  if let Some(etag) = etag {
    write_json_property_name(&mut output, &mut first, "_deno.etag")
      .map_err(JsErrorBox::from_err)?;
    serde_json::to_writer(&mut output, etag).map_err(JsErrorBox::from_err)?;
  }

  if full_packument {
    // Record that this cache entry came from a full packument response, so
    // an empty `time` map means the registry provides no publish dates and
    // there's no point re-fetching to look for them (see #35761).
    write_json_property_name(&mut output, &mut first, "_deno.packumentFormat")
      .map_err(JsErrorBox::from_err)?;
    output.extend_from_slice(b"\"full\"");
  }

  output.push(b'}');
  Ok(output)
}

fn write_slim_version(
  output: &mut Vec<u8>,
  version: &str,
  version_json: &str,
  evidence: Option<fast_registry_json::TrustEvidence>,
) -> Result<(), JsErrorBox> {
  let raw_fields =
    serde_json::from_str::<BTreeMap<String, &RawValue>>(version_json)
      .map_err(JsErrorBox::from_err)?;

  output.push(b'{');
  let mut first = true;
  let mut wrote_version = false;
  for key in [
    "version",
    "bin",
    "dependencies",
    "bundleDependencies",
    "bundledDependencies",
    "optionalDependencies",
    "peerDependencies",
    "peerDependenciesMeta",
    "os",
    "cpu",
    "deprecated",
  ] {
    if let Some(value) = raw_fields.get(key) {
      if key == "version" {
        wrote_version = true;
      }
      write_raw_property(output, &mut first, key, value.get())
        .map_err(JsErrorBox::from_err)?;
    }
  }

  if !wrote_version {
    write_json_property_name(output, &mut first, "version")
      .map_err(JsErrorBox::from_err)?;
    serde_json::to_writer(&mut *output, version)
      .map_err(JsErrorBox::from_err)?;
  }

  if has_install_script(&raw_fields)? {
    output.extend_from_slice(if first {
      first = false;
      br#""hasInstallScript":true"#
    } else {
      br#","hasInstallScript":true"#
    });
  }

  if let Some(dist) = raw_fields.get("dist")
    && should_write_slim_dist(dist.get(), evidence)
  {
    write_json_property_name(output, &mut first, "dist")
      .map_err(JsErrorBox::from_err)?;
    write_slim_dist(output, dist.get(), evidence)?;
  }

  match evidence {
    Some(fast_registry_json::TrustEvidence::TrustedPublisher) => {
      output.extend_from_slice(if first {
        br#""_npmUser":{"trustedPublisher":true}"#
      } else {
        br#","_npmUser":{"trustedPublisher":true}"#
      });
    }
    Some(fast_registry_json::TrustEvidence::StagedPublish) => {
      output.extend_from_slice(if first {
        br#""_npmUser":{"approver":true}"#
      } else {
        br#","_npmUser":{"approver":true}"#
      });
    }
    Some(fast_registry_json::TrustEvidence::Provenance) | None => {}
  }

  output.push(b'}');
  Ok(())
}

fn has_install_script(
  raw_fields: &BTreeMap<String, &RawValue>,
) -> Result<bool, JsErrorBox> {
  if raw_fields
    .get("hasInstallScript")
    .is_some_and(|value| value.get() == "true")
  {
    return Ok(true);
  }
  let Some(scripts) = raw_fields.get("scripts") else {
    return Ok(false);
  };
  let scripts =
    serde_json::from_str::<BTreeMap<String, &RawValue>>(scripts.get())
      .map_err(JsErrorBox::from_err)?;
  Ok(
    scripts.contains_key("preinstall")
      || scripts.contains_key("install")
      || scripts.contains_key("postinstall"),
  )
}

fn should_write_slim_dist(
  dist_json: &str,
  evidence: Option<fast_registry_json::TrustEvidence>,
) -> bool {
  evidence
    .is_some_and(|e| e <= fast_registry_json::TrustEvidence::TrustedPublisher)
    || dist_json.contains(r#""tarball""#)
    || dist_json.contains(r#""shasum""#)
    || dist_json.contains(r#""integrity""#)
}

fn write_slim_dist(
  output: &mut Vec<u8>,
  dist_json: &str,
  evidence: Option<fast_registry_json::TrustEvidence>,
) -> Result<(), JsErrorBox> {
  let raw_fields =
    serde_json::from_str::<BTreeMap<String, &RawValue>>(dist_json)
      .map_err(JsErrorBox::from_err)?;
  output.push(b'{');
  let mut first = true;
  for key in ["tarball", "shasum", "integrity"] {
    if let Some(value) = raw_fields.get(key) {
      write_raw_property(output, &mut first, key, value.get())
        .map_err(JsErrorBox::from_err)?;
    }
  }
  if evidence
    .is_some_and(|e| e <= fast_registry_json::TrustEvidence::TrustedPublisher)
  {
    // `TrustedPublisher` already means the source had provenance too. The slim
    // cache keeps only presence markers because the resolver only ranks signal
    // presence, not the full registry attestation payload.
    output.extend_from_slice(if first {
      br#""attestations":{"provenance":true}"#
    } else {
      br#","attestations":{"provenance":true}"#
    });
  }
  output.push(b'}');
  Ok(())
}

fn write_string_map<'a>(
  output: &mut Vec<u8>,
  entries: impl Iterator<Item = (&'a &'a str, &'a &'a str)>,
) -> Result<(), serde_json::Error> {
  output.push(b'{');
  let mut first = true;
  for (key, value) in entries {
    write_json_property_name(output, &mut first, key)?;
    serde_json::to_writer(&mut *output, value)?;
  }
  output.push(b'}');
  Ok(())
}

fn write_raw_property(
  output: &mut Vec<u8>,
  first: &mut bool,
  key: &str,
  value: &str,
) -> Result<(), serde_json::Error> {
  write_json_property_name(output, first, key)?;
  output.extend_from_slice(value.as_bytes());
  Ok(())
}

fn write_json_property_name(
  output: &mut Vec<u8>,
  first: &mut bool,
  key: &str,
) -> Result<(), serde_json::Error> {
  if *first {
    *first = false;
  } else {
    output.push(b',');
  }
  serde_json::to_writer(&mut *output, key)?;
  output.push(b':');
  Ok(())
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
  #[error(
    "Error creating package sync lock file at '{path}'. Maybe try manually deleting this folder."
  )]
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
  #[error(
    "Failed setting up package cache directory for {package}, then failed cleaning it up.\n\nOriginal error:\n\n{error}\n\nRemove error:\n\n{remove_error}\n\nPlease manually delete this folder or you will run into issues using this package in the future:\n\n{output_folder}"
  )]
  SetUpPackageCacheDir {
    package: Box<PackageNv>,
    error: Box<WithFolderSyncLockError>,
    remove_error: std::io::Error,
    output_folder: PathBuf,
  },
}

fn with_folder_sync_lock(
  sys: &(impl FsCreateDirAll + FsOpen + FsRemoveDirAll + FsRemoveFile),
  package: &PackageNv,
  output_folder: &Path,
  action: impl FnOnce() -> Result<(), JsErrorBox>,
) -> Result<(), WithFolderSyncLockError> {
  fn inner(
    sys: &(impl FsCreateDirAll + FsOpen + FsRemoveFile),
    output_folder: &Path,
    action: impl FnOnce() -> Result<(), JsErrorBox>,
  ) -> Result<(), WithFolderSyncLockError> {
    sys.fs_create_dir_all(output_folder).map_err(|source| {
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
    let mut open_options = sys_traits::OpenOptions::new();
    open_options.write = true;
    open_options.create = true;
    open_options.truncate = false;
    match sys.fs_open(&sync_lock_path, &open_options) {
      Ok(_) => {
        action()?;
        // extraction succeeded, so only now delete this file
        let _ignore = sys.fs_remove_file(&sync_lock_path);
        Ok(())
      }
      Err(err) => Err(WithFolderSyncLockError::CreateLockFile {
        path: output_folder.to_path_buf(),
        source: err,
      }),
    }
  }

  match inner(sys, output_folder, action) {
    Ok(()) => Ok(()),
    Err(err) => {
      if let Err(remove_err) = sys.fs_remove_dir_all(output_folder)
        && remove_err.kind() != std::io::ErrorKind::NotFound
      {
        return Err(WithFolderSyncLockError::SetUpPackageCacheDir {
          package: Box::new(package.clone()),
          error: Box::new(err),
          remove_error: remove_err,
          output_folder: output_folder.to_path_buf(),
        });
      }
      Err(err)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn slim_package_info_bytes_round_trips_version_info_fields() {
    let input = br#"{
      "name":"pkg",
      "readme":"large",
      "dist-tags":{"latest":"1.0.0"},
      "versions":{
        "1.0.0":{
          "version":"1.0.0",
          "exports":{"./x":"./x.js"},
          "scripts":{"postinstall":"node postinstall.js","test":"node test.js"},
          "bin":{"pkg":"./bin.js"},
          "dependencies":{"dep":"^1.0.0"},
          "bundleDependencies":["bundled-dep"],
          "bundledDependencies":["bundled-alias-dep"],
          "optionalDependencies":{"optional-dep":"^2.0.0"},
          "peerDependencies":{"peer-dep":"^3.0.0"},
          "peerDependenciesMeta":{"peer-dep":{"optional":true}},
          "os":["darwin"],
          "cpu":["arm64"],
          "deprecated":"use something else",
          "dist":{
            "tarball":"https://example.com/pkg.tgz",
            "shasum":"abc123",
            "integrity":"sha512-test",
            "fileCount":100,
            "attestations":{"provenance":{"large":true}}
          },
          "_npmUser":{"trustedPublisher":{"large":true},"name":"ignored"}
        }
      },
      "time":{"1.0.0":"2026-01-01T00:00:00.000Z"}
    }"#;

    let original =
      deno_npm::registry::NpmPackageInfo::from_packument_slice(input).unwrap();
    let output = slim_package_info_bytes(input, Some("etag"), true).unwrap();
    let reparsed =
      deno_npm::registry::NpmPackageInfo::from_packument_slice(&output)
        .unwrap();
    let version = Version::parse_from_npm("1.0.0").unwrap();
    let mut expected = original.versions.get(&version).unwrap().clone();
    expected.scripts.clear();
    expected.has_install_script = Some(true);

    assert_eq!(reparsed.name, original.name);
    assert_eq!(reparsed.dist_tags, original.dist_tags);
    assert_eq!(reparsed.time, original.time);
    assert_eq!(reparsed.versions.get(&version), Some(&expected));

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let version_json = &value["versions"]["1.0.0"];

    assert_eq!(value["name"], "pkg");
    assert_eq!(value["_deno.etag"], "etag");
    assert_eq!(value["_deno.packumentFormat"], "full");
    assert_eq!(version_json["hasInstallScript"], true);
    assert_eq!(version_json["_npmUser"]["trustedPublisher"], true);
    assert!(version_json.get("exports").is_none());
    assert!(version_json.get("scripts").is_none());
    assert!(version_json["dist"].get("fileCount").is_none());
    assert!(value.get("readme").is_none());
  }

  #[test]
  fn slim_package_info_bytes_full_packument_marker() {
    // a registry response with no time data at all
    let input = br#"{
      "name":"pkg",
      "dist-tags":{"latest":"1.0.0"},
      "versions":{"1.0.0":{"version":"1.0.0"}}
    }"#;

    // abbreviated fetches don't write the marker
    let output = slim_package_info_bytes(input, None, false).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(value.get("_deno.packumentFormat").is_none());
    let (_, cache_metadata) =
      deno_npm::registry::NpmPackageInfo::from_packument_bytes_with_cache_info(
        output,
      )
      .unwrap();
    assert!(!cache_metadata.full_packument);

    // full packument fetches record the marker even when the registry
    // provides no `time` data, so it doesn't get re-fetched on every
    // process start (see #35761)
    let output = slim_package_info_bytes(input, None, true).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["_deno.packumentFormat"], "full");
    let (info, cache_metadata) =
      deno_npm::registry::NpmPackageInfo::from_packument_bytes_with_cache_info(
        output,
      )
      .unwrap();
    assert!(cache_metadata.full_packument);
    assert!(info.time.is_empty());

    // a marker nested inside a version object is ignored
    let nested = br#"{
      "name":"pkg",
      "dist-tags":{"latest":"1.0.0"},
      "versions":{"1.0.0":{"version":"1.0.0","_deno.packumentFormat":"full"}}
    }"#;
    let (_, cache_metadata) =
      deno_npm::registry::NpmPackageInfo::from_packument_bytes_with_cache_info(
        nested.to_vec(),
      )
      .unwrap();
    assert!(!cache_metadata.full_packument);
  }
}
