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
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use sys_traits::OpenOptions;

use crate::LifecycleScriptsConfig;
use crate::NpmPackageFsInstaller;
use crate::PackageCaching;
use crate::lifecycle_scripts::LifecycleScripts;
use crate::lifecycle_scripts::LifecycleScriptsStrategy;

/// Resolves packages from the global npm cache.
pub struct GlobalNpmPackageInstaller<
  THttpClient: NpmCacheHttpClient,
  TSys: NpmCacheSys,
> {
  cache: Arc<NpmCache<TSys>>,
  tarball_cache: Arc<TarballCache<THttpClient, TSys>>,
  sys: TSys,
  resolution: Arc<NpmResolutionCell>,
  lifecycle_scripts: Arc<LifecycleScriptsConfig>,
  system_info: NpmSystemInfo,
  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
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
    lifecycle_scripts: Arc<LifecycleScriptsConfig>,
    system_info: NpmSystemInfo,
    install_reporter: Option<Arc<dyn crate::InstallReporter>>,
  ) -> Self {
    Self {
      cache,
      tarball_cache,
      sys,
      resolution,
      lifecycle_scripts,
      system_info,
      install_reporter,
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
        self.install_reporter.clone(),
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

  install_reporter: Option<Arc<dyn crate::InstallReporter>>,
}

impl<'a, TSys: NpmCacheSys> GlobalLifecycleScripts<'a, TSys> {
  fn new(
    cache: &'a NpmCache<TSys>,
    sys: &'a TSys,
    root_dir: &Path,
    install_reporter: Option<Arc<dyn crate::InstallReporter>>,
  ) -> Self {
    use std::hash::Hasher;
    let mut hasher = twox_hash::XxHash64::default();
    hasher.write(root_dir.to_string_lossy().as_bytes());
    let path_hash = hasher.finish();
    Self {
      cache,
      sys,
      path_hash,
      install_reporter,
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
    use std::fmt::Write;
    use std::writeln;
    let mut output = String::new();

    _ = writeln!(
      &mut output,
      "{} {}",
      colors::yellow("╭"),
      colors::yellow_bold("Warning")
    );
    _ = writeln!(&mut output, "{}", colors::yellow("│"));
    _ = writeln!(
      &mut output,
      "{}  Ignored build scripts for packages:",
      colors::yellow("│"),
    );
    for (package, _) in packages {
      _ = writeln!(
        &mut output,
        "{}  {}",
        colors::yellow("│"),
        colors::italic(format!("npm:{}", package.id.nv))
      );
    }
    _ = writeln!(&mut output, "{}", colors::yellow("│"));
    _ = writeln!(
      &mut output,
      "{}  Lifecycle scripts are only supported when using a `node_modules` directory.",
      colors::yellow("│")
    );
    _ = writeln!(
      &mut output,
      "{}  Enable it in your deno config file:",
      colors::yellow("│")
    );
    _ = writeln!(
      &mut output,
      "{}  {}",
      colors::yellow("│"),
      colors::bold("\"nodeModulesDir\": \"auto\"")
    );
    _ = write!(&mut output, "{}", colors::yellow("╰─"));

    if let Some(install_reporter) = &self.install_reporter {
      let paths = packages
        .iter()
        .map(|(package, _)| self.warned_scripts_file(package))
        .collect::<Vec<_>>();
      install_reporter.scripts_not_run_warning(
        crate::lifecycle_scripts::LifecycleScriptsWarning::new(
          output,
          Box::new(move |sys| {
            for path in paths {
              let _ignore_err =
                sys.fs_open_boxed(&path, &OpenOptions::new_write());
            }
          }),
        ),
      );
    } else {
      log::warn!("{}", output);
      for (package, _) in packages {
        let _ignore_err = self.sys.fs_open(
          self.warned_scripts_file(package),
          &OpenOptions::new_write(),
        );
      }
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
