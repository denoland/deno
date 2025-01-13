// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::npm::NpmCacheDir;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_resolver::npm::managed::ResolvePkgFolderFromPkgIdError;
use deno_resolver::npm::managed::ResolvePkgIdFromSpecifierError;
use deno_resolver::npm::ByonmOrManagedNpmResolver;
use deno_resolver::npm::ManagedNpmResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use node_resolver::NpmPackageFolderResolver;
use sys_traits::FsMetadata;
use thiserror::Error;

use super::CliNpmRegistryInfoProvider;
use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;
use crate::args::CliLockfile;
use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::cache::FastInsecureHasher;
use crate::sys::CliSys;

#[derive(Debug, Clone)]
pub enum CliNpmResolverManagedSnapshotOption {
  ResolveFromLockfile(Arc<CliLockfile>),
  Specified(Option<ValidSerializedNpmResolutionSnapshot>),
}

#[derive(Debug)]
enum SyncState {
  Pending(Option<CliNpmResolverManagedSnapshotOption>),
  Err(ResolveSnapshotError),
  Success,
}

#[derive(Debug)]
pub struct NpmResolutionInitializer {
  npm_registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
  npm_resolution: Arc<NpmResolutionCell>,
  queue: tokio::sync::Mutex<()>,
  sync_state: Mutex<SyncState>,
}

impl NpmResolutionInitializer {
  pub fn new(
    npm_registry_info_provider: Arc<CliNpmRegistryInfoProvider>,
    npm_resolution: Arc<NpmResolutionCell>,
    snapshot_option: CliNpmResolverManagedSnapshotOption,
  ) -> Self {
    Self {
      npm_registry_info_provider,
      npm_resolution,
      queue: tokio::sync::Mutex::new(()),
      sync_state: Mutex::new(SyncState::Pending(Some(snapshot_option))),
    }
  }

  #[cfg(debug_assertions)]
  pub fn debug_assert_initialized(&self) {
    if !matches!(*self.sync_state.lock(), SyncState::Success) {
      panic!("debug assert: npm resolution must be initialized before calling this code");
    }
  }

  pub async fn ensure_initialized(&self) -> Result<(), JsErrorBox> {
    // fast exit if not pending
    {
      match &*self.sync_state.lock() {
        SyncState::Pending(_) => {}
        SyncState::Err(err) => return Err(JsErrorBox::from_err(err.clone())),
        SyncState::Success => return Ok(()),
      }
    }

    // only allow one task in here at a time
    let _guard = self.queue.lock().await;

    let snapshot_option = {
      let mut sync_state = self.sync_state.lock();
      match &mut *sync_state {
        SyncState::Pending(snapshot_option) => {
          // this should never panic, but if it does it means that a
          // previous future was dropped while initialization occurred...
          // that should never happen because this is initialized during
          // startup
          snapshot_option.take().unwrap()
        }
        // another thread updated the state while we were waiting
        SyncState::Err(resolve_snapshot_error) => {
          return Err(JsErrorBox::from_err(resolve_snapshot_error.clone()));
        }
        SyncState::Success => {
          return Ok(());
        }
      }
    };

    match resolve_snapshot(&self.npm_registry_info_provider, snapshot_option)
      .await
    {
      Ok(maybe_snapshot) => {
        if let Some(snapshot) = maybe_snapshot {
          self
            .npm_resolution
            .set_snapshot(NpmResolutionSnapshot::new(snapshot));
        }
        let mut sync_state = self.sync_state.lock();
        *sync_state = SyncState::Success;
        Ok(())
      }
      Err(err) => {
        let mut sync_state = self.sync_state.lock();
        *sync_state = SyncState::Err(err.clone());
        Err(JsErrorBox::from_err(err))
      }
    }
  }
}

pub struct CliManagedNpmResolverCreateOptions {
  pub npm_cache_dir: Arc<NpmCacheDir>,
  pub sys: CliSys,
  pub maybe_node_modules_path: Option<PathBuf>,
  pub npm_system_info: NpmSystemInfo,
  pub npmrc: Arc<ResolvedNpmRc>,
  pub npm_resolution: Arc<NpmResolutionCell>,
}

