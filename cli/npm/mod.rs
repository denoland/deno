// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod byonm;
mod cache_dir;
mod common;
mod managed;

use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_runtime::deno_node::NpmResolver;
use deno_semver::package::PackageReq;

pub use self::byonm::ByonmCliNpmResolver;
pub use self::byonm::CliNpmResolverByonmCreateOptions;
pub use self::cache_dir::NpmCacheDir;
pub use self::managed::CliNpmResolverManagedCreateOptions;
pub use self::managed::CliNpmResolverManagedPackageJsonInstallerOption;
pub use self::managed::CliNpmResolverManagedSnapshotOption;
pub use self::managed::ManagedCliNpmResolver;

pub enum CliNpmResolverCreateOptions {
  Managed(CliNpmResolverManagedCreateOptions),
  Byonm(CliNpmResolverByonmCreateOptions),
}

pub async fn create_cli_npm_resolver_for_lsp(
  options: CliNpmResolverCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  use CliNpmResolverCreateOptions::*;
  match options {
    Managed(options) => {
      managed::create_managed_npm_resolver_for_lsp(options).await
    }
    Byonm(options) => byonm::create_byonm_npm_resolver(options),
  }
}

pub async fn create_cli_npm_resolver(
  options: CliNpmResolverCreateOptions,
) -> Result<Arc<dyn CliNpmResolver>, AnyError> {
  use CliNpmResolverCreateOptions::*;
  match options {
    Managed(options) => managed::create_managed_npm_resolver(options).await,
    Byonm(options) => Ok(byonm::create_byonm_npm_resolver(options)),
  }
}

pub enum InnerCliNpmResolverRef<'a> {
  Managed(&'a ManagedCliNpmResolver),
  #[allow(dead_code)]
  Byonm(&'a ByonmCliNpmResolver),
}

pub trait CliNpmResolver: NpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver>;

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver>;

  fn as_inner(&self) -> InnerCliNpmResolverRef;

  fn as_managed(&self) -> Option<&ManagedCliNpmResolver> {
    match self.as_inner() {
      InnerCliNpmResolverRef::Managed(inner) => Some(inner),
      InnerCliNpmResolverRef::Byonm(_) => None,
    }
  }

  fn as_byonm(&self) -> Option<&ByonmCliNpmResolver> {
    match self.as_inner() {
      InnerCliNpmResolverRef::Managed(_) => None,
      InnerCliNpmResolverRef::Byonm(inner) => Some(inner),
    }
  }

  fn root_node_modules_path(&self) -> Option<&PathBuf>;

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError>;

  /// Returns a hash returning the state of the npm resolver
  /// or `None` if the state currently can't be determined.
  fn check_state_hash(&self) -> Option<u64>;
}
