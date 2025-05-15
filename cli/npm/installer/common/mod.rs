// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::parking_lot::RwLock;
use deno_error::JsErrorBox;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmResolutionPackage;
use deno_semver::package::PackageNv;
pub use deno_task_executor::DenoTaskLifeCycleScriptsExecutor;

use super::PackageCaching;
use crate::npm::CliNpmCache;
use crate::npm::WorkspaceNpmPatchPackages;

pub mod bin_entries;
mod deno_task_executor;
pub mod lifecycle_scripts;

/// Part of the resolution that interacts with the file system.
#[async_trait(?Send)]
pub trait NpmPackageFsInstaller: std::fmt::Debug + Send + Sync {
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox>;
}

pub struct CachedNpmPackageExtraInfoProvider {
  inner: Arc<NpmPackageExtraInfoProvider>,
  cache: RwLock<rustc_hash::FxHashMap<PackageNv, NpmPackageExtraInfo>>,
}

impl CachedNpmPackageExtraInfoProvider {
  pub fn new(inner: Arc<NpmPackageExtraInfoProvider>) -> Self {
    Self {
      inner,
      cache: Default::default(),
    }
  }

  pub async fn get_package_extra_info(
    &self,
    package_nv: &PackageNv,
    package_path: &Path,
    expected: ExpectedExtraInfo,
  ) -> Result<NpmPackageExtraInfo, JsErrorBox> {
    if let Some(extra_info) = self.cache.read().get(package_nv) {
      return Ok(extra_info.clone());
    }

    let extra_info = self
      .inner
      .get_package_extra_info(package_nv, package_path, expected)
      .await?;
    self
      .cache
      .write()
      .insert(package_nv.clone(), extra_info.clone());
    Ok(extra_info)
  }
}

pub struct NpmPackageExtraInfoProvider {
  npm_cache: Arc<CliNpmCache>,
  npm_registry_info_provider: Arc<dyn NpmRegistryApi + Send + Sync>,
  workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
}

impl std::fmt::Debug for NpmPackageExtraInfoProvider {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NpmPackageExtraInfoProvider")
      .field("npm_cache", &self.npm_cache)
      .finish()
  }
}

impl NpmPackageExtraInfoProvider {
  pub fn new(
    npm_cache: Arc<CliNpmCache>,
    npm_registry_info_provider: Arc<dyn NpmRegistryApi + Send + Sync>,
    workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
  ) -> Self {
    Self {
      npm_cache,
      npm_registry_info_provider,
      workspace_patch_packages,
    }
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExpectedExtraInfo {
  pub deprecated: bool,
  pub bin: bool,
  pub scripts: bool,
}

impl ExpectedExtraInfo {
  pub fn from_package(package: &NpmResolutionPackage) -> Self {
    Self {
      deprecated: package.is_deprecated,
      bin: package.has_bin,
      scripts: package.has_scripts,
    }
  }
}

impl NpmPackageExtraInfoProvider {
  pub async fn get_package_extra_info(
    &self,
    package_nv: &PackageNv,
    package_path: &Path,
    expected: ExpectedExtraInfo,
  ) -> Result<NpmPackageExtraInfo, JsErrorBox> {
    if expected.deprecated {
      // we need the registry version info to get the deprecated string, since it's not in the
      // package's package.json
      self.fetch_from_registry(package_nv).await
    } else {
      match self.fetch_from_package_json(package_path).await {
        Ok(extra_info) => {
          // some packages have bin in registry but not in package.json (e.g. esbuild-wasm)
          // still not sure how that happens
          if (expected.bin && extra_info.bin.is_none())
            || (expected.scripts && extra_info.scripts.is_empty())
          {
            self.fetch_from_registry(package_nv).await
          } else {
            Ok(extra_info)
          }
        }
        Err(err) => {
          log::debug!(
            "failed to get extra info for {} from package.json at {}: {}",
            package_nv,
            package_path.join("package.json").display(),
            err
          );
          self.fetch_from_registry(package_nv).await
        }
      }
    }
  }

  async fn fetch_from_registry(
    &self,
    package_nv: &PackageNv,
  ) -> Result<NpmPackageExtraInfo, JsErrorBox> {
    let package_info = self
      .npm_registry_info_provider
      .package_info(&package_nv.name)
      .await
      .map_err(JsErrorBox::from_err)?;
    let version_info = package_info
      .version_info(package_nv, &self.workspace_patch_packages.0)
      .map_err(JsErrorBox::from_err)?;
    Ok(NpmPackageExtraInfo {
      deprecated: version_info.deprecated.clone(),
      bin: version_info.bin.clone(),
      scripts: version_info.scripts.clone(),
    })
  }

  async fn fetch_from_package_json(
    &self,
    package_path: &Path,
  ) -> Result<NpmPackageExtraInfo, JsErrorBox> {
    let package_json_path = package_path.join("package.json");
    let extra_info: NpmPackageExtraInfo =
      deno_core::unsync::spawn_blocking(move || {
        let package_json = std::fs::read_to_string(&package_json_path)
          .map_err(JsErrorBox::from_err)?;
        let extra_info: NpmPackageExtraInfo =
          deno_core::serde_json::from_str(&package_json)
            .map_err(JsErrorBox::from_err)?;

        Ok::<_, JsErrorBox>(extra_info)
      })
      .await
      .map_err(JsErrorBox::from_err)??;
    Ok(extra_info)
  }
}
