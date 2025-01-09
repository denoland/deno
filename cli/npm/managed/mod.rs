// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::npm::NpmCacheDir;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_npm_cache::NpmCacheSetting;
use deno_path_util::fs::canonicalize_path_maybe_not_exists;
use deno_resolver::npm::CliNpmReqResolver;
use deno_runtime::colors;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use installer::AddPkgReqsResult;
use installer::NpmResolutionInstaller;
use installers::create_npm_fs_installer;
use installers::NpmPackageFsInstaller;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use self::resolution::NpmResolution;
use self::resolvers::create_npm_fs_resolver;
use self::resolvers::NpmPackageFsResolver;
use super::CliNpmCache;
use super::CliNpmCacheHttpClient;
use super::CliNpmRegistryInfoProvider;
use super::CliNpmResolver;
use super::CliNpmTarballCache;
use super::InnerCliNpmResolverRef;
use super::ResolvePkgFolderFromDenoReqError;
use crate::args::CliLockfile;
use crate::args::LifecycleScriptsConfig;
use crate::args::NpmInstallDepsProvider;
use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::args::PackageJsonDepValueParseWithLocationError;
use crate::cache::FastInsecureHasher;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;
use crate::util::sync::AtomicFlag;

mod installer;
mod installers;
mod resolution;
mod resolvers;

pub enum CliNpmResolverManagedSnapshotOption {
  ResolveFromLockfile(Arc<CliLockfile>),
  Specified(Option<ValidSerializedNpmResolutionSnapshot>),
}

pub struct CliManagedNpmResolverCreateOptions {
  pub snapshot: CliNpmResolverManagedSnapshotOption,
  pub maybe_lockfile: Option<Arc<CliLockfile>>,
  pub http_client_provider: Arc<crate::http_util::HttpClientProvider>,
  pub npm_cache_dir: Arc<NpmCacheDir>,
  pub sys: CliSys,
  pub cache_setting: deno_cache_dir::file_fetcher::CacheSetting,
  pub text_only_progress_bar: crate::util::progress_bar::ProgressBar,
  pub maybe_node_modules_path: Option<PathBuf>,
  pub npm_system_info: NpmSystemInfo,
  pub npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  pub npmrc: Arc<ResolvedNpmRc>,
  pub lifecycle_scripts: LifecycleScriptsConfig,
}

pub async fn create_managed_npm_resolver_for_lsp(
  options: CliManagedNpmResolverCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  let npm_cache = create_cache(&options);
  let http_client = Arc::new(CliNpmCacheHttpClient::new(
    options.http_client_provider.clone(),
    options.text_only_progress_bar.clone(),
  ));
  let npm_api = create_api(npm_cache.clone(), http_client.clone(), &options);
  // spawn due to the lsp's `Send` requirement
  deno_core::unsync::spawn(async move {
    let snapshot = match resolve_snapshot(&npm_api, options.snapshot).await {
      Ok(snapshot) => snapshot,
      Err(err) => {
        log::warn!("failed to resolve snapshot: {}", err);
        None
      }
    };
    create_inner(
      http_client,
      npm_cache,
      options.npm_install_deps_provider,
      npm_api,
      options.sys,
      options.text_only_progress_bar,
      options.maybe_lockfile,
      options.npmrc,
      options.maybe_node_modules_path,
      options.npm_system_info,
      snapshot,
      options.lifecycle_scripts,
    )
  })
  .await
  .unwrap()
}

pub async fn create_managed_npm_resolver(
  options: CliManagedNpmResolverCreateOptions,
) -> Result<Arc<dyn CliNpmResolver>, AnyError> {
  let npm_cache = create_cache(&options);
  let http_client = Arc::new(CliNpmCacheHttpClient::new(
    options.http_client_provider.clone(),
    options.text_only_progress_bar.clone(),
  ));
  let api = create_api(npm_cache.clone(), http_client.clone(), &options);
  let snapshot = resolve_snapshot(&api, options.snapshot).await?;
  Ok(create_inner(
    http_client,
    npm_cache,
    options.npm_install_deps_provider,
    api,
    options.sys,
    options.text_only_progress_bar,
    options.maybe_lockfile,
    options.npmrc,
    options.maybe_node_modules_path,
    options.npm_system_info,
    snapshot,
    options.lifecycle_scripts,
  ))
}

