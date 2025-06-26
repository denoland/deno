// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_npm_cache::NpmCache;
use deno_npm_cache::NpmCacheHttpClient;
use deno_npm_cache::NpmCacheSys;
use deno_npm_cache::TarballCache;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_terminal::colors;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use sys_traits::OpenOptions;

use crate::lifecycle_scripts::LifecycleScripts;
use crate::lifecycle_scripts::LifecycleScriptsStrategy;
use crate::LifecycleScriptsConfig;
use crate::NpmPackageFsInstaller;
use crate::PackageCaching;

/// Resolves packages from the global npm cache.
pub struct GlobalNpmPackageInstaller<
  THttpClient: NpmCacheHttpClient,
  TSys: NpmCacheSys,
> {
  cache: Arc<NpmCache<TSys>>,
  tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
  sys: TSys,
  resolution: Arc<NpmResolutionCell>,
  lifecycle_scripts: LifecycleScriptsConfig,
  system_info: NpmSystemInfo,
}

impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys> std::fmt::Debug
  for GlobalNpmPackageInstaller<THttpClient, TSys>
{
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

impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys>
  GlobalNpmPackageInstaller<THttpClient, TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
    sys: TSys,
    resolution: Arc<NpmResolutionCell>,
    lifecycle_scripts: LifecycleScriptsConfig,
    system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      cache,
      tarball_cache,
      sys,
      resolution,
      lifecycle_scripts,
      system_info,
    }
  }

  async fn cache_packages(
    &self,
    packages: &[NpmResolutionPackage],
  ) -> Result<(), deno_npm_cache::EnsurePackageError> {
    let mut futures_unordered = FuturesUnordered::new();
    for package in packages {
      if let Some(dist) = &package.dist {
        futures_unordered.push(async move {
          self
            .tarball_cache
            .ensure_package(&package.id.nv, dist)
            .await
        });
      }
    }
    while let Some(result) = futures_unordered.next().await {
      // surface the first error
      result?;
    }
    Ok(())
  }
}

#[async_trait::async_trait(?Send)]
impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys> NpmPackageFsInstaller
  for GlobalNpmPackageInstaller<THttpClient, TSys>
{
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
    self
      .cache_packages(&package_partitions.packages)
      .await
      .map_err(JsErrorBox::from_err)?;

    // create the copy package folders
    for copy in package_partitions.copy_packages {
      self
        .cache
        .ensure_copy_package(&copy.get_package_cache_folder_id())
        .map_err(JsErrorBox::from_err)?;
    }

    let mut lifecycle_scripts = LifecycleScripts::new(
      &self.sys,
      &self.lifecycle_scripts,
      GlobalLifecycleScripts::new(
        self.cache.as_ref(),
        &self.sys,
        &self.lifecycle_scripts.root_dir,
      ),
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

struct GlobalLifecycleScripts<'a, TSys: NpmCacheSys> {
  cache: &'a NpmCache<TSys>,
  sys: &'a TSys,
  path_hash: u64,
}

impl<'a, TSys: NpmCacheSys> GlobalLifecycleScripts<'a, TSys> {
  fn new(cache: &'a NpmCache<TSys>, sys: &'a TSys, root_dir: &Path) -> Self {
    use std::hash::Hasher;
    let mut hasher = twox_hash::XxHash64::default();
    hasher.write(root_dir.to_string_lossy().as_bytes());
    let path_hash = hasher.finish();
    Self {
      cache,
      sys,
      path_hash,
    }
  }

  fn warned_scripts_file(&self, package: &NpmResolutionPackage) -> PathBuf {
    self
      .cache
      .package_folder_for_nv(&package.id.nv)
      .join(format!(".scripts-warned-{}", self.path_hash))
  }
}

impl<TSys: NpmCacheSys> LifecycleScriptsStrategy
  for GlobalLifecycleScripts<'_, TSys>
{
  fn can_run_scripts(&self) -> bool {
    false
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
      self.sys.fs_open(
        self.warned_scripts_file(package),
        &OpenOptions::new_write(),
      )?;
    }
    Ok(())
  }

  fn has_warned(&self, package: &NpmResolutionPackage) -> bool {
    self.sys.fs_exists_no_err(self.warned_scripts_file(package))
  }

  fn has_run(&self, _package: &NpmResolutionPackage) -> bool {
    false
  }
}
