// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod byonm;
mod common;
mod managed;

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use dashmap::DashMap;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_resolver::npm::ByonmInNpmPackageChecker;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::CliNpmReqResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use http::HeaderName;
use http::HeaderValue;
use managed::create_managed_in_npm_pkg_checker;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::http_util::HttpClientProvider;
use crate::util::fs::atomic_write_file_with_retries_and_fs;
use crate::util::fs::hard_link_dir_recursive;
use crate::util::fs::AtomicWriteFileFsAdapter;
use crate::util::progress_bar::ProgressBar;

pub use self::byonm::CliByonmNpmResolver;
pub use self::byonm::CliByonmNpmResolverCreateOptions;
pub use self::managed::CliManagedInNpmPkgCheckerCreateOptions;
pub use self::managed::CliManagedNpmResolverCreateOptions;
pub use self::managed::CliNpmResolverManagedSnapshotOption;
pub use self::managed::ManagedCliNpmResolver;

pub type CliNpmTarballCache = deno_npm_cache::TarballCache<CliNpmCacheEnv>;
pub type CliNpmCache = deno_npm_cache::NpmCache<CliNpmCacheEnv>;
pub type CliNpmRegistryInfoDownloader =
  deno_npm_cache::RegistryInfoDownloader<CliNpmCacheEnv>;

#[derive(Debug)]
pub struct CliNpmCacheEnv {
  fs: Arc<dyn FileSystem>,
  http_client_provider: Arc<HttpClientProvider>,
  progress_bar: ProgressBar,
}

impl CliNpmCacheEnv {
  pub fn new(
    fs: Arc<dyn FileSystem>,
    http_client_provider: Arc<HttpClientProvider>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      fs,
      http_client_provider,
      progress_bar,
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_npm_cache::NpmCacheEnv for CliNpmCacheEnv {
  fn exists(&self, path: &Path) -> bool {
    self.fs.exists_sync(path)
  }

  fn hard_link_dir_recursive(
    &self,
    from: &Path,
    to: &Path,
  ) -> Result<(), AnyError> {
    // todo(dsherret): use self.fs here instead
    hard_link_dir_recursive(from, to)
  }

  fn atomic_write_file_with_retries(
    &self,
    file_path: &Path,
    data: &[u8],
  ) -> std::io::Result<()> {
    atomic_write_file_with_retries_and_fs(
      &AtomicWriteFileFsAdapter {
        fs: self.fs.as_ref(),
        write_mode: crate::cache::CACHE_PERM,
      },
      file_path,
      data,
    )
  }

  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth_header: Option<(HeaderName, HeaderValue)>,
  ) -> Result<Option<Vec<u8>>, deno_npm_cache::DownloadError> {
    let guard = self.progress_bar.update(url.as_str());
    let client = self.http_client_provider.get_or_create().map_err(|err| {
      deno_npm_cache::DownloadError {
        status_code: None,
        error: err,
      }
    })?;
    client
      .download_with_progress_and_retries(url, maybe_auth_header, &guard)
      .await
      .map_err(|err| {
        use crate::http_util::DownloadError::*;
        let status_code = match &err {
          Fetch { .. }
          | UrlParse { .. }
          | HttpParse { .. }
          | Json { .. }
          | ToStr { .. }
          | NoRedirectHeader { .. }
          | TooManyRedirects => None,
          BadResponse(bad_response_error) => {
            Some(bad_response_error.status_code)
          }
        };
        deno_npm_cache::DownloadError {
          status_code,
          error: err.into(),
        }
      })
  }
}

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
  registry_info_downloader: Arc<CliNpmRegistryInfoDownloader>,
  npmrc: Arc<ResolvedNpmRc>,
}

impl NpmFetchResolver {
  pub fn new(
    npmrc: Arc<ResolvedNpmRc>,
    registry_info_downloader: Arc<CliNpmRegistryInfoDownloader>,
  ) -> Self {
    Self {
      nv_by_req: Default::default(),
      registry_info_downloader,
      npmrc,
    }
  }

  pub async fn req_to_nv(
    &self,
    req: &PackageReq,
  ) -> Result<Option<PackageNv>, AnyError> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return Ok(nv.value().clone());
    }
    let maybe_get_version = || async {
      let Some(package_info) = self.package_info(&req.name).await? else {
        return Result::<_, AnyError>::Ok(None);
      };
      if let Some(dist_tag) = req.version_req.tag() {
        return Ok(package_info.dist_tags.get(dist_tag).cloned());
      }
      // Find the first matching version of the package.
      let mut versions = package_info.versions.keys().collect::<Vec<_>>();
      versions.sort();
      Ok(
        versions
          .into_iter()
          .rev()
          .find(|v| {
            req.version_req.tag().is_none() && req.version_req.matches(v)
          })
          .cloned(),
      )
    };
    let maybe_nv = maybe_get_version().await?.map(|version| PackageNv {
      name: req.name.clone(),
      version,
    });
    self.nv_by_req.insert(req.clone(), maybe_nv.clone());
    Ok(maybe_nv)
  }

  pub async fn package_info(
    &self,
    name: &str,
  ) -> Result<Option<Arc<NpmPackageInfo>>, AnyError> {
    self.registry_info_downloader.load_package_info(name).await
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
