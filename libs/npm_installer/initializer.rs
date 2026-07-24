// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use boxed_error::Boxed;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npmrc::ResolvedNpmRc;
use deno_resolver::lockfile::LockfileLock;
use deno_resolver::lockfile::LockfileSys;
use deno_resolver::npm::managed::NpmResolutionCell;
use deno_resolver::workspace::WorkspaceNpmLinkPackagesRc;
use deno_unsync::sync::TaskQueue;
use parking_lot::Mutex;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone)]
pub enum NpmResolverManagedSnapshotOption<TSys: LockfileSys> {
  ResolveFromLockfile {
    lockfile: Arc<LockfileLock<TSys>>,
    /// Whether to dedupe equivalent peer-dep variants when loading the
    /// snapshot. Safe to enable on any path — it's an in-memory
    /// normalization. See
    /// [`deno_npm::resolution::SnapshotFromLockfileParams`].
    dedup_equivalent_peer_variants: bool,
  },
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
  npmrc: Arc<ResolvedNpmRc>,
  link_packages: WorkspaceNpmLinkPackagesRc,
  queue: TaskQueue,
  sync_state: Mutex<SyncState<TSys>>,
}

impl<TSys: LockfileSys> NpmResolutionInitializer<TSys> {
  pub fn new(
    npm_resolution: Arc<NpmResolutionCell>,
    npmrc: Arc<ResolvedNpmRc>,
    link_packages: WorkspaceNpmLinkPackagesRc,
    snapshot_option: NpmResolverManagedSnapshotOption<TSys>,
  ) -> Self {
    Self {
      npm_resolution,
      npmrc,
      link_packages,
      queue: Default::default(),
      sync_state: Mutex::new(SyncState::Pending(Some(snapshot_option))),
    }
  }

