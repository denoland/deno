// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmResolutionPackage;
use deno_npm_cache::NpmCache;
use deno_npm_cache::NpmCacheSys;
use deno_resolver::workspace::WorkspaceNpmPatchPackages;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

mod bin_entries;
mod flag;
mod fs;
mod global;
mod lifecycle_scripts;
mod local;
pub mod package_json;

pub use bin_entries::BinEntries;
pub use bin_entries::BinEntriesError;
use parking_lot::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCaching<'a> {
  Only(Cow<'a, [PackageReq]>),
  All,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
/// The set of npm packages that are allowed to run lifecycle scripts.
pub enum PackagesAllowedScripts {
  All,
  Some(Vec<String>),
  #[default]
  None,
}

/// Info needed to run NPM lifecycle scripts
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct LifecycleScriptsConfig {
  pub allowed: PackagesAllowedScripts,
  pub initial_cwd: PathBuf,
  pub root_dir: PathBuf,
  /// Part of an explicit `deno install`
  pub explicit_install: bool,
}

/// Part of the resolution that interacts with the file system.
#[async_trait::async_trait(?Send)]
pub(crate) trait NpmPackageFsInstaller:
  std::fmt::Debug + Send + Sync
{
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox>;
}

pub struct CachedNpmPackageExtraInfoProvider<TSys: NpmCacheSys> {
  inner: Arc<NpmPackageExtraInfoProvider<TSys>>,
  cache: RwLock<rustc_hash::FxHashMap<PackageNv, NpmPackageExtraInfo>>,
}

impl<TSys: NpmCacheSys> CachedNpmPackageExtraInfoProvider<TSys> {
  pub fn new(inner: Arc<NpmPackageExtraInfoProvider<TSys>>) -> Self {
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

pub struct NpmPackageExtraInfoProvider<TSys: NpmCacheSys> {
  npm_cache: Arc<NpmCache<TSys>>,
  npm_registry_info_provider: Arc<dyn NpmRegistryApi + Send + Sync>,
  workspace_patch_packages: Arc<WorkspaceNpmPatchPackages>,
}

impl<TSys: NpmCacheSys> std::fmt::Debug for NpmPackageExtraInfoProvider<TSys> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NpmPackageExtraInfoProvider").finish()
  }
}

impl<TSys: NpmCacheSys> NpmPackageExtraInfoProvider<TSys> {
  pub fn new(
    npm_cache: Arc<NpmCache<TSys>>,
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

impl<TSys: NpmCacheSys> NpmPackageExtraInfoProvider<TSys> {
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
      deno_unsync::spawn_blocking(move || {
        let package_json = std::fs::read_to_string(&package_json_path)
          .map_err(JsErrorBox::from_err)?;
        let extra_info: NpmPackageExtraInfo =
          serde_json::from_str(&package_json).map_err(JsErrorBox::from_err)?;

        Ok::<_, JsErrorBox>(extra_info)
      })
      .await
      .map_err(JsErrorBox::from_err)??;
    Ok(extra_info)
  }
}