#[allow(clippy::too_many_arguments)]
fn create_inner(
  http_client: Arc<CliNpmCacheHttpClient>,
  npm_cache: Arc<CliNpmCache>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
  sys: CliSys,
  text_only_progress_bar: crate::util::progress_bar::ProgressBar,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  npm_rc: Arc<ResolvedNpmRc>,
  node_modules_dir_path: Option<PathBuf>,
  npm_system_info: NpmSystemInfo,
  snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  lifecycle_scripts: LifecycleScriptsConfig,
) -> Arc<dyn CliNpmResolver> {
  let resolution = Arc::new(NpmResolution::from_serialized(snapshot));
  let tarball_cache = Arc::new(CliNpmTarballCache::new(
    npm_cache.clone(),
    http_client,
    sys.clone(),
    npm_rc.clone(),
  ));

  let fs_installer = create_npm_fs_installer(
    npm_cache.clone(),
    &npm_install_deps_provider,
    &text_only_progress_bar,
    resolution.clone(),
    sys.clone(),
    tarball_cache.clone(),
    node_modules_dir_path.clone(),
    npm_system_info.clone(),
    lifecycle_scripts.clone(),
  );
  let fs_resolver = create_npm_fs_resolver(
    npm_cache.clone(),
    resolution.clone(),
    sys.clone(),
    node_modules_dir_path,
  );
  Arc::new(ManagedCliNpmResolver::new(
    fs_installer,
    fs_resolver,
    maybe_lockfile,
    registry_info_provider,
    npm_cache,
    npm_install_deps_provider,
    resolution,
    sys,
    tarball_cache,
    text_only_progress_bar,
    npm_system_info,
    lifecycle_scripts,
  ))
}

fn create_cache(
  options: &CliManagedNpmResolverCreateOptions,
) -> Arc<CliNpmCache> {
  Arc::new(CliNpmCache::new(
    options.npm_cache_dir.clone(),
    options.sys.clone(),
    NpmCacheSetting::from_cache_setting(&options.cache_setting),
    options.npmrc.clone(),
  ))
}

fn create_api(
  cache: Arc<CliNpmCache>,
  http_client: Arc<CliNpmCacheHttpClient>,
  options: &CliManagedNpmResolverCreateOptions,
) -> Arc<CliNpmRegistryInfoProvider> {
  Arc::new(CliNpmRegistryInfoProvider::new(
    cache,
    http_client,
    options.npmrc.clone(),
  ))
}

async fn resolve_snapshot(
  registry_info_provider: &Arc<CliNpmRegistryInfoProvider>,
  snapshot: CliNpmResolverManagedSnapshotOption,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, AnyError> {
  match snapshot {
    CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(lockfile) => {
      if !lockfile.overwrite() {
        let snapshot = snapshot_from_lockfile(
          lockfile.clone(),
          &registry_info_provider.as_npm_registry_api(),
        )
        .await
        .with_context(|| {
          format!("failed reading lockfile '{}'", lockfile.filename.display())
        })?;
        Ok(Some(snapshot))
      } else {
        Ok(None)
      }
    }
    CliNpmResolverManagedSnapshotOption::Specified(snapshot) => Ok(snapshot),
  }
}

async fn snapshot_from_lockfile(
  lockfile: Arc<CliLockfile>,
  api: &dyn NpmRegistryApi,
) -> Result<ValidSerializedNpmResolutionSnapshot, AnyError> {
  let (incomplete_snapshot, skip_integrity_check) = {
    let lock = lockfile.lock();
    (
      deno_npm::resolution::incomplete_snapshot_from_lockfile(&lock)?,
      lock.overwrite,
    )
  };
  let snapshot = deno_npm::resolution::snapshot_from_lockfile(
    deno_npm::resolution::SnapshotFromLockfileParams {
      incomplete_snapshot,
      api,
      skip_integrity_check,
    },
  )
  .await?;
  Ok(snapshot)
}

#[derive(Debug)]
struct ManagedInNpmPackageChecker {
  root_dir: Url,
}

impl InNpmPackageChecker for ManagedInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.as_ref().starts_with(self.root_dir.as_str())
  }
}

pub struct CliManagedInNpmPkgCheckerCreateOptions<'a> {
  pub root_cache_dir_url: &'a Url,
  pub maybe_node_modules_path: Option<&'a Path>,
}

