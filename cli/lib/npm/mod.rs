// Copyright 2018-2025 the Deno authors. MIT license.

mod permission_checker;

use std::path::Path;
use std::sync::Arc;

use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ManagedNpmResolverRc;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_process::NpmProcessStateProvider;
use deno_runtime::deno_process::NpmProcessStateProviderRc;
pub use permission_checker::NpmRegistryReadPermissionChecker;
pub use permission_checker::NpmRegistryReadPermissionCheckerMode;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::sys::DenoLibSys;

pub fn create_npm_process_state_provider<TSys: DenoLibSys>(
  npm_resolver: &NpmResolver<TSys>,
) -> NpmProcessStateProviderRc {
  match npm_resolver {
    NpmResolver::Byonm(byonm_npm_resolver) => {
      Arc::new(ByonmNpmProcessStateProvider(byonm_npm_resolver.clone()))
    }
    NpmResolver::Managed(managed_npm_resolver) => {
      Arc::new(ManagedNpmProcessStateProvider(managed_npm_resolver.clone()))
    }
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

#[derive(Debug)]
pub struct ManagedNpmProcessStateProvider<TSys: DenoLibSys>(
  pub ManagedNpmResolverRc<TSys>,
);

impl<TSys: DenoLibSys> NpmProcessStateProvider
  for ManagedNpmProcessStateProvider<TSys>
{
  fn get_npm_process_state(&self) -> String {
    npm_process_state(
      self.0.resolution().serialized_valid_snapshot(),
      self.0.root_node_modules_path(),
    )
  }
}

#[derive(Debug)]
pub struct ByonmNpmProcessStateProvider<TSys: DenoLibSys>(
  pub Arc<ByonmNpmResolver<TSys>>,
);

impl<TSys: DenoLibSys> NpmProcessStateProvider
  for ByonmNpmProcessStateProvider<TSys>
{
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: self
        .0
        .root_node_modules_path()
        .map(|p| p.to_string_lossy().to_string()),
    })
    .unwrap()
  }
}
