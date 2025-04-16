// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_error::JsErrorBox;
use deno_lib::util::hash::FastInsecureHasher;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_resolver::npm::managed::NpmResolutionCell;

use super::common::lifecycle_scripts::LifecycleScriptsStrategy;
use super::common::NpmPackageFsInstaller;
use super::PackageCaching;
use crate::args::LifecycleScriptsConfig;
use crate::colors;
use crate::npm::CliNpmCache;
use crate::npm::CliNpmTarballCache;

/// Resolves packages from the global npm cache.
pub struct GlobalNpmPackageInstaller {
  cache: Arc<CliNpmCache>,
  tarball_cache: Arc<CliNpmTarballCache>,
  resolution: Arc<NpmResolutionCell>,
  lifecycle_scripts: LifecycleScriptsConfig,
  system_info: NpmSystemInfo,
}

impl std::fmt::Debug for GlobalNpmPackageInstaller {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("GlobalNpmPackageInstaller")
      .field("cache", &self.cache)
      .field("tarball_cache", &self.tarball_cache)
      .field("resolution", &self.resolution)
      .field("lifecycle_scripts", &self.lifecycle_scripts)
      .field("system_info", &self.system_info)
      .finish()
  }
}

impl GlobalNpmPackageInstaller {
  pub fn new(
    cache: Arc<CliNpmCache>,
    tarball_cache: Arc<CliNpmTarballCache>,
    resolution: Arc<NpmResolutionCell>,
    lifecycle_scripts: LifecycleScriptsConfig,
    system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      cache,
      tarball_cache,
      resolution,
      lifecycle_scripts,
      system_info,
    }
  }
}

#[async_trait(?Send)]
impl NpmPackageFsInstaller for GlobalNpmPackageInstaller {
  async fn cache_packages<'a>(
    &self,
    caching: PackageCaching<'a>,
  ) -> Result<(), JsErrorBox> {
    let package_partitions = match caching {
      PackageCaching::All => self
        .resolution
        .all_system_packages_partitioned(&self.system_info),
      PackageCaching::Only(reqs) => self
        .resolution
        .subset(&reqs)
        .all_system_packages_partitioned(&self.system_info),
    };
    cache_packages(&package_partitions.packages, &self.tarball_cache)
      .await
      .map_err(JsErrorBox::from_err)?;

    // create the copy package folders
    for copy in package_partitions.copy_packages {
      self
        .cache
        .ensure_copy_package(&copy.get_package_cache_folder_id())
        .map_err(JsErrorBox::from_err)?;
    }

    let mut lifecycle_scripts =
      super::common::lifecycle_scripts::LifecycleScripts::new(
        &self.lifecycle_scripts,
        GlobalLifecycleScripts::new(self, &self.lifecycle_scripts.root_dir),
      );

    // For the global cache, we don't run scripts so we just care that there _are_
    // scripts. Kind of hacky, but avoids fetching the "extra" info from the registry.
    let extra = deno_npm::NpmPackageExtraInfo {
      deprecated: None,
      bin: None,
      scripts: [("postinstall".into(), "".into())].into_iter().collect(),
    };

    for package in &package_partitions.packages {
      if package.has_scripts {
        let package_folder = self.cache.package_folder_for_nv(&package.id.nv);
        lifecycle_scripts.add(package, &extra, Cow::Borrowed(&package_folder));
      }
    }

    lifecycle_scripts
      .warn_not_run_scripts()
      .map_err(JsErrorBox::from_err)?;

    Ok(())
  }
}

async fn cache_packages(
  packages: &[NpmResolutionPackage],
  tarball_cache: &Arc<CliNpmTarballCache>,
) -> Result<(), deno_npm_cache::EnsurePackageError> {
  let mut futures_unordered = FuturesUnordered::new();
  for package in packages {
    if let Some(dist) = &package.dist {
      futures_unordered.push(async move {
        tarball_cache.ensure_package(&package.id.nv, dist).await
      });
    }
  }
  while let Some(result) = futures_unordered.next().await {
    // surface the first error
    result?;
  }
  Ok(())
}

struct GlobalLifecycleScripts<'a> {
  installer: &'a GlobalNpmPackageInstaller,
  path_hash: u64,
}

impl<'a> GlobalLifecycleScripts<'a> {
  fn new(installer: &'a GlobalNpmPackageInstaller, root_dir: &Path) -> Self {
    let mut hasher = FastInsecureHasher::new_without_deno_version();
    hasher.write(root_dir.to_string_lossy().as_bytes());
    let path_hash = hasher.finish();
    Self {
      installer,
      path_hash,
    }
  }

  fn warned_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    self
      .package_path(package)
      .join(format!(".scripts-warned-{}", self.path_hash))
  }
}

impl super::common::lifecycle_scripts::LifecycleScriptsStrategy
  for GlobalLifecycleScripts<'_>
{
  fn can_run_scripts(&self) -> bool {
    false
  }
  fn package_path(&self, package: &NpmResolutionPackage) -> PathBuf {
    self.installer.cache.package_folder_for_nv(&package.id.nv)
  }

  fn warn_on_scripts_not_run(
    &self,
    packages: &[(&NpmResolutionPackage, PathBuf)],
  ) -> std::result::Result<(), std::io::Error> {
    log::warn!("{} The following packages contained npm lifecycle scripts ({}) that were not executed:", colors::yellow("Warning"), colors::gray("preinstall/install/postinstall"));
    for (package, _) in packages {
      log::warn!("┠─ {}", colors::gray(format!("npm:{}", package.id.nv)));
    }
    log::warn!("┃");
    log::warn!(
      "┠─ {}",
      colors::italic("This may cause the packages to not work correctly.")
    );
    log::warn!("┠─ {}", colors::italic("Lifecycle scripts are only supported when using a `node_modules` directory."));
    log::warn!(
      "┠─ {}",
      colors::italic("Enable it in your deno config file:")
    );
    log::warn!("┖─ {}", colors::bold("\"nodeModulesDir\": \"auto\""));

    for (package, _) in packages {
      std::fs::write(self.warned_scripts_file(package), "")?;
    }
    Ok(())
  }

  fn did_run_scripts(
    &self,
    _package: &NpmResolutionPackage,
  ) -> Result<(), std::io::Error> {
    Ok(())
  }

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool {
    self.warned_scripts_file(package).exists()
  }

  fn has_run(&self, _package: &NpmResolutionPackage) -> bool {
    false
  }
}