  #[cfg(debug_assertions)]
  pub fn debug_assert_initialized(&self) {
    if !matches!(*self.sync_state.lock(), SyncState::Success) {
      panic!(
        "debug assert: npm resolution must be initialized before calling this code"
      );
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
    let _guard = self.queue.acquire().await;

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

    match resolve_snapshot(snapshot_option, &self.npmrc, &self.link_packages) {
      Ok(maybe_snapshot) => {
        if let Some(snapshot) = maybe_snapshot {
          self
            .npm_resolution
            .set_snapshot(NpmResolutionSnapshot::new(snapshot.snapshot));
          if snapshot.is_pending {
            self.npm_resolution.mark_pending();
          }
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

#[derive(Debug, Clone, JsError, Boxed)]
#[class(inherit)]
pub struct ResolveSnapshotError(Box<ResolveSnapshotErrorData>);

#[derive(Debug, Error, Clone, JsError)]
#[error("failed reading lockfile '{}'", lockfile_path.display())]
#[class(inherit)]
pub struct ResolveSnapshotErrorData {
  lockfile_path: PathBuf,
  #[inherit]
  #[source]
  source: SnapshotFromLockfileError,
}

fn resolve_snapshot<TSys: LockfileSys>(
  snapshot: NpmResolverManagedSnapshotOption<TSys>,
  npmrc: &ResolvedNpmRc,
  link_packages: &WorkspaceNpmLinkPackagesRc,
) -> Result<Option<SnapshotWithPending>, ResolveSnapshotError> {
  match snapshot {
    NpmResolverManagedSnapshotOption::ResolveFromLockfile {
      lockfile,
      dedup_equivalent_peer_variants,
    } => {
      if !lockfile.overwrite() {
        let snapshot = snapshot_from_lockfile(
          lockfile.clone(),
          npmrc,
          link_packages,
          dedup_equivalent_peer_variants,
        )
        .map_err(|source| {
          ResolveSnapshotErrorData {
            lockfile_path: lockfile.filename.clone(),
            source,
          }
          .into_box()
        })?;
        Ok(Some(snapshot))
      } else {
        Ok(None)
      }
    }
    NpmResolverManagedSnapshotOption::Specified(maybe_snapshot) => {
      Ok(maybe_snapshot.map(|snapshot| SnapshotWithPending {
        snapshot,
        is_pending: false,
      }))
    }
  }
}

#[derive(Debug, Error, Clone, JsError)]
pub enum SnapshotFromLockfileError {
  #[error(transparent)]
  #[class(inherit)]
  SnapshotFromLockfile(#[from] deno_npm::resolution::SnapshotFromLockfileError),
}

struct SnapshotWithPending {
  snapshot: ValidSerializedNpmResolutionSnapshot,
  is_pending: bool,
}

struct NpmRcDefaultTarballUrlProvider<'a>(&'a ResolvedNpmRc);

impl deno_npm::resolution::DefaultTarballUrlProvider
  for NpmRcDefaultTarballUrlProvider<'_>
{
  fn default_tarball_url(
    &self,
    nv: &deno_semver::package::PackageNv,
  ) -> String {
    let default_url =
      deno_npm::resolution::NpmRegistryDefaultTarballUrlProvider
        .default_tarball_url(nv);
    self
      .0
      .replace_tarball_url(Url::parse(&default_url).unwrap(), &nv.name)
      .to_string()
  }
}

fn snapshot_from_lockfile<TSys: LockfileSys>(
  lockfile: Arc<LockfileLock<TSys>>,
  npmrc: &ResolvedNpmRc,
  link_packages: &WorkspaceNpmLinkPackagesRc,
  dedup_equivalent_peer_variants: bool,
) -> Result<SnapshotWithPending, SnapshotFromLockfileError> {
  let lockfile = lockfile.lock();
  let default_tarball_url = NpmRcDefaultTarballUrlProvider(npmrc);
  let snapshot = deno_npm::resolution::snapshot_from_lockfile(
    deno_npm::resolution::SnapshotFromLockfileParams {
      link_packages: &link_packages.0,
      lockfile: &lockfile,
      default_tarball_url: &default_tarball_url,
      dedup_equivalent_peer_variants,
    },
  )?;

  Ok(SnapshotWithPending {
    snapshot,
    is_pending: lockfile.has_content_changed,
  })
}

#[cfg(test)]
mod tests {
  use deno_npm::resolution::DefaultTarballUrlProvider;
  use deno_npmrc::NPM_DEFAULT_REGISTRY;
  use deno_npmrc::NpmRc;
  use deno_npmrc::NpmRegistryUrl;
  use deno_semver::package::PackageNv;
  use sys_traits::impls::InMemorySys;

  use super::*;

  fn npmrc(config: &str) -> ResolvedNpmRc {
    NpmRc::parse(&InMemorySys::default(), config)
      .unwrap()
      .as_resolved(&NpmRegistryUrl {
        url: Url::parse(NPM_DEFAULT_REGISTRY).unwrap(),
        from_env: false,
      })
      .unwrap()
  }

  #[test]
  fn default_tarball_url_uses_configured_registry() {
    let npmrc = npmrc("registry=https://mirror.example.com/npm/");
    let provider = NpmRcDefaultTarballUrlProvider(&npmrc);
    assert_eq!(
      provider.default_tarball_url(&PackageNv::from_str("pkg@1.0.0").unwrap()),
      "https://mirror.example.com/npm/pkg/-/pkg-1.0.0.tgz",
    );
  }

  #[test]
  fn default_tarball_url_honors_never() {
    let npmrc = npmrc(
      "registry=https://mirror.example.com/npm/\nreplace-registry-host=never",
    );
    let provider = NpmRcDefaultTarballUrlProvider(&npmrc);
    assert_eq!(
      provider.default_tarball_url(&PackageNv::from_str("pkg@1.0.0").unwrap()),
      "https://registry.npmjs.org/pkg/-/pkg-1.0.0.tgz",
    );
  }

  #[test]
  fn default_tarball_url_uses_scoped_registry() {
    let npmrc = npmrc(
      "registry=https://default.example.com/\n@scope:registry=https://scope.example.com/npm/",
    );
    let provider = NpmRcDefaultTarballUrlProvider(&npmrc);
    assert_eq!(
      provider
        .default_tarball_url(&PackageNv::from_str("@scope/pkg@1.0.0").unwrap()),
      "https://scope.example.com/npm/@scope/pkg/-/pkg-1.0.0.tgz",
    );
  }
}
