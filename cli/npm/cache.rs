// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fs;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_runtime::colors;
use deno_runtime::deno_fetch::reqwest;

use crate::deno_dir::DenoDir;
use crate::fs_util;

use super::tarball::verify_and_extract_tarball;
use super::NpmPackageId;
use super::NpmPackageVersionDistInfo;

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
    let mut dir = self
      .root_dir
      .join(fs_util::root_url_to_safe_local_dirname(registry_url));
    // ensure backslashes are used on windows
    for part in name.split('/') {
      dir = dir.join(part);
    }
    dir
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
      version: semver::Version::parse(version).unwrap(),
    })
  }
}

/// Stores a single copy of npm packages in a cache.
#[derive(Clone, Debug)]
pub struct NpmCache(ReadonlyNpmCache);

impl NpmCache {
  pub fn new(root_dir: PathBuf) -> Self {
    Self(ReadonlyNpmCache::new(root_dir))
  }

  pub fn from_deno_dir(dir: &DenoDir) -> Self {
    Self(ReadonlyNpmCache::from_deno_dir(dir))
  }

  pub fn as_readonly(&self) -> ReadonlyNpmCache {
    self.0.clone()
  }

  pub async fn ensure_package(
    &self,
    id: &NpmPackageId,
    dist: &NpmPackageVersionDistInfo,
    registry_url: &Url,
  ) -> Result<(), AnyError> {
    let package_folder = self.0.package_folder(id, registry_url);
    if package_folder.exists()
      // if this file exists, then the package didn't successfully extract
      // the first time, or another process is currently extracting the zip file
      && !package_folder.join(NPM_PACKAGE_SYNC_LOCK_FILENAME).exists()
    {
      return Ok(());
    }

    log::log!(
      log::Level::Info,
      "{} {}",
      colors::green("Download"),
      dist.tarball,
    );

    let response = reqwest::get(&dist.tarball).await?;

    if response.status() == 404 {
      bail!("Could not find npm package tarball at: {}", dist.tarball);
    } else if !response.status().is_success() {
      bail!("Bad response: {:?}", response.status());
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
    self.0.package_folder(id, registry_url)
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    self.0.package_name_folder(name, registry_url)
  }

  pub fn resolve_package_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
    registry_url: &Url,
  ) -> Result<NpmPackageId, AnyError> {
    self
      .0
      .resolve_package_id_from_specifier(specifier, registry_url)
  }
}
