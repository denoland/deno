// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::CliNpmReqResolver;
use deno_runtime::deno_node::DenoFsNodeResolverEnv;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::ops::process::NpmProcessStateProvider;
use node_resolver::NpmPackageFolderResolver;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::resolver::CliDenoResolverFs;

use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;

pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliDenoResolverFs, DenoFsNodeResolverEnv>;
pub type CliByonmNpmResolver =
  ByonmNpmResolver<CliDenoResolverFs, DenoFsNodeResolverEnv>;

// todo(dsherret): the services hanging off `CliNpmResolver` doesn't seem ideal. We should probably decouple.
#[derive(Debug)]
struct CliByonmWrapper(Arc<CliByonmNpmResolver>);

impl NpmProcessStateProvider for CliByonmWrapper {
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: self
        .0
        .root_node_modules_dir()
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

  fn into_npm_req_resolver(self: Arc<Self>) -> Arc<dyn CliNpmReqResolver> {
    self
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
    Arc::new(CliByonmWrapper(self))
  }

  fn into_maybe_byonm(self: Arc<Self>) -> Option<Arc<CliByonmNpmResolver>> {
    Some(self)
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    Arc::new(self.clone())
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Byonm(self)
  }

  fn root_node_modules_path(&self) -> Option<&Path> {
    self.root_node_modules_dir()
  }

  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, AnyError> {
    if !path
      .components()
      .any(|c| c.as_os_str().to_ascii_lowercase() == "node_modules")
    {
      permissions.check_read_path(path).map_err(Into::into)
    } else {
      Ok(Cow::Borrowed(path))
    }
  }

  fn check_state_hash(&self) -> Option<u64> {
    // it is very difficult to determine the check state hash for byonm
    // so we just return None to signify check caching is not supported
    None
  }
}
