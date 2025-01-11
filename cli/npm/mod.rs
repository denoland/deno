// Copyright 2018-2025 the Deno authors. MIT license.

mod byonm;
mod managed;
mod permission_checker;

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_npm::registry::NpmPackageInfo;
use deno_resolver::npm::ByonmNpmResolver;
use deno_resolver::npm::ByonmOrManagedNpmResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_runtime::ops::process::NpmProcessStateProvider;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use http::HeaderName;
use http::HeaderValue;
use node_resolver::NpmPackageFolderResolver;

pub use self::byonm::CliByonmNpmResolver;
pub use self::byonm::CliByonmNpmResolverCreateOptions;
pub use self::managed::CliManagedNpmResolverCreateOptions;
pub use self::managed::CliNpmResolverManagedSnapshotOption;
pub use self::managed::ManagedCliNpmResolver;
pub use self::managed::PackageCaching;
pub use self::managed::ResolvePkgFolderFromDenoModuleError;
pub use self::permission_checker::NpmRegistryReadPermissionChecker;
pub use self::permission_checker::NpmRegistryReadPermissionCheckerMode;
use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClientProvider;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

pub type CliNpmTarballCache =
  deno_npm_cache::TarballCache<CliNpmCacheHttpClient, CliSys>;
pub type CliNpmCache = deno_npm_cache::NpmCache<CliSys>;
pub type CliNpmRegistryInfoProvider =
  deno_npm_cache::RegistryInfoProvider<CliNpmCacheHttpClient, CliSys>;

#[derive(Debug)]
pub struct CliNpmCacheHttpClient {
  http_client_provider: Arc<HttpClientProvider>,
  progress_bar: ProgressBar,
}

impl CliNpmCacheHttpClient {
  pub fn new(
    http_client_provider: Arc<HttpClientProvider>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      http_client_provider,
      progress_bar,
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_npm_cache::NpmCacheHttpClient for CliNpmCacheHttpClient {
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
        use crate::http_util::DownloadErrorKind::*;
        let status_code = match err.as_kind() {
          Fetch { .. }
          | UrlParse { .. }
          | HttpParse { .. }
          | Json { .. }
          | ToStr { .. }
          | RedirectHeaderParse { .. }
          | TooManyRedirects
          | NotFound
          | Other(_) => None,
          BadResponse(bad_response_error) => {
            Some(bad_response_error.status_code)
          }
        };
        deno_npm_cache::DownloadError {
          status_code,
          error: JsErrorBox::from_err(err),
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

pub enum InnerCliNpmResolverRef<'a> {
  Managed(&'a ManagedCliNpmResolver),
  #[allow(dead_code)]
  Byonm(&'a CliByonmNpmResolver),
}

// todo(dsherret): replace with an enum
pub trait CliNpmResolver: Send + Sync + std::fmt::Debug {
  fn into_npm_pkg_folder_resolver(
    self: Arc<Self>,
  ) -> Arc<dyn NpmPackageFolderResolver>;
  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider>;
  fn into_byonm_or_managed(
    self: Arc<Self>,
  ) -> ByonmOrManagedNpmResolver<CliSys>;

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

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError>;

  fn root_node_modules_path(&self) -> Option<&Path>;

  /// Returns a hash returning the state of the npm resolver
  /// or `None` if the state currently can't be determined.
  fn check_state_hash(&self) -> Option<u64>;
}

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name: DashMap<String, Option<Arc<NpmPackageInfo>>>,
  file_fetcher: Arc<CliFileFetcher>,
  npmrc: Arc<ResolvedNpmRc>,
}

impl NpmFetchResolver {
  pub fn new(
    file_fetcher: Arc<CliFileFetcher>,
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
    // todo(#27198): use RegistryInfoProvider instead
    let fetch_package_info = || async {
      let info_url = deno_npm_cache::get_package_url(&self.npmrc, name);
      let file_fetcher = self.file_fetcher.clone();
      let registry_config = self.npmrc.get_registry_config(name);
      // TODO(bartlomieju): this should error out, not use `.ok()`.
      let maybe_auth_header =
        deno_npm_cache::maybe_auth_header_for_npm_registry(registry_config)
          .ok()?;
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
