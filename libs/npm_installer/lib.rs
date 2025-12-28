// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_npm::NpmSystemInfo;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm_cache::NpmCache;
use deno_npm_cache::NpmCacheHttpClient;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

mod bin_entries;
mod extra_info;
mod factory;
mod flag;
mod fs;
mod global;
pub mod graph;
pub mod initializer;
pub mod lifecycle_scripts;
mod local;
pub mod package_json;
pub mod process_state;
pub mod resolution;
mod rt;

pub use bin_entries::BinEntries;
pub use bin_entries::BinEntriesError;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
use deno_unsync::sync::TaskQueue;
use parking_lot::Mutex;
use rustc_hash::FxHashSet;

pub use self::extra_info::CachedNpmPackageExtraInfoProvider;
pub use self::extra_info::ExpectedExtraInfo;
pub use self::extra_info::NpmPackageExtraInfoProvider;
use self::extra_info::NpmPackageExtraInfoProviderSys;
pub use self::factory::InstallReporter;
pub use self::factory::NpmInstallerFactory;
pub use self::factory::NpmInstallerFactoryOptions;
pub use self::factory::NpmInstallerFactorySys;
use self::global::GlobalNpmPackageInstaller;
use self::initializer::NpmResolutionInitializer;
use self::lifecycle_scripts::LifecycleScriptsExecutor;
use self::local::LocalNpmInstallSys;
use self::local::LocalNpmPackageInstaller;
pub use self::local::LocalSetupCache;
use self::package_json::NpmInstallDepsProvider;
use self::package_json::PackageJsonDepValueParseWithLocationError;
use self::resolution::AddPkgReqsResult;
use self::resolution::NpmResolutionInstaller;
use self::resolution::NpmResolutionInstallerSys;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCaching<'a> {
  Only(Cow<'a, [PackageReq]>),
  All,
}

/// The set of npm packages that are allowed to run lifecycle scripts.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub enum PackagesAllowedScripts {
  All,
  Some(Vec<PackageReq>),
  #[default]
  None,
}

/// Info needed to run NPM lifecycle scripts
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct LifecycleScriptsConfig {
  pub allowed: PackagesAllowedScripts,
  pub denied: Vec<PackageReq>,
  pub initial_cwd: PathBuf,
  pub root_dir: PathBuf,
  /// Part of an explicit `deno install`
  pub explicit_install: bool,
}

pub trait InstallProgressReporter:
  std::fmt::Debug + Send + Sync + 'static
{
  fn blocking(&self, message: &str);
  fn initializing(&self, nv: &PackageNv);
  fn initialized(&self, nv: &PackageNv);

  fn scripts_not_run_warning(
    &self,
    warning: crate::lifecycle_scripts::LifecycleScriptsWarning,
  );

  fn deprecated_message(&self, message: String);
}
pub trait Reporter:
  std::fmt::Debug + Send + Sync + 'static + dyn_clone::DynClone
{
  type Guard;
  type ClearGuard;

  fn on_blocking(&self, message: &str) -> Self::Guard;
  fn on_initializing(&self, message: &str) -> Self::Guard;
  fn clear_guard(&self) -> Self::ClearGuard;
}

#[derive(Debug, Clone)]
pub struct LogReporter;

impl Reporter for LogReporter {
  type Guard = ();
  type ClearGuard = ();

  fn on_blocking(&self, message: &str) -> Self::Guard {
    log::info!("{} {}", deno_terminal::colors::cyan("Blocking"), message);
  }

  fn on_initializing(&self, message: &str) -> Self::Guard {
    log::info!("{} {}", deno_terminal::colors::green("Initialize"), message);
  }

