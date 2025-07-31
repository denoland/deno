// Copyright 2018-2025 the Deno authors. MIT license.

mod permission_checker;

use std::sync::Arc;

use deno_npm_installer::process_state::NpmProcessState;
use deno_npm_installer::process_state::NpmProcessStateKind;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ManagedNpmResolverRc;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_process::NpmProcessStateProvider;
use deno_runtime::deno_process::NpmProcessStateProviderRc;
pub use permission_checker::NpmRegistryReadPermissionChecker;
pub use permission_checker::NpmRegistryReadPermissionCheckerMode;

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

#[derive(Debug)]
pub struct ManagedNpmProcessStateProvider<TSys: DenoLibSys>(
  pub ManagedNpmResolverRc<TSys>,
);

impl<TSys: DenoLibSys> NpmProcessStateProvider
  for ManagedNpmProcessStateProvider<TSys>
{
  fn get_npm_process_state(&self) -> String {
    NpmProcessState::new_managed(
      self.0.resolution().serialized_valid_snapshot(),
      self.0.root_node_modules_path(),
    )
    .as_serialized()
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
    NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: self
        .0
        .root_node_modules_path()
        .map(|p| p.to_string_lossy().into_owned()),
    }
    .as_serialized()
  }
}
