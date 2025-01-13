// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::serde_json;
use deno_core::url::Url;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::ByonmOrManagedNpmResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageReq;
use node_resolver::NpmPackageFolderResolver;

use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;
use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::sys::CliSys;

pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliSys>;
pub type CliByonmNpmResolver = ByonmNpmResolver<CliSys>;

// todo(dsherret): the services hanging off `CliNpmResolver` doesn't seem ideal. We should probably decouple.
#[derive(Debug)]
struct CliByonmWrapper(Arc<CliByonmNpmResolver>);

impl NpmProcessStateProvider for CliByonmWrapper {
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

impl CliNpmResolver for CliByonmNpmResolver {
  fn into_npm_pkg_folder_resolver(
    self: Arc<Self>,
  ) -> Arc<dyn NpmPackageFolderResolver> {
    self
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
    Arc::new(CliByonmWrapper(self))
  }

  fn into_byonm_or_managed(
    self: Arc<Self>,
  ) -> ByonmOrManagedNpmResolver<CliSys> {
    ByonmOrManagedNpmResolver::Byonm(self)
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    Arc::new(self.clone())
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Byonm(self)
  }

  fn check_state_hash(&self) -> Option<u64> {
    // it is very difficult to determine the check state hash for byonm
    // so we just return None to signify check caching is not supported
    None
  }
}
