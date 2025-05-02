// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::parking_lot::Mutex;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_resolver::npm::managed::ManagedNpmResolverCreateOptions;
use deno_resolver::npm::managed::NpmResolutionCell;
use thiserror::Error;

use super::WorkspaceNpmPatchPackages;
use crate::args::CliLockfile;
use crate::sys::CliSys;

pub type CliManagedNpmResolverCreateOptions =
  ManagedNpmResolverCreateOptions<CliSys>;

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
  npm_resolution: Arc<NpmResolutionCell>,
  patch_packages: Arc<WorkspaceNpmPatchPackages>,
  queue: tokio::sync::Mutex<()>,
  sync_state: Mutex<SyncState>,
}

impl NpmResolutionInitializer {
  pub fn new(
    npm_resolution: Arc<NpmResolutionCell>,
    patch_packages: Arc<WorkspaceNpmPatchPackages>,
    snapshot_option: CliNpmResolverManagedSnapshotOption,
  ) -> Self {
    Self {
      npm_resolution,
      patch_packages,
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

    match resolve_snapshot(snapshot_option, &self.patch_packages) {
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

#[derive(Debug, Error, Clone, JsError)]
#[error("failed reading lockfile '{}'", lockfile_path.display())]
#[class(inherit)]
pub struct ResolveSnapshotError {
  lockfile_path: PathBuf,
  #[inherit]
  #[source]
  source: SnapshotFromLockfileError,
}

fn resolve_snapshot(
  snapshot: CliNpmResolverManagedSnapshotOption,
  patch_packages: &WorkspaceNpmPatchPackages,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, ResolveSnapshotError>
{
  match snapshot {
    CliNpmResolverManagedSnapshotOption::ResolveFromLockfile(lockfile) => {
      if !lockfile.overwrite() {
        let snapshot = snapshot_from_lockfile(lockfile.clone(), patch_packages)
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
  SnapshotFromLockfile(#[from] deno_npm::resolution::SnapshotFromLockfileError),
}

pub(crate) struct DefaultTarballUrl;

impl deno_npm::resolution::DefaultTarballUrlProvider for DefaultTarballUrl {
  fn default_tarball_url(
    &self,
    nv: &deno_semver::package::PackageNv,
  ) -> String {
    let scope = nv.scope();
    let package_name = if let Some(scope) = scope {
      nv.name
        .strip_prefix(scope)
        .unwrap_or(&nv.name)
        .trim_start_matches('/')
    } else {
      &nv.name
    };
    format!(
      "https://registry.npmjs.org/{}/-/{}-{}.tgz",
      nv.name, package_name, nv.version
    )
  }
}

fn snapshot_from_lockfile(
  lockfile: Arc<CliLockfile>,
  patch_packages: &WorkspaceNpmPatchPackages,
) -> Result<ValidSerializedNpmResolutionSnapshot, SnapshotFromLockfileError> {
  let snapshot = deno_npm::resolution::snapshot_from_lockfile(
    deno_npm::resolution::SnapshotFromLockfileParams {
      patch_packages: &patch_packages.0,
      lockfile: &lockfile.lock(),
      default_tarball_url: &DefaultTarballUrl,
    },
  )?;

  Ok(snapshot)
}
