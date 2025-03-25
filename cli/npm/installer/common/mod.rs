// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::parking_lot::RwLock;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::NpmPackageExtraInfo;
use deno_semver::package::PackageNv;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use deno_error::JsErrorBox;

use crate::npm::CliNpmCache;

use super::PackageCaching;

pub mod bin_entries;
pub mod lifecycle_scripts;

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsInstaller: std::fmt::Debug + Send + Sync {
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox>;
}

pub trait NpmPackageExtraInfoProvider: std::fmt::Debug + Send + Sync {
  async fn get_package_extra_info(
    &self,
    package_id: &PackageNv,
    is_deprecated: bool,
  ) -> Result<deno_npm::NpmPackageExtraInfo, JsErrorBox>;
}

impl<T: NpmPackageExtraInfoProvider + ?Sized> NpmPackageExtraInfoProvider
  for Arc<T>
{
  async fn get_package_extra_info(
    &self,
    package_id: &PackageNv,
    is_deprecated: bool,
  ) -> Result<deno_npm::NpmPackageExtraInfo, JsErrorBox> {
    self
      .as_ref()
      .get_package_extra_info(package_id, is_deprecated)
      .await
  }
}

pub struct ExtraInfoProvider {
  npm_cache: Arc<CliNpmCache>,
  npm_registry_info_provider: Arc<dyn NpmRegistryApi + Send + Sync>,
  cache: RwLock<rustc_hash::FxHashMap<PackageNv, NpmPackageExtraInfo>>,
}

impl std::fmt::Debug for ExtraInfoProvider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ExtraInfoProvider")
      .field("npm_cache", &self.npm_cache)
      .field("cache", &self.cache)
      .finish()
  }
}

impl ExtraInfoProvider {
  pub fn new(
    npm_cache: Arc<CliNpmCache>,
    npm_registry_info_provider: Arc<dyn NpmRegistryApi + Send + Sync>,
  ) -> Self {
    Self {
      npm_cache,
      npm_registry_info_provider,
      cache: RwLock::new(rustc_hash::FxHashMap::default()),
    }
  }
}

impl super::common::NpmPackageExtraInfoProvider for ExtraInfoProvider {
  async fn get_package_extra_info(
    &self,
    package_nv: &PackageNv,
    deprecated: bool,
  ) -> Result<NpmPackageExtraInfo, JsErrorBox> {
    if let Some(extra_info) = self.cache.read().get(package_nv) {
      return Ok(extra_info.clone());
    }

    if deprecated {
      // we need the registry version info to get the deprecated string, since it's not in the
      // package's package.json
      let package_info = self
        .npm_registry_info_provider
        .package_info(&package_nv.name)
        .await
        .map_err(JsErrorBox::from_err)?;
      let patched_packages = HashMap::new();
      let version_info = package_info
        .version_info(&package_nv, &patched_packages)
        .map_err(JsErrorBox::from_err)?;
      Ok(NpmPackageExtraInfo {
        deprecated: version_info.deprecated.clone(),
        bin: version_info.bin.clone(),
        scripts: version_info.scripts.clone(),
      })
    } else {
      let folder_path = self.npm_cache.package_folder_for_nv(package_nv);
      let package_json_path = folder_path.join("package.json");
      let package_json = std::fs::read_to_string(&package_json_path)
        .map_err(JsErrorBox::from_err)?;
      let extra_info: NpmPackageExtraInfo =
        deno_core::serde_json::from_str(&package_json)
          .map_err(JsErrorBox::from_err)?;
      self
        .cache
        .write()
        .insert(package_nv.clone(), extra_info.clone());
      Ok(extra_info)
    }
  }
}
