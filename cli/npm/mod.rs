// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod byonm;
mod cache_dir;
mod common;
mod managed;

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_npm::registry::NpmPackageInfo;
use deno_runtime::deno_node::NpmResolver;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

use crate::args::npm_registry_url;
use crate::file_fetcher::FileFetcher;

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

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name: DashMap<String, Option<Arc<NpmPackageInfo>>>,
  file_fetcher: FileFetcher,
}

impl NpmFetchResolver {
  pub fn new(file_fetcher: FileFetcher) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
    }
  }

  pub async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return nv.value().clone();
    }
    let maybe_get_nv = || async {
      let name = req.name.clone();
      let package_info = self.package_info(&name).await?;
      // Find the first matching version of the package which is cached.
      let mut versions = package_info.versions.keys().collect::<Vec<_>>();
      versions.sort();
      let version = versions
        .into_iter()
        .rev()
        .find(|v| req.version_req.tag().is_none() && req.version_req.matches(v))
        .cloned()?;
      Some(PackageNv { name, version })
    };
    let nv = maybe_get_nv().await;
    self.nv_by_req.insert(req.clone(), nv.clone());
    nv
  }

  pub async fn package_info(&self, name: &str) -> Option<Arc<NpmPackageInfo>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    let fetch_package_info = || async {
      let info_url = npm_registry_url().join(name).ok()?;
      let file = self
        .file_fetcher
        .fetch(&info_url, PermissionsContainer::allow_all())
        .await
        .ok()?;
      serde_json::from_slice::<NpmPackageInfo>(&file.source).ok()
    };
    let info = fetch_package_info().await.map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
  }
}
