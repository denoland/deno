// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_runtime::deno_fetch::reqwest;

use crate::deno_dir::DenoDir;
use crate::file_fetcher::CacheSetting;
use crate::fs_util;
use crate::progress_bar::ProgressBar;

use super::registry::NpmPackageVersionDistInfo;
use super::semver::NpmVersion;
use super::tarball::verify_and_extract_tarball;

/// For some of the tests, we want downloading of packages
/// to be deterministic so that the output is always the same
pub fn should_sync_download() -> bool {
  std::env::var("DENO_UNSTABLE_NPM_SYNC_DOWNLOAD").is_ok()
}

const NPM_PACKAGE_SYNC_LOCK_FILENAME: &str = ".deno_sync_lock";

pub fn with_folder_sync_lock(
  package: (&str, &NpmVersion),
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
              "Failed setting up package cache directory for {}@{}, then ",
              "failed cleaning it up.\n\nOriginal error:\n\n{}\n\n",
              "Remove error:\n\n{}\n\nPlease manually ",
              "delete this folder or you will run into issues using this ",
              "package in the future:\n\n{}"
            ),
            package.0,
            package.1,
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

pub struct NpmPackageCacheFolderId {
  pub name: String,
  pub version: NpmVersion,
  /// Peer dependency resolution may require us to have duplicate copies
  /// of the same package.
  pub copy_index: usize,
}

impl NpmPackageCacheFolderId {
  pub fn with_no_count(&self) -> Self {
    Self {
      name: self.name.clone(),
      version: self.version.clone(),
      copy_index: 0,
    }
  }
}

impl std::fmt::Display for NpmPackageCacheFolderId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}@{}", self.name, self.version)?;
    if self.copy_index > 0 {
      write!(f, "_{}", self.copy_index)?;
    }
    Ok(())
  }
}

#[derive(Clone, Debug)]
pub struct ReadonlyNpmCache {
  root_dir: PathBuf,
  // cached url representation of the root directory
  root_dir_url: Url,
}

// todo(dsherret): implementing Default for this is error prone because someone
// might accidentally use the default implementation instead of getting the
// correct location of the deno dir, which might be provided via a CLI argument.
// That said, the rest of the LSP code does this at the moment and so this code
// copies that.
impl Default for ReadonlyNpmCache {
  fn default() -> Self {
    // This only gets used when creating the tsc runtime and for testing, and so
    // it shouldn't ever actually access the DenoDir, so it doesn't support a
    // custom root.
    Self::from_deno_dir(&crate::deno_dir::DenoDir::new(None).unwrap())
  }
}

impl ReadonlyNpmCache {
  pub fn new(root_dir: PathBuf) -> Self {
    fn try_get_canonicalized_root_dir(
      root_dir: &Path,
    ) -> Result<PathBuf, AnyError> {
      if !root_dir.exists() {
        std::fs::create_dir_all(root_dir)
          .with_context(|| format!("Error creating {}", root_dir.display()))?;
      }
      Ok(crate::fs_util::canonicalize_path(root_dir)?)
    }

    // this may fail on readonly file systems, so just ignore if so
    let root_dir =
      try_get_canonicalized_root_dir(&root_dir).unwrap_or(root_dir);
    let root_dir_url = Url::from_directory_path(&root_dir).unwrap();
    Self {
      root_dir,
      root_dir_url,
    }
  }

  pub fn from_deno_dir(dir: &DenoDir) -> Self {
    Self::new(dir.root.join("npm"))
  }

