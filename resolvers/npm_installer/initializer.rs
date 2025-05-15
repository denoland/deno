// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::lockfile::LockfileSys;
use deno_resolver::npm::managed::NpmResolutionCell;
use parking_lot::Mutex;
use thiserror::Error;

use super::WorkspaceNpmPatchPackages;

#[derive(Debug, Clone)]
pub enum NpmResolverManagedSnapshotOption<TSys: LockfileSys> {
  ResolveFromLockfile(Arc<LockfileLock<TSys>>),
  Specified(Option<ValidSerializedNpmResolutionSnapshot>),
}

#[derive(Debug)]
enum SyncState<TSys: LockfileSys> {
  Pending(Option<NpmResolverManagedSnapshotOption<TSys>>),
  Err(ResolveSnapshotError),
  Success,
}

#[derive(Debug)]
pub struct NpmResolutionInitializer<TSys: LockfileSys> {
  npm_resolution: Arc<NpmResolutionCell>,
  patch_packages: Arc<WorkspaceNpmPatchPackages>,
  queue: tokio::sync::Mutex<()>,
  sync_state: Mutex<SyncState<TSys>>,
}

impl<TSys: LockfileSys> NpmResolutionInitializer<TSys> {
  pub fn new(
    npm_resolution: Arc<NpmResolutionCell>,
    patch_packages: Arc<WorkspaceNpmPatchPackages>,
    snapshot_option: NpmResolverManagedSnapshotOption<TSys>,
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

fn resolve_snapshot<TSys: LockfileSys>(
  snapshot: NpmResolverManagedSnapshotOption<TSys>,
  patch_packages: &WorkspaceNpmPatchPackages,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, ResolveSnapshotError>
{
  match snapshot {
    NpmResolverManagedSnapshotOption::ResolveFromLockfile(lockfile) => {
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
    NpmResolverManagedSnapshotOption::Specified(snapshot) => Ok(snapshot),
  }
}

#[derive(Debug, Error, Clone, JsError)]
pub enum SnapshotFromLockfileError {
  #[error(transparent)]
  #[class(inherit)]
  SnapshotFromLockfile(#[from] deno_npm::resolution::SnapshotFromLockfileError),
}

fn snapshot_from_lockfile<TSys: LockfileSys>(
  lockfile: Arc<LockfileLock<TSys>>,
  patch_packages: &WorkspaceNpmPatchPackages,
) -> Result<ValidSerializedNpmResolutionSnapshot, SnapshotFromLockfileError> {
  let snapshot = deno_npm::resolution::snapshot_from_lockfile(
    deno_npm::resolution::SnapshotFromLockfileParams {
      patch_packages: &patch_packages.0,
      lockfile: &lockfile.lock(),
      default_tarball_url: Default::default(),
    },
  )?;

  Ok(snapshot)
}
