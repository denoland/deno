// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeRequireResolver;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageReq;
use node_resolver::NpmResolver;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::resolver::CliDenoResolverFs;

use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;
use super::ResolvePkgFolderFromDenoReqError;

pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliDenoResolverFs>;
pub type CliByonmNpmResolver = ByonmNpmResolver<CliDenoResolverFs>;

// todo(dsherret): the services hanging off `CliNpmResolver` doesn't seem ideal. We should probably decouple.
#[derive(Debug)]
struct CliByonmWrapper(Arc<CliByonmNpmResolver>);

impl NodeRequireResolver for CliByonmWrapper {
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, AnyError> {
    if !path
      .components()
      .any(|c| c.as_os_str().to_ascii_lowercase() == "node_modules")
    {
      permissions.check_read_path(path)
    } else {
      Ok(Cow::Borrowed(path))
    }
  }
}

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
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver> {
    self
  }

  fn into_require_resolver(self: Arc<Self>) -> Arc<dyn NodeRequireResolver> {
    Arc::new(CliByonmWrapper(self))
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
    Arc::new(CliByonmWrapper(self))
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

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError> {
    ByonmNpmResolver::resolve_pkg_folder_from_deno_module_req(
      self, req, referrer,
    )
    .map_err(ResolvePkgFolderFromDenoReqError::Byonm)
  }

  fn check_state_hash(&self) -> Option<u64> {
    // it is very difficult to determine the check state hash for byonm
    // so we just return None to signify check caching is not supported
    None
  }
}