  pub fn package_folder_for_id(
    &self,
    id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> PathBuf {
    if id.copy_index == 0 {
      self.package_folder_for_name_and_version(
        &id.name,
        &id.version,
        registry_url,
      )
    } else {
      self
        .package_name_folder(&id.name, registry_url)
        .join(format!("{}_{}", id.version, id.copy_index))
    }
  }

  pub fn package_folder_for_name_and_version(
    &self,
    name: &str,
    version: &NpmVersion,
    registry_url: &Url,
  ) -> PathBuf {
    self
      .package_name_folder(name, registry_url)
      .join(version.to_string())
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    let mut dir = self.registry_folder(registry_url);
    if name.to_lowercase() != name {
      let encoded_name = mixed_case_package_name_encode(name);
      // Using the encoded directory may have a collision with an actual package name
      // so prefix it with an underscore since npm packages can't start with that
      dir.join(format!("_{}", encoded_name))
    } else {
      // ensure backslashes are used on windows
      for part in name.split('/') {
        dir = dir.join(part);
      }
      dir
    }
  }

  pub fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self
      .root_dir
      .join(fs_util::root_url_to_safe_local_dirname(registry_url))
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Result<NpmPackageCacheFolderId, AnyError> {
    match self
      .maybe_resolve_package_folder_id_from_specifier(specifier, registry_url)
    {
      Some(id) => Ok(id),
      None => bail!("could not find npm package for '{}'", specifier),
    }
  }

  fn maybe_resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Option<NpmPackageCacheFolderId> {
    let registry_root_dir = self
      .root_dir_url
      .join(&format!(
        "{}/",
        fs_util::root_url_to_safe_local_dirname(registry_url)
          .to_string_lossy()
          .replace('\\', "/")
      ))
      // this not succeeding indicates a fatal issue, so unwrap
      .unwrap();
    let mut relative_url = registry_root_dir.make_relative(specifier)?;
    if relative_url.starts_with("../") {
      return None;
    }

    // base32 decode the url if it starts with an underscore
    // * Ex. _{base32(package_name)}/
    if let Some(end_url) = relative_url.strip_prefix('_') {
      let mut parts = end_url
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
      match mixed_case_package_name_decode(&parts[0]) {
        Some(part) => {
          parts[0] = part;
        }
        None => return None,
      }
      relative_url = parts.join("/");
    }

    // examples:
    // * chalk/5.0.1/
    // * @types/chalk/5.0.1/
    // * some-package/5.0.1_1/ -- where the `_1` (/_\d+/) is a copy of the folder for peer deps
    let is_scoped_package = relative_url.starts_with('@');
    let mut parts = relative_url
      .split('/')
      .enumerate()
      .take(if is_scoped_package { 3 } else { 2 })
      .map(|(_, part)| part)
      .collect::<Vec<_>>();
    if parts.len() < 2 {
      return None;
    }
    let version_part = parts.pop().unwrap();
    let name = parts.join("/");
    let (version, copy_index) =
      if let Some((version, copy_count)) = version_part.split_once('_') {
        (version, copy_count.parse::<usize>().ok()?)
      } else {
        (version_part, 0)
      };
    Some(NpmPackageCacheFolderId {
      name,
      version: NpmVersion::parse(version).ok()?,
      copy_index,
    })
  }

  pub fn get_cache_location(&self) -> PathBuf {
    self.root_dir.clone()
  }
}

/// Stores a single copy of npm packages in a cache.
#[derive(Clone, Debug)]
pub struct NpmCache {
  readonly: ReadonlyNpmCache,
  cache_setting: CacheSetting,
  progress_bar: ProgressBar,
}

impl NpmCache {
  pub fn from_deno_dir(
    dir: &DenoDir,
    cache_setting: CacheSetting,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      readonly: ReadonlyNpmCache::from_deno_dir(dir),
      cache_setting,
      progress_bar,
    }
  }

  pub fn as_readonly(&self) -> ReadonlyNpmCache {
    self.readonly.clone()
  }

  pub async fn ensure_package(
    &self,
    package: (&str, &NpmVersion),
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    self
      .ensure_package_inner(package, dist, registry_url)
      .await
      .with_context(|| {
        format!("Failed caching npm package '{}@{}'.", package.0, package.1)
      })
  }

  pub fn should_use_cache_for_npm_package(&self, package_name: &str) -> bool {
    self.cache_setting.should_use_for_npm_package(package_name)
  }