pub fn create_managed_npm_resolver(
  options: CliManagedNpmResolverCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  let managed_npm_resolver =
    Arc::new(ManagedNpmResolver::<CliSys>::new::<CliSys>(
      &options.npm_cache_dir,
      &options.npmrc,
      options.npm_resolution.clone(),
      options.sys.clone(),
      options.maybe_node_modules_path,
    ));
  Arc::new(ManagedCliNpmResolver::new(
    managed_npm_resolver,
    options.npm_cache_dir,
    options.npmrc,
    options.npm_resolution,
    options.sys,
    options.npm_system_info,
  ))
}

#[derive(Debug, Error, Clone, JsError)]
#[error("failed reading lockfile '{}'", lockfile_path.display())]
#[class(inherit)]
pub struct ResolveSnapshotError {
  lockfile_path: PathBuf,
  #[inherit]
  #[source]
  source: SnapshotFromLockfileError,
}

impl ResolveSnapshotError {
  pub fn maybe_integrity_check_error(
    &self,
  ) -> Option<&deno_npm::resolution::IntegrityCheckFailedError> {
    match &self.source {
      SnapshotFromLockfileError::SnapshotFromLockfile(
        deno_npm::resolution::SnapshotFromLockfileError::IntegrityCheckFailed(
          err,
        ),
      ) => Some(err),
      _ => None,
    }
  }
}