pub fn create_managed_in_npm_pkg_checker(
  options: CliManagedInNpmPkgCheckerCreateOptions,
) -> Arc<dyn InNpmPackageChecker> {
  let root_dir = match options.maybe_node_modules_path {
    Some(node_modules_folder) => {
      deno_path_util::url_from_directory_path(node_modules_folder).unwrap()
    }
    None => options.root_cache_dir_url.clone(),
  };
  debug_assert!(root_dir.as_str().ends_with('/'));
  Arc::new(ManagedInNpmPackageChecker { root_dir })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCaching<'a> {
  Only(Cow<'a, [PackageReq]>),
  All,
}

/// An npm resolver where the resolution is managed by Deno rather than
/// the user bringing their own node_modules (BYONM) on the file system.
pub struct ManagedCliNpmResolver {
  fs_installer: Arc<dyn NpmPackageFsInstaller>,
  fs_resolver: Arc<dyn NpmPackageFsResolver>,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
  npm_cache: Arc<CliNpmCache>,
  npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
  sys: CliSys,
  resolution: Arc<NpmResolution>,
  resolution_installer: NpmResolutionInstaller,
  tarball_cache: Arc<CliNpmTarballCache>,
  text_only_progress_bar: ProgressBar,
  npm_system_info: NpmSystemInfo,
  top_level_install_flag: AtomicFlag,
  lifecycle_scripts: LifecycleScriptsConfig,
}

impl std::fmt::Debug for ManagedCliNpmResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ManagedNpmResolver")
      .field("<omitted>", &"<omitted>")
      .finish()
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolvePkgFolderFromPkgIdError {
  #[class(inherit)]
  #[error("{0}")]
  NpmPackageFsResolverPackageFolder(
    #[from] resolvers::NpmPackageFsResolverPackageFolderError,
  ),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolvePkgFolderFromDenoModuleError {
  #[class(inherit)]
  #[error("{0}")]
  PackageNvNotFound(#[from] deno_npm::resolution::PackageNvNotFoundError),
  #[class(inherit)]
  #[error("{0}")]
  ResolvePkgFolderFromPkgId(#[from] ResolvePkgFolderFromPkgIdError),
}

impl ManagedCliNpmResolver {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    fs_installer: Arc<dyn NpmPackageFsInstaller>,
    fs_resolver: Arc<dyn NpmPackageFsResolver>,
    maybe_lockfile: Option<Arc<CliLockfile>>,
    registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
    npm_cache: Arc<CliNpmCache>,
    npm_install_deps_provider: Arc<NpmInstallDepsProvider>,
    resolution: Arc<NpmResolution>,
    sys: CliSys,
    tarball_cache: Arc<CliNpmTarballCache>,
    text_only_progress_bar: ProgressBar,
    npm_system_info: NpmSystemInfo,
    lifecycle_scripts: LifecycleScriptsConfig,
  ) -> Self {
    let resolution_installer = NpmResolutionInstaller::new(
      registry_info_provider.clone(),
      resolution.clone(),
      maybe_lockfile.clone(),
    );
    Self {
      fs_installer,
      fs_resolver,
      maybe_lockfile,
      registry_info_provider,
      npm_cache,
      npm_install_deps_provider,
      text_only_progress_bar,
      resolution,
      resolution_installer,
      sys,
      tarball_cache,
      npm_system_info,
      top_level_install_flag: Default::default(),
      lifecycle_scripts,
    }
  }

  pub fn resolve_pkg_folder_from_pkg_id(
    &self,
    pkg_id: &NpmPackageId,
  ) -> Result<PathBuf, ResolvePkgFolderFromPkgIdError> {
    let path = self.fs_resolver.package_folder(pkg_id)?;
    let path = canonicalize_path_maybe_not_exists(&self.sys, &path)?;
    log::debug!(
      "Resolved package folder of {} to {}",
      pkg_id.as_serialized(),
      path.display()
    );
    Ok(path)
  }

  /// Resolves the package id from the provided specifier.
  pub fn resolve_pkg_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageId>, AnyError> {
    let Some(cache_folder_id) = self
      .fs_resolver
      .resolve_package_cache_folder_id_from_specifier(specifier)?
    else {
      return Ok(None);
    };
    Ok(Some(
      self
        .resolution
        .resolve_pkg_id_from_pkg_cache_folder_id(&cache_folder_id)?,
    ))
  }

  pub fn resolve_pkg_reqs_from_pkg_id(
    &self,
    id: &NpmPackageId,
  ) -> Vec<PackageReq> {
    self.resolution.resolve_pkg_reqs_from_pkg_id(id)
  }

  /// Attempts to get the package size in bytes.
  pub fn package_size(
    &self,
    package_id: &NpmPackageId,
  ) -> Result<u64, AnyError> {
    let package_folder = self.fs_resolver.package_folder(package_id)?;
    Ok(crate::util::fs::dir_size(&package_folder)?)
  }

  pub fn all_system_packages(
    &self,
    system_info: &NpmSystemInfo,
  ) -> Vec<NpmResolutionPackage> {
    self.resolution.all_system_packages(system_info)
  }

  /// Checks if the provided package req's folder is cached.
  pub fn is_pkg_req_folder_cached(&self, req: &PackageReq) -> bool {
    self
      .resolve_pkg_id_from_pkg_req(req)
      .ok()
      .and_then(|id| self.fs_resolver.package_folder(&id).ok())
      .map(|folder| folder.exists())
      .unwrap_or(false)
  }

  /// Adds package requirements to the resolver and ensures everything is setup.
  /// This includes setting up the `node_modules` directory, if applicable.
  pub async fn add_and_cache_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), JsErrorBox> {
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
    self
      .add_package_reqs_raw(packages, Some(caching))
      .await
      .dependencies_result
  }

  pub async fn add_package_reqs_raw<'a>(
    &self,
    packages: &[PackageReq],
    caching: Option<PackageCaching<'a>>,
  ) -> AddPkgReqsResult {
    if packages.is_empty() {
      return AddPkgReqsResult {
        dependencies_result: Ok(()),
        results: vec![],
      };
    }

    let mut result = self.resolution_installer.add_package_reqs(packages).await;

    if result.dependencies_result.is_ok() {
      if let Some(lockfile) = self.maybe_lockfile.as_ref() {
        result.dependencies_result = lockfile.error_if_changed();
      }
    }
    if result.dependencies_result.is_ok() {
      if let Some(caching) = caching {
        result.dependencies_result = self.cache_packages(caching).await;
      }
    }

    result
  }

  /// Sets package requirements to the resolver, removing old requirements and adding new ones.
  ///
  /// This will retrieve and resolve package information, but not cache any package files.
  pub async fn set_package_reqs(
    &self,
    packages: &[PackageReq],
  ) -> Result<(), AnyError> {
    self.resolution_installer.set_package_reqs(packages).await
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.resolution.snapshot()
  }

  pub fn top_package_req_for_name(&self, name: &str) -> Option<PackageReq> {
    let package_reqs = self.resolution.package_reqs();
    let mut entries = package_reqs
      .iter()
      .filter(|(_, nv)| nv.name == name)
      .collect::<Vec<_>>();
    entries.sort_by_key(|(_, nv)| &nv.version);
    Some(entries.last()?.0.clone())
  }

  pub fn serialized_valid_snapshot_for_system(
    &self,
    system_info: &NpmSystemInfo,
  ) -> ValidSerializedNpmResolutionSnapshot {
    self
      .resolution
      .serialized_valid_snapshot_for_system(system_info)
  }

  pub async fn inject_synthetic_types_node_package(
    &self,
  ) -> Result<(), JsErrorBox> {
    let reqs = &[PackageReq::from_str("@types/node").unwrap()];
    // add and ensure this isn't added to the lockfile
    self
      .add_package_reqs(reqs, PackageCaching::Only(reqs.into()))
      .await?;

    Ok(())
  }

  pub async fn cache_packages(
    &self,
    caching: PackageCaching<'_>,
  ) -> Result<(), JsErrorBox> {
    self.fs_installer.cache_packages(caching).await
  }

  pub fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoModuleError> {
    let pkg_id = self.resolution.resolve_pkg_id_from_deno_module(nv)?;
    Ok(self.resolve_pkg_folder_from_pkg_id(&pkg_id)?)
  }

  pub fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self.resolution.resolve_pkg_id_from_pkg_req(req)
  }

  pub fn ensure_no_pkg_json_dep_errors(
    &self,
  ) -> Result<(), Box<PackageJsonDepValueParseWithLocationError>> {
    for err in self.npm_install_deps_provider.pkg_json_dep_errors() {
      match err.source.as_kind() {
        deno_package_json::PackageJsonDepValueParseErrorKind::VersionReq(_) => {
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
  /// This may set up the `node_modules` directory.
  ///
  /// Returns `true` if the top level packages are already installed. A
  /// return value of `false` means that new packages were added to the NPM resolution.
  pub async fn ensure_top_level_package_json_install(
    &self,
  ) -> Result<bool, JsErrorBox> {
    if !self.top_level_install_flag.raise() {
      return Ok(true); // already did this
    }

    let pkg_json_remote_pkgs = self.npm_install_deps_provider.remote_pkgs();
    if pkg_json_remote_pkgs.is_empty() {
      return Ok(true);
    }

    // check if something needs resolving before bothering to load all
    // the package information (which is slow)
    if pkg_json_remote_pkgs.iter().all(|pkg| {
      self
        .resolution
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

  pub async fn cache_package_info(
    &self,
    package_name: &str,
  ) -> Result<Arc<NpmPackageInfo>, AnyError> {
    // this will internally cache the package information
    self
      .registry_info_provider
      .package_info(package_name)
      .await
      .map_err(|err| err.into())
  }

  pub fn maybe_node_modules_path(&self) -> Option<&Path> {
    self.fs_resolver.node_modules_path()
  }

  pub fn global_cache_root_path(&self) -> &Path {
    self.npm_cache.root_dir_path()
  }

  pub fn global_cache_root_url(&self) -> &Url {
    self.npm_cache.root_dir_url()
  }
}

fn npm_process_state(
  snapshot: ValidSerializedNpmResolutionSnapshot,
  node_modules_path: Option<&Path>,
) -> String {
  serde_json::to_string(&NpmProcessState {
    kind: NpmProcessStateKind::Snapshot(snapshot.into_serialized()),
    local_node_modules_path: node_modules_path
      .map(|p| p.to_string_lossy().to_string()),
  })
  .unwrap()
}

impl NpmPackageFolderResolver for ManagedCliNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    let path = self
      .fs_resolver
      .resolve_package_folder_from_package(name, referrer)?;
    let path =
      canonicalize_path_maybe_not_exists(&self.sys, &path).map_err(|err| {
        PackageFolderResolveIoError {
          package_name: name.to_string(),
          referrer: referrer.clone(),
          source: err,
        }
      })?;
    log::debug!("Resolved {} from {} to {}", name, referrer, path.display());
    Ok(path)
  }
}

impl NpmProcessStateProvider for ManagedCliNpmResolver {
  fn get_npm_process_state(&self) -> String {
    npm_process_state(
      self.resolution.serialized_valid_snapshot(),
      self.fs_resolver.node_modules_path(),
    )
  }
}

impl CliNpmReqResolver for ManagedCliNpmResolver {
  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    _referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError> {
    let pkg_id = self.resolve_pkg_id_from_pkg_req(req).map_err(|err| {
      ResolvePkgFolderFromDenoReqError::Managed(Box::new(err))
    })?;
    self
      .resolve_pkg_folder_from_pkg_id(&pkg_id)
      .map_err(|err| ResolvePkgFolderFromDenoReqError::Managed(Box::new(err)))
  }
}

impl CliNpmResolver for ManagedCliNpmResolver {
  fn into_npm_pkg_folder_resolver(
    self: Arc<Self>,
  ) -> Arc<dyn NpmPackageFolderResolver> {
    self
  }

  fn into_npm_req_resolver(self: Arc<Self>) -> Arc<dyn CliNpmReqResolver> {
    self
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
    self
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    // create a new snapshotted npm resolution and resolver
    let npm_resolution =
      Arc::new(NpmResolution::new(self.resolution.snapshot()));

    Arc::new(ManagedCliNpmResolver::new(
      create_npm_fs_installer(
        self.npm_cache.clone(),
        &self.npm_install_deps_provider,
        &self.text_only_progress_bar,
        npm_resolution.clone(),
        self.sys.clone(),
        self.tarball_cache.clone(),
        self.root_node_modules_path().map(ToOwned::to_owned),
        self.npm_system_info.clone(),
        self.lifecycle_scripts.clone(),
      ),
      create_npm_fs_resolver(
        self.npm_cache.clone(),
        npm_resolution.clone(),
        self.sys.clone(),
        self.root_node_modules_path().map(ToOwned::to_owned),
      ),
      self.maybe_lockfile.clone(),
      self.registry_info_provider.clone(),
      self.npm_cache.clone(),
      self.npm_install_deps_provider.clone(),
      npm_resolution,
      self.sys.clone(),
      self.tarball_cache.clone(),
      self.text_only_progress_bar.clone(),
      self.npm_system_info.clone(),
      self.lifecycle_scripts.clone(),
    ))
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Managed(self)
  }

  fn root_node_modules_path(&self) -> Option<&Path> {
    self.fs_resolver.node_modules_path()
  }

  fn check_state_hash(&self) -> Option<u64> {
    // We could go further and check all the individual
    // npm packages, but that's probably overkill.
    let mut package_reqs = self
      .resolution
      .package_reqs()
      .into_iter()
      .collect::<Vec<_>>();
    package_reqs.sort_by(|a, b| a.0.cmp(&b.0)); // determinism
    let mut hasher = FastInsecureHasher::new_without_deno_version();
    // ensure the cache gets busted when turning nodeModulesDir on or off
    // as this could cause changes in resolution
    hasher.write_hashable(self.fs_resolver.node_modules_path().is_some());
    for (pkg_req, pkg_nv) in package_reqs {
      hasher.write_hashable(&pkg_req);
      hasher.write_hashable(&pkg_nv);
    }
    Some(hasher.finish())
  }
}
