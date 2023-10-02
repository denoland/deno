use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_graph::NpmPackageReqResolution;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NpmResolver;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;

pub struct CliNpmResolverByonmCreateOptions {
  pub root_node_modules_dir: PathBuf,
}

pub fn create_byonm_npm_resolver(
  options: CliNpmResolverByonmCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  Arc::new(ByonmCliNpmResolver {
    root_node_modules_dir: options.root_node_modules_dir,
  })
}

// todo(#18967): implement this
#[derive(Debug)]
pub struct ByonmCliNpmResolver {
  root_node_modules_dir: PathBuf,
}

impl NpmResolver for ByonmCliNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError> {
    todo!()
  }

  fn resolve_package_folder_from_path(
    &self,
    specifier: &deno_core::ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    todo!()
  }

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    specifier.scheme() == "file" && specifier.path().contains("/node_modules/")
  }

  fn ensure_read_permission(
    &self,
    permissions: &dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    todo!()
  }
}

impl CliNpmResolver for ByonmCliNpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver> {
    self
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    todo!()
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Byonm(self)
  }

  fn root_node_modules_path(&self) -> Option<std::path::PathBuf> {
    todo!()
  }

  fn resolve_pkg_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError> {
    todo!()
  }

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    todo!()
  }

  fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, AnyError> {
    todo!()
  }

  fn get_npm_process_state(&self) -> String {
    todo!()
  }

  fn check_state_hash(&self) -> Option<u64> {
    // it is very difficult to determine the check state hash for byonm
    // so we just return None to signify check caching is not supported
    None
  }
}