async fn resolve_snapshot(
  registry_info_provider: &Arc<CliNpmRegistryInfoProvider>,
  snapshot: CliNpmResolverManagedSnapshotOption,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, ResolveSnapshotError>
{
  match snapshot {
    CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(lockfile) => {
      if !lockfile.overwrite() {
        let snapshot = snapshot_from_lockfile(
          lockfile.clone(),
          &registry_info_provider.as_npm_registry_api(),
        )
        .await
        .map_err(|source| ResolveSnapshotError {
          lockfile_path: lockfile.filename.clone(),
          source,
        })?;
        Ok(Some(snapshot))
      } else {
        Ok(None)
      }
    }
    CliNpmResolverManagedSnapshotOption::Specified(snapshot) => Ok(snapshot),
  }
}

#[derive(Debug, Error, Clone, JsError)]
pub enum SnapshotFromLockfileError {
  #[error(transparent)]
  #[class(inherit)]
  IncompleteError(
    #[from] deno_npm::resolution::IncompleteSnapshotFromLockfileError,
  ),
  #[error(transparent)]
  #[class(inherit)]
  SnapshotFromLockfile(#[from] deno_npm::resolution::SnapshotFromLockfileError),
}

async fn snapshot_from_lockfile(
  lockfile: Arc<CliLockfile>,
  api: &dyn NpmRegistryApi,
) -> Result<ValidSerializedNpmResolutionSnapshot, SnapshotFromLockfileError> {
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

/// An npm resolver where the resolution is managed by Deno rather than
/// the user bringing their own node_modules (BYONM) on the file system.
pub struct ManagedCliNpmResolver {
  managed_npm_resolver: Arc<ManagedNpmResolver<CliSys>>,
  npm_cache_dir: Arc<NpmCacheDir>,
  npm_rc: Arc<ResolvedNpmRc>,
  sys: CliSys,
  resolution: Arc<NpmResolutionCell>,
  system_info: NpmSystemInfo,
}

impl std::fmt::Debug for ManagedCliNpmResolver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ManagedCliNpmResolver")
      .field("<omitted>", &"<omitted>")
      .finish()
  }
}

impl ManagedCliNpmResolver {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    managed_npm_resolver: Arc<ManagedNpmResolver<CliSys>>,
    npm_cache_dir: Arc<NpmCacheDir>,
    npm_rc: Arc<ResolvedNpmRc>,
    resolution: Arc<NpmResolutionCell>,
    sys: CliSys,
    system_info: NpmSystemInfo,
  ) -> Self {
    Self {
      managed_npm_resolver,
      npm_cache_dir,
      npm_rc,
      resolution,
      sys,
      system_info,
    }
  }

  pub fn resolve_pkg_folder_from_pkg_id(
    &self,
    pkg_id: &NpmPackageId,
  ) -> Result<PathBuf, ResolvePkgFolderFromPkgIdError> {
    self
      .managed_npm_resolver
      .resolve_pkg_folder_from_pkg_id(pkg_id)
  }

  /// Resolves the package id from the provided specifier.
  pub fn resolve_pkg_id_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<NpmPackageId>, ResolvePkgIdFromSpecifierError> {
    self
      .managed_npm_resolver
      .resolve_pkg_id_from_specifier(specifier)
  }

  pub fn resolve_pkg_reqs_from_pkg_id(
    &self,
    id: &NpmPackageId,
  ) -> Vec<PackageReq> {
    self.resolution.resolve_pkg_reqs_from_pkg_id(id)
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
      .and_then(|id| {
        self
          .managed_npm_resolver
          .resolve_pkg_folder_from_pkg_id(&id)
          .ok()
      })
      .map(|folder| self.sys.fs_exists_no_err(folder))
      .unwrap_or(false)
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

  pub fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoModuleError> {
    self
      .managed_npm_resolver
      .resolve_pkg_folder_from_deno_module(nv)
  }

  pub fn resolve_pkg_id_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<NpmPackageId, PackageReqNotFoundError> {
    self.resolution.resolve_pkg_id_from_pkg_req(req)
  }

  pub fn maybe_node_modules_path(&self) -> Option<&Path> {
    self.managed_npm_resolver.node_modules_path()
  }

  pub fn global_cache_root_path(&self) -> &Path {
    self.npm_cache_dir.root_dir()
  }

  pub fn global_cache_root_url(&self) -> &Url {
    self.npm_cache_dir.root_dir_url()
  }
}

pub fn npm_process_state(
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

impl NpmProcessStateProvider for ManagedCliNpmResolver {
  fn get_npm_process_state(&self) -> String {
    npm_process_state(
      self.resolution.serialized_valid_snapshot(),
      self.managed_npm_resolver.node_modules_path(),
    )
  }
}

impl CliNpmResolver for ManagedCliNpmResolver {
  fn into_npm_pkg_folder_resolver(
    self: Arc<Self>,
  ) -> Arc<dyn NpmPackageFolderResolver> {
    self.managed_npm_resolver.clone()
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
    self
  }

  fn into_byonm_or_managed(
    self: Arc<Self>,
  ) -> ByonmOrManagedNpmResolver<CliSys> {
    ByonmOrManagedNpmResolver::Managed(self.managed_npm_resolver.clone())
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    // create a new snapshotted npm resolution and resolver
    let npm_resolution =
      Arc::new(NpmResolutionCell::new(self.resolution.snapshot()));

    Arc::new(ManagedCliNpmResolver::new(
      Arc::new(ManagedNpmResolver::<CliSys>::new::<CliSys>(
        &self.npm_cache_dir,
        &self.npm_rc,
        npm_resolution.clone(),
        self.sys.clone(),
        self.root_node_modules_path().map(ToOwned::to_owned),
      )),
      self.npm_cache_dir.clone(),
      self.npm_rc.clone(),
      npm_resolution,
      self.sys.clone(),
      self.system_info.clone(),
    ))
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Managed(self)
  }

  fn root_node_modules_path(&self) -> Option<&Path> {
    self.managed_npm_resolver.node_modules_path()
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
    hasher
      .write_hashable(self.managed_npm_resolver.node_modules_path().is_some());
    for (pkg_req, pkg_nv) in package_reqs {
      hasher.write_hashable(&pkg_req);
      hasher.write_hashable(&pkg_nv);
    }
    Some(hasher.finish())
  }

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError> {
    self
      .managed_npm_resolver
      .resolve_pkg_folder_from_deno_module_req(req, referrer)
      .map_err(ResolvePkgFolderFromDenoReqError::Managed)
  }
}
