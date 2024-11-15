// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod byonm;
mod common;
mod managed;

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use common::maybe_auth_header_for_npm_registry;
use dashmap::DashMap;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_resolver::npm::ByonmInNpmPackageChecker;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::CliNpmReqResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use managed::cache::registry_info::get_package_url;
use managed::create_managed_in_npm_pkg_checker;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::file_fetcher::FileFetcher;

pub use self::byonm::CliByonmNpmResolver;
pub use self::byonm::CliByonmNpmResolverCreateOptions;
pub use self::managed::CliManagedInNpmPkgCheckerCreateOptions;
pub use self::managed::CliManagedNpmResolverCreateOptions;
pub use self::managed::CliNpmResolverManagedSnapshotOption;
pub use self::managed::ManagedCliNpmResolver;

pub enum CliNpmResolverCreateOptions {
  Managed(CliManagedNpmResolverCreateOptions),
  Byonm(CliByonmNpmResolverCreateOptions),
}

pub async fn create_cli_npm_resolver_for_lsp(
  options: CliNpmResolverCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  use CliNpmResolverCreateOptions::*;
  match options {
    Managed(options) => {
      managed::create_managed_npm_resolver_for_lsp(options).await
    }
    Byonm(options) => Arc::new(ByonmNpmResolver::new(options)),
  }
}

pub async fn create_cli_npm_resolver(
  options: CliNpmResolverCreateOptions,
) -> Result<Arc<dyn CliNpmResolver>, AnyError> {
  use CliNpmResolverCreateOptions::*;
  match options {
    Managed(options) => managed::create_managed_npm_resolver(options).await,
    Byonm(options) => Ok(Arc::new(ByonmNpmResolver::new(options))),
  }
}

pub enum CreateInNpmPkgCheckerOptions<'a> {
  Managed(CliManagedInNpmPkgCheckerCreateOptions<'a>),
  Byonm,
}

pub fn create_in_npm_pkg_checker(
  options: CreateInNpmPkgCheckerOptions,
) -> Arc<dyn InNpmPackageChecker> {
  match options {
    CreateInNpmPkgCheckerOptions::Managed(options) => {
      create_managed_in_npm_pkg_checker(options)
    }
    CreateInNpmPkgCheckerOptions::Byonm => Arc::new(ByonmInNpmPackageChecker),
  }
}

pub enum InnerCliNpmResolverRef<'a> {
  Managed(&'a ManagedCliNpmResolver),
  #[allow(dead_code)]
  Byonm(&'a CliByonmNpmResolver),
}

pub trait CliNpmResolver: NpmPackageFolderResolver + CliNpmReqResolver {
  fn into_npm_pkg_folder_resolver(
    self: Arc<Self>,
  ) -> Arc<dyn NpmPackageFolderResolver>;
  fn into_npm_req_resolver(self: Arc<Self>) -> Arc<dyn CliNpmReqResolver>;
  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider>;
  fn into_maybe_byonm(self: Arc<Self>) -> Option<Arc<CliByonmNpmResolver>> {
    None
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver>;

  fn as_inner(&self) -> InnerCliNpmResolverRef;

  fn as_managed(&self) -> Option<&ManagedCliNpmResolver> {
    match self.as_inner() {
      InnerCliNpmResolverRef::Managed(inner) => Some(inner),
      InnerCliNpmResolverRef::Byonm(_) => None,
    }
  }

  fn as_byonm(&self) -> Option<&CliByonmNpmResolver> {
    match self.as_inner() {
      InnerCliNpmResolverRef::Managed(_) => None,
      InnerCliNpmResolverRef::Byonm(inner) => Some(inner),
    }
  }

  fn root_node_modules_path(&self) -> Option<&Path>;

  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, AnyError>;

  /// Returns a hash returning the state of the npm resolver
  /// or `None` if the state currently can't be determined.
  fn check_state_hash(&self) -> Option<u64>;
}

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name: DashMap<String, Option<Arc<NpmPackageInfo>>>,
  file_fetcher: Arc<FileFetcher>,
  npmrc: Arc<ResolvedNpmRc>,
}

impl NpmFetchResolver {
  pub fn new(
    file_fetcher: Arc<FileFetcher>,
    npmrc: Arc<ResolvedNpmRc>,
  ) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
      npmrc,
    }
  }

  pub async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return nv.value().clone();
    }
    let maybe_get_nv = || async {
      let name = req.name.clone();
      let package_info = self.package_info(&name).await?;
      if let Some(dist_tag) = req.version_req.tag() {
        let version = package_info.dist_tags.get(dist_tag)?.clone();
        return Some(PackageNv { name, version });
      }
      // Find the first matching version of the package.
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
      let info_url = get_package_url(&self.npmrc, name);
      let file_fetcher = self.file_fetcher.clone();
      let registry_config = self.npmrc.get_registry_config(name);
      // TODO(bartlomieju): this should error out, not use `.ok()`.
      let maybe_auth_header =
        maybe_auth_header_for_npm_registry(registry_config).ok()?;
      // spawn due to the lsp's `Send` requirement
      let file = deno_core::unsync::spawn(async move {
        file_fetcher
          .fetch_bypass_permissions_with_maybe_auth(
            &info_url,
            maybe_auth_header,
          )
          .await
          .ok()
      })
      .await
      .ok()??;
      serde_json::from_slice::<NpmPackageInfo>(&file.source).ok()
    };
    let info = fetch_package_info().await.map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
  }
}

pub const NPM_CONFIG_USER_AGENT_ENV_VAR: &str = "npm_config_user_agent";

pub fn get_npm_config_user_agent() -> String {
  format!(
    "deno/{} npm/? deno/{} {} {}",
    env!("CARGO_PKG_VERSION"),
    env!("CARGO_PKG_VERSION"),
    std::env::consts::OS,
    std::env::consts::ARCH
  )
}