  fn clear_guard(&self) -> Self::ClearGuard {}
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

#[sys_traits::auto_impl]
pub trait NpmInstallerSys:
  NpmResolutionInstallerSys + LocalNpmInstallSys + NpmPackageExtraInfoProviderSys
{
}

pub struct NpmInstallerOptions<TSys: NpmInstallerSys> {
  pub maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
  pub maybe_node_modules_path: Option<PathBuf>,
  pub lifecycle_scripts: Arc<LifecycleScriptsConfig>,
  pub system_info: NpmSystemInfo,
  pub workspace_link_packages: WorkspaceNpmLinkPackagesRc,
}

#[derive(Debug)]
pub struct NpmInstaller<
  TNpmCacheHttpClient: NpmCacheHttpClient,
  TSys: NpmInstallerSys,
> {
  fs_installer: Arc<dyn NpmPackageFsInstaller>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  npm_resolution_initializer: Arc<NpmResolutionInitializer<TSys>>,
  npm_resolution_installer:
    Arc<NpmResolutionInstaller<TNpmCacheHttpClient, TSys>>,
  maybe_lockfile: Option<Arc<LockfileLock<TSys>>>,
  npm_resolution: Arc<NpmResolutionCell>,
  top_level_install_flag: AtomicFlag,
  install_queue: TaskQueue,
  cached_reqs: Mutex<FxHashSet<PackageReq>>,
}

impl<TNpmCacheHttpClient: NpmCacheHttpClient, TSys: NpmInstallerSys>
  NpmInstaller<TNpmCacheHttpClient, TSys>
{
  #[allow(clippy::too_many_arguments)]
  pub fn new<TReporter: Reporter>(
    install_reporter: Option<Arc<dyn InstallReporter>>,
    lifecycle_scripts_executor: Arc<dyn LifecycleScriptsExecutor>,
    npm_cache: Arc<NpmCache<TSys>>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    npm_registry_info_provider: Arc<
      dyn deno_npm::registry::NpmRegistryApi + Send + Sync,
    >,
    npm_resolution: Arc<NpmResolutionCell>,
    npm_resolution_initializer: Arc<NpmResolutionInitializer<TSys>>,
    npm_resolution_installer: Arc<
      NpmResolutionInstaller<TNpmCacheHttpClient, TSys>,
    >,
    reporter: &TReporter,
    sys: TSys,
    tarball_cache: Arc<deno_npm_cache::TarballCache<TNpmCacheHttpClient, TSys>>,
    options: NpmInstallerOptions<TSys>,
  ) -> Self {
    let fs_installer: Arc<dyn NpmPackageFsInstaller> =
      match options.maybe_node_modules_path {
        Some(node_modules_folder) => Arc::new(LocalNpmPackageInstaller::new(
          lifecycle_scripts_executor,
          npm_cache.clone(),
          Arc::new(NpmPackageExtraInfoProvider::new(
            npm_registry_info_provider,
            Arc::new(sys.clone()),
            options.workspace_link_packages,
          )),
          npm_install_deps_provider.clone(),
          dyn_clone::clone(reporter),
          npm_resolution.clone(),
          sys,
          tarball_cache,
          node_modules_folder,
          options.lifecycle_scripts,
          options.system_info,
          install_reporter,
        )),
        None => Arc::new(GlobalNpmPackageInstaller::new(
          npm_cache,
          tarball_cache,
          sys,
          npm_resolution.clone(),
          options.lifecycle_scripts,
          options.system_info,
          install_reporter,
        )),
      };
    Self {
      fs_installer,
      npm_install_deps_provider,
      npm_resolution,
      npm_resolution_initializer,
      npm_resolution_installer,
      maybe_lockfile: options.maybe_lockfile,
      top_level_install_flag: Default::default(),
      install_queue: Default::default(),
      cached_reqs: Default::default(),
    }
  }

  /// Adds package requirements to the resolver and ensures everything is setup.
  /// This includes setting up the `node_modules` directory, if applicable.
  pub async fn add_and_cache_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .add_package_reqs_raw(
        packages,
        Some(PackageCaching::Only(packages.into())),
      )
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs_no_cache(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .add_package_reqs_raw(packages, None)
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs(
    &self,
    packages: &[PackageReq],
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .add_package_reqs_raw(packages, Some(caching))
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs_raw(
    &self,
    packages: &[PackageReq],
    caching: Option<PackageCaching<'_>>,
  ) -> AddPkgReqsResult {
    if packages.is_empty() && !self.npm_resolution.is_pending() {
      return AddPkgReqsResult {
        dependencies_result: Ok(()),
        results: vec![],
      };
    }

    #[cfg(debug_assertions)]
    self.npm_resolution_initializer.debug_assert_initialized();

    let mut result = self
      .npm_resolution_installer
      .add_package_reqs(packages)
      .await;

    if result.dependencies_result.is_ok()
      && let Some(lockfile) = self.maybe_lockfile.as_ref()
    {
      result.dependencies_result = lockfile.error_if_changed();
    }
    if result.dependencies_result.is_ok()
      && let Some(caching) = caching
    {
      result.dependencies_result =
        self.maybe_cache_packages(packages, caching).await;
    }

    result
  }

  async fn maybe_cache_packages(
    &self,
    packages: &[PackageReq],
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    // the async mutex is unfortunate, but needed to handle the edge case where two workers
    // try to cache the same package at the same time. we need to hold the lock while we cache
    // and since that crosses an await point, we need the async mutex.
    //
    // should have a negligible perf impact because acquiring the lock is still in the order of nanoseconds
    // while caching typically takes micro or milli seconds.
    let _permit = self.install_queue.acquire().await;
    let uncached = {
      let cached_reqs = self.cached_reqs.lock();
      packages
        .iter()
        .filter(|req| !cached_reqs.contains(req))
        .collect::<Vec<_>>()
    };

    if uncached.is_empty() {
      return Ok(());
    }
    let result = self.fs_installer.cache_packages(caching).await;
    if result.is_ok() {
      let mut cached_reqs = self.cached_reqs.lock();
      for req in uncached {
        cached_reqs.insert(req.clone());
      }
    }
    result
  }

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    self
      .npm_resolution_installer
      .cache_package_info(package_name)
      .await
  }

  pub async fn cache_packages(
    &self,
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    if self.npm_resolution.is_pending() {
      self.add_package_reqs(&[], caching).await
    } else {
      self.npm_resolution_initializer.ensure_initialized().await?;
      self.fs_installer.cache_packages(caching).await
    }
  }

  pub fn ensure_no_pkg_json_dep_errors(
    &self,
  ) -> Result<(), Box<PackageJsonDepValueParseWithLocationError>> {
    for err in self.npm_install_deps_provider.pkg_json_dep_errors() {
      match err.source.as_kind() {
        deno_package_json::PackageJsonDepValueParseErrorKind::VersionReq(_)
        | deno_package_json::PackageJsonDepValueParseErrorKind::JsrVersionReq(
          _,
        ) => {
          return Err(Box::new(err.clone()));
        }
        deno_package_json::PackageJsonDepValueParseErrorKind::Unsupported {
          scheme,
        } if scheme == "jsr" => {
          return Err(Box::new(err.clone()));
        }
        deno_package_json::PackageJsonDepValueParseErrorKind::Unsupported {
          ..
        } => {
          // only warn for this one
          log::warn!(
            "{} {}\n    at {}",
            colors::yellow("Warning"),
            err.source,
            err.location,
          )
        }
      }
    }
    Ok(())
  }

  /// Ensures that the top level `package.json` dependencies are installed.
  ///
  /// Returns `true` if the top level packages are already installed. A
  /// return value of `false` means that new packages were added to the npm resolution.
  pub async fn ensure_top_level_package_json_install(
    &self,
  ) -> Result<bool, JsErrorBox> {
    if !self.top_level_install_flag.raise() {
      return Ok(true); // already did this
    }

    self.npm_resolution_initializer.ensure_initialized().await?;

    let pkg_json_remote_pkgs = self.npm_install_deps_provider.remote_pkgs();
    if pkg_json_remote_pkgs.is_empty() {
      return Ok(true);
    }

    // check if something needs resolving before bothering to load all
    // the package information (which is slow)
    if pkg_json_remote_pkgs.iter().all(|pkg| {
      self
        .npm_resolution
        .resolve_pkg_id_from_pkg_req(&pkg.req)
        .is_ok()
    }) {
      log::debug!(
        "All package.json deps resolvable. Skipping top level install."
      );
      return Ok(true); // everything is already resolvable
    }

    let pkg_reqs = pkg_json_remote_pkgs
      .iter()
      .map(|pkg| pkg.req.clone())
      .collect::<Vec<_>>();
    self.add_package_reqs_no_cache(&pkg_reqs).await?;

    Ok(false)
  }

  /// Run a resolution install if the npm snapshot is in a pending state
  /// due to a config file change.
  pub async fn install_resolution_if_pending(&self) -> Result<(), JsErrorBox> {
    self.npm_resolution_initializer.ensure_initialized().await?;
    self
      .npm_resolution_installer
      .install_if_pending()
      .await
      .map_err(JsErrorBox::from_err)?;
    Ok(())
  }
}