  async fn ensure_package_inner(
    &self,
    package: (&str, &NpmVersion),
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    let package_folder = self.readonly.package_folder_for_name_and_version(
      package.0,
      package.1,
      registry_url,
    );
    if package_folder.exists()
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
      && self.should_use_cache_for_npm_package(package.0)
    {
      return Ok(());
    } else if self.cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{}\", --cached-only is specified.",
          &package.0
        )
      )
      );
    }

    let _guard = self.progress_bar.update(&dist.tarball);
    let response = reqwest::get(&dist.tarball).await?;

    if response.status() == 404 {
      bail!("Could not find npm package tarball at: {}", dist.tarball);
    } else if !response.status().is_success() {
      let status = response.status();
      let maybe_response_text = response.text().await.ok();
      bail!(
        "Bad response: {:?}{}",
        status,
        match maybe_response_text {
          Some(text) => format!("\n\n{}", text),
          None => String::new(),
        }
      );
    } else {
      let bytes = response.bytes().await?;

      verify_and_extract_tarball(package, &bytes, dist, &package_folder)
    }
  }

  /// Ensures a copy of the package exists in the global cache.
  ///
  /// This assumes that the original package folder being hard linked
  /// from exists before this is called.
  pub fn ensure_copy_package(
    &self,
    id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    assert_ne!(id.copy_index, 0);
    let package_folder = self.readonly.package_folder_for_id(id, registry_url);

    if package_folder.exists()
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
      && self.cache_setting.should_use_for_npm_package(&id.name)
    {
      return Ok(());
    }

    let original_package_folder = self
      .readonly
      .package_folder_for_name_and_version(&id.name, &id.version, registry_url);
    with_folder_sync_lock(
      (id.name.as_str(), &id.version),
      &package_folder,
      || {
        fs_util::hard_link_dir_recursive(
          &original_package_folder,
          &package_folder,
        )
      },
    )?;
    Ok(())
  }

  pub fn package_folder_for_id(
    &self,
    id: &NpmPackageCacheFolderId,
    registry_url: &Url,
  ) -> PathBuf {
    self.readonly.package_folder_for_id(id, registry_url)
  }

  pub fn package_folder_for_name_and_version(
    &self,
    name: &str,
    version: &NpmVersion,
    registry_url: &Url,
  ) -> PathBuf {
    self.readonly.package_folder_for_name_and_version(
      name,
      version,
      registry_url,
    )
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    self.readonly.package_name_folder(name, registry_url)
  }

  pub fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self.readonly.registry_folder(registry_url)
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Result<NpmPackageCacheFolderId, AnyError> {
    self
      .readonly
      .resolve_package_folder_id_from_specifier(specifier, registry_url)
  }
}

pub fn mixed_case_package_name_encode(name: &str) -> String {
  // use base32 encoding because it's reversable and the character set
  // only includes the characters within 0-9 and A-Z so it can be lower cased
  base32::encode(
    base32::Alphabet::RFC4648 { padding: false },
    name.as_bytes(),
  )
  .to_lowercase()
}

pub fn mixed_case_package_name_decode(name: &str) -> Option<String> {
  base32::decode(base32::Alphabet::RFC4648 { padding: false }, name)
    .and_then(|b| String::from_utf8(b).ok())
}

#[cfg(test)]
mod test {
  use deno_core::url::Url;

  use super::ReadonlyNpmCache;
  use crate::npm::cache::NpmPackageCacheFolderId;
  use crate::npm::semver::NpmVersion;

  #[test]
  fn should_get_package_folder() {
    let root_dir = crate::deno_dir::DenoDir::new(None).unwrap().root;
    let cache = ReadonlyNpmCache::new(root_dir.clone());
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          name: "json".to_string(),
          version: NpmVersion::parse("1.2.5").unwrap(),
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          name: "json".to_string(),
          version: NpmVersion::parse("1.2.5").unwrap(),
          copy_index: 1,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5_1"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          name: "JSON".to_string(),
          version: NpmVersion::parse("2.1.5").unwrap(),
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_jjju6tq")
        .join("2.1.5"),
    );

    assert_eq!(
      cache.package_folder_for_id(
        &NpmPackageCacheFolderId {
          name: "@types/JSON".to_string(),
          version: NpmVersion::parse("2.1.5").unwrap(),
          copy_index: 0,
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_ib2hs4dfomxuuu2pjy")
        .join("2.1.5"),
    );
  }
}
