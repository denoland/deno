// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
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
use super::NpmPackageId;

/// For some of the tests, we want downloading of packages
/// to be deterministic so that the output is always the same
pub fn should_sync_download() -> bool {
  std::env::var("DENO_UNSTABLE_NPM_SYNC_DOWNLOAD") == Ok("1".to_string())
}

pub const NPM_PACKAGE_SYNC_LOCK_FILENAME: &str = ".deno_sync_lock";

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
        std::fs::create_dir_all(&root_dir)
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

  pub fn package_folder(
    &self,
    id: &NpmPackageId,
    registry_url: &Url,
  ) -> PathBuf {
    self
      .package_name_folder(&id.name, registry_url)
      .join(id.version.to_string())
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    let mut dir = self.registry_folder(registry_url);
    let mut parts = name.split('/').map(Cow::Borrowed).collect::<Vec<_>>();
    // package names were not always enforced to be lowercase and so we need
    // to ensure package names, which are therefore case sensitive, are stored
    // on a case insensitive file system to not have conflicts. We do this by
    // first putting it in a "_" folder then hashing the package name.
    if name.to_lowercase() != name {
      let last_part = parts.last_mut().unwrap();
      *last_part = Cow::Owned(crate::checksum::gen(&[last_part.as_bytes()]));
      // We can't just use the hash as part of the directory because it may
      // have a collision with an actual package name in case someone wanted
      // to name an actual package that. To get around this, put all these
      // in a folder called "_" since npm packages can't start with an underscore
      // and there is no package currently called just "_".
      dir = dir.join("_");
    }
    // ensure backslashes are used on windows
    for part in parts {
      dir = dir.join(&*part);
    }
    dir
  }

  pub fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self
      .root_dir
      .join(fs_util::root_url_to_safe_local_dirname(registry_url))
  }

  pub fn resolve_package_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Result<NpmPackageId, AnyError> {
    match self.maybe_resolve_package_id_from_specifier(specifier, registry_url)
    {
      Some(id) => Ok(id),
      None => bail!("could not find npm package for '{}'", specifier),
    }
  }

  fn maybe_resolve_package_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Option<NpmPackageId> {
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
    let relative_url = registry_root_dir.make_relative(specifier)?;
    if relative_url.starts_with("../") {
      return None;
    }

    // examples:
    // * chalk/5.0.1/
    // * @types/chalk/5.0.1/
    let is_scoped_package = relative_url.starts_with('@');
    let mut parts = relative_url
      .split('/')
      .enumerate()
      .take(if is_scoped_package { 3 } else { 2 })
      .map(|(_, part)| part)
      .collect::<Vec<_>>();
    let version = parts.pop().unwrap();
    let name = parts.join("/");

    Some(NpmPackageId {
      name,
      version: NpmVersion::parse(version).unwrap(),
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
    id: &NpmPackageId,
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    self
      .ensure_package_inner(id, dist, registry_url)
      .await
      .with_context(|| format!("Failed caching npm package '{}'.", id))
  }

  async fn ensure_package_inner(
    &self,
    id: &NpmPackageId,
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    let package_folder = self.readonly.package_folder(id, registry_url);
    if package_folder.exists()
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
      && self.cache_setting.should_use_for_npm_package(&id.name)
    {
      return Ok(());
    } else if self.cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "An npm specifier not found in cache: \"{}\", --cached-only is specified.",
          id.name
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

      match verify_and_extract_tarball(id, &bytes, dist, &package_folder) {
        Ok(()) => Ok(()),
        Err(err) => {
          if let Err(remove_err) = fs::remove_dir_all(&package_folder) {
            if remove_err.kind() != std::io::ErrorKind::NotFound {
              bail!(
                concat!(
                  "Failed verifying and extracting npm tarball for {}, then ",
                  "failed cleaning up package cache folder.\n\nOriginal ",
                  "error:\n\n{}\n\nRemove error:\n\n{}\n\nPlease manually ",
                  "delete this folder or you will run into issues using this ",
                  "package in the future:\n\n{}"
                ),
                id,
                err,
                remove_err,
                package_folder.display(),
              );
            }
          }
          Err(err)
        }
      }
    }
  }

  pub fn package_folder(
    &self,
    id: &NpmPackageId,
    registry_url: &Url,
  ) -> PathBuf {
    self.readonly.package_folder(id, registry_url)
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    self.readonly.package_name_folder(name, registry_url)
  }

  pub fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self.readonly.registry_folder(registry_url)
  }

  pub fn resolve_package_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Result<NpmPackageId, AnyError> {
    self
      .readonly
      .resolve_package_id_from_specifier(specifier, registry_url)
  }
}

#[cfg(test)]
mod test {
  use deno_core::url::Url;

  use super::ReadonlyNpmCache;
  use crate::npm::semver::NpmVersion;
  use crate::npm::NpmPackageId;

  #[test]
  fn should_get_lowercase_package_folder() {
    let root_dir = crate::deno_dir::DenoDir::new(None).unwrap().root;
    let cache = ReadonlyNpmCache::new(root_dir.clone());
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();

    // all lowercase should be as-is
    assert_eq!(
      cache.package_folder(
        &NpmPackageId {
          name: "json".to_string(),
          version: NpmVersion::parse("1.2.5").unwrap(),
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5"),
    );
  }

  #[test]
  fn should_handle_non_all_lowercase_package_names() {
    // it was possible at one point for npm packages to not just be lowercase
    let root_dir = crate::deno_dir::DenoDir::new(None).unwrap().root;
    let cache = ReadonlyNpmCache::new(root_dir.clone());
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();
    let json_uppercase_hash =
      "db1a21a0bc2ef8fbe13ac4cf044e8c9116d29137d5ed8b916ab63dcb2d4290df";
    assert_eq!(
      cache.package_folder(
        &NpmPackageId {
          name: "JSON".to_string(),
          version: NpmVersion::parse("1.2.5").unwrap(),
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_")
        .join(json_uppercase_hash)
        .join("1.2.5"),
    );
    assert_eq!(
      cache.package_folder(
        &NpmPackageId {
          name: "@types/JSON".to_string(),
          version: NpmVersion::parse("1.2.5").unwrap(),
        },
        &registry_url,
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("_")
        .join("@types")
        .join(json_uppercase_hash)
        .join("1.2.5"),
    );
  }
}
