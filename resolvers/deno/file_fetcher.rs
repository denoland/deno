// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_cache_dir::file_fetcher::AuthTokens;
use deno_cache_dir::file_fetcher::BlobStore;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::FetchCachedError;
use deno_cache_dir::file_fetcher::File;
use deno_cache_dir::file_fetcher::FileFetcherSys;
use deno_cache_dir::file_fetcher::FileOrRedirect;
use deno_cache_dir::file_fetcher::HttpClient;
use deno_cache_dir::file_fetcher::TooManyRedirectsError;
use deno_cache_dir::file_fetcher::UnsupportedSchemeError;
use deno_cache_dir::GlobalHttpCacheRc;
use deno_cache_dir::GlobalHttpCacheSys;
use deno_cache_dir::HttpCacheRc;
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_graph::source::LoaderChecksum;
use deno_permissions::CheckSpecifierKind;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
use futures::FutureExt;
use http::header;
use node_resolver::InNpmPackageChecker;
use parking_lot::Mutex;
use thiserror::Error;
use url::Url;

use crate::npm::DenoInNpmPackageChecker;

#[derive(Debug, Default)]
struct MemoryFiles(Mutex<HashMap<Url, File>>);

impl MemoryFiles {
  pub fn insert(&self, specifier: Url, file: File) -> Option<File> {
    self.0.lock().insert(specifier, file)
  }

  pub fn clear(&self) {
    self.0.lock().clear();
  }
}

impl deno_cache_dir::file_fetcher::MemoryFiles for MemoryFiles {
  fn get(&self, specifier: &Url) -> Option<File> {
    self.0.lock().get(specifier).cloned()
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchError(pub Box<FetchErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  FetchNoFollow(#[from] FetchNoFollowError),
  #[error(transparent)]
  #[class(generic)]
  TooManyRedirects(#[from] TooManyRedirectsError),
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchNoFollowError(pub Box<FetchNoFollowErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchNoFollowErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  FetchNoFollow(#[from] deno_cache_dir::file_fetcher::FetchNoFollowError),
  #[error(transparent)]
  #[class(generic)]
  PermissionCheck(#[from] PermissionCheckError),
}

#[derive(Debug, Boxed, JsError)]
pub struct GetCachedSourceOrLocalError(
  pub Box<GetCachedSourceOrLocalErrorKind>,
);

#[derive(Debug, Error, JsError)]
pub enum GetCachedSourceOrLocalErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  FetchLocal(#[from] deno_cache_dir::file_fetcher::FetchLocalError),
  #[error(transparent)]
  #[class(inherit)]
  FetchCached(#[from] deno_cache_dir::file_fetcher::FetchCachedError),
}

#[derive(Debug, Copy, Clone)]
pub enum FetchPermissionsOptionRef<'a> {
  AllowAll,
  Restricted(&'a PermissionsContainer, CheckSpecifierKind),
}

#[derive(Debug, Default)]
pub struct FetchOptions<'a> {
  pub local: FetchLocalOptions,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
}

pub type FetchLocalOptions = deno_cache_dir::file_fetcher::FetchLocalOptions;

pub struct FetchNoFollowOptions<'a> {
  pub local: FetchLocalOptions,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
  pub maybe_checksum: Option<&'a LoaderChecksum>,
}

#[sys_traits::auto_impl]
pub trait PermissionedFileFetcherSys:
  FileFetcherSys + sys_traits::EnvVar
{
}

#[allow(clippy::disallowed_types)]
type PermissionedFileFetcherRc<TBlobStore, TSys, THttpClient> =
  crate::sync::MaybeArc<PermissionedFileFetcher<TBlobStore, TSys, THttpClient>>;
#[allow(clippy::disallowed_types)]
type MemoryFilesRc = crate::sync::MaybeArc<MemoryFiles>;

pub struct PermissionedFileFetcherOptions {
  pub allow_remote: bool,
  pub cache_setting: CacheSetting,
}

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct PermissionedFileFetcher<
  TBlobStore: BlobStore,
  TSys: PermissionedFileFetcherSys,
  THttpClient: HttpClient,
> {
  file_fetcher:
    deno_cache_dir::file_fetcher::FileFetcher<TBlobStore, TSys, THttpClient>,
  memory_files: MemoryFilesRc,
}

impl<
    TBlobStore: BlobStore,
    TSys: PermissionedFileFetcherSys,
    THttpClient: HttpClient,
  > PermissionedFileFetcher<TBlobStore, TSys, THttpClient>
{
  pub fn new(
    blob_store: TBlobStore,
    http_cache: HttpCacheRc,
    http_client: THttpClient,
    sys: TSys,
    options: PermissionedFileFetcherOptions,
  ) -> Self {
    let memory_files = crate::sync::new_rc(MemoryFiles::default());
    let auth_tokens = AuthTokens::new_from_sys(&sys);
    let file_fetcher = deno_cache_dir::file_fetcher::FileFetcher::new(
      blob_store,
      sys,
      http_cache,
      http_client,
      memory_files.clone(),
      deno_cache_dir::file_fetcher::FileFetcherOptions {
        allow_remote: options.allow_remote,
        cache_setting: options.cache_setting,
        auth_tokens,
      },
    );
    Self {
      file_fetcher,
      memory_files,
    }
  }

  pub fn cache_setting(&self) -> &CacheSetting {
    self.file_fetcher.cache_setting()
  }

  #[inline(always)]
  pub async fn fetch_bypass_permissions(
    &self,
    specifier: &Url,
  ) -> Result<File, FetchError> {
    self
      .fetch_inner(specifier, None, FetchPermissionsOptionRef::AllowAll)
      .await
  }

  #[inline(always)]
  pub async fn fetch_bypass_permissions_with_maybe_auth(
    &self,
    specifier: &Url,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<File, FetchError> {
    self
      .fetch_inner(specifier, maybe_auth, FetchPermissionsOptionRef::AllowAll)
      .await
  }

  /// Fetch a source file and asynchronously return it.
  #[inline(always)]
  pub async fn fetch(
    &self,
    specifier: &Url,
    permissions: &PermissionsContainer,
  ) -> Result<File, FetchError> {
    self
      .fetch_inner(
        specifier,
        None,
        FetchPermissionsOptionRef::Restricted(
          permissions,
          CheckSpecifierKind::Static,
        ),
      )
      .await
  }

  async fn fetch_inner(
    &self,
    specifier: &Url,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
    permissions: FetchPermissionsOptionRef<'_>,
  ) -> Result<File, FetchError> {
    self
      .fetch_with_options(
        specifier,
        permissions,
        FetchOptions {
          local: Default::default(),
          maybe_auth,
          maybe_accept: None,
          maybe_cache_setting: None,
        },
      )
      .await
  }

  pub async fn fetch_with_options(
    &self,
    specifier: &Url,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchOptions<'_>,
  ) -> Result<File, FetchError> {
    self
      .fetch_with_options_and_max_redirect(specifier, permissions, options, 10)
      .await
  }

  pub async fn fetch_with_options_and_max_redirect(
    &self,
    specifier: &Url,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchOptions<'_>,
    max_redirect: usize,
  ) -> Result<File, FetchError> {
    let mut specifier = Cow::Borrowed(specifier);
    let mut maybe_auth = options.maybe_auth;
    for _ in 0..=max_redirect {
      match self
        .fetch_no_follow(
          &specifier,
          permissions,
          FetchNoFollowOptions {
            local: options.local.clone(),
            maybe_auth: maybe_auth.clone(),
            maybe_accept: options.maybe_accept,
            maybe_cache_setting: options.maybe_cache_setting,
            maybe_checksum: None,
          },
        )
        .await?
      {
        FileOrRedirect::File(file) => {
          return Ok(file);
        }
        FileOrRedirect::Redirect(redirect_specifier) => {
          // If we were redirected to another origin, don't send the auth header anymore.
          if redirect_specifier.origin() != specifier.origin() {
            maybe_auth = None;
          }
          specifier = Cow::Owned(redirect_specifier);
        }
      }
    }

    Err(TooManyRedirectsError(specifier.into_owned()).into())
  }

  /// Fetches without following redirects.
  pub async fn fetch_no_follow(
    &self,
    specifier: &Url,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    validate_scheme(specifier).map_err(|err| {
      FetchNoFollowErrorKind::FetchNoFollow(err.into()).into_box()
    })?;
    match permissions {
      FetchPermissionsOptionRef::AllowAll => {
        // allow
      }
      FetchPermissionsOptionRef::Restricted(permissions, kind) => {
        permissions.check_specifier(specifier, kind)?;
      }
    }
    self
      .file_fetcher
      .fetch_no_follow(
        specifier,
        deno_cache_dir::file_fetcher::FetchNoFollowOptions {
          local: options.local,
          maybe_auth: options.maybe_auth,
          maybe_checksum: options
            .maybe_checksum
            .map(|c| deno_cache_dir::Checksum::new(c.as_str())),
          maybe_accept: options.maybe_accept,
          maybe_cache_setting: options.maybe_cache_setting,
        },
      )
      .await
      .map_err(|err| FetchNoFollowErrorKind::FetchNoFollow(err).into_box())
  }

  /// A synchronous way to retrieve a source file, where if the file has already
  /// been cached in memory it will be returned, otherwise for local files will
  /// be read from disk.
  pub fn get_cached_source_or_local(
    &self,
    specifier: &Url,
  ) -> Result<Option<File>, GetCachedSourceOrLocalError> {
    if specifier.scheme() == "file" {
      Ok(
        self
          .file_fetcher
          .fetch_local(specifier, &Default::default())?,
      )
    } else {
      Ok(self.file_fetcher.fetch_cached(specifier, 10)?)
    }
  }

  pub fn fetch_cached_remote(
    &self,
    url: &Url,
    redirect_limit: i64,
  ) -> Result<Option<File>, FetchCachedError> {
    self.file_fetcher.fetch_cached(url, redirect_limit)
  }

  /// Insert a temporary module for the file fetcher.
  pub fn insert_memory_files(&self, file: File) -> Option<File> {
    self.memory_files.insert(file.url.clone(), file)
  }

  pub fn clear_memory_files(&self) {
    self.memory_files.clear();
  }
}

pub struct DenoGraphLoaderOptions {
  pub file_header_overrides: HashMap<Url, HashMap<String, String>>,
  pub permissions: Option<PermissionsContainer>,
}

#[sys_traits::auto_impl]
pub trait DenoGraphLoaderSys:
  GlobalHttpCacheSys + PermissionedFileFetcherSys + sys_traits::FsCanonicalize
{
}

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// an implementation of `deno_graph::source::Loader`.
pub struct DenoGraphLoader<
  TBlobStore: BlobStore,
  TSys: DenoGraphLoaderSys,
  THttpClient: HttpClient,
> {
  file_header_overrides: HashMap<Url, HashMap<String, String>>,
  file_fetcher: PermissionedFileFetcherRc<TBlobStore, TSys, THttpClient>,
  global_http_cache: GlobalHttpCacheRc<TSys>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  permissions: Option<PermissionsContainer>,
  sys: TSys,
  cache_info_enabled: bool,
}

impl<
    TBlobStore: BlobStore,
    TSys: DenoGraphLoaderSys,
    THttpClient: HttpClient,
  > DenoGraphLoader<TBlobStore, TSys, THttpClient>
{
  pub fn new(
    file_fetcher: PermissionedFileFetcherRc<TBlobStore, TSys, THttpClient>,
    global_http_cache: GlobalHttpCacheRc<TSys>,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    sys: TSys,
    options: DenoGraphLoaderOptions,
  ) -> Self {
    Self {
      file_fetcher,
      global_http_cache,
      in_npm_pkg_checker,
      sys,
      file_header_overrides: options.file_header_overrides,
      permissions: options.permissions,
      cache_info_enabled: false,
    }
  }

  pub fn insert_file_header_override(
    &mut self,
    specifier: Url,
    headers: HashMap<String, String>,
  ) {
    self.file_header_overrides.insert(specifier, headers);
  }

  /// The cache information takes a bit of time to fetch and it's
  /// not always necessary. It should only be enabled for deno info.
  pub fn enable_loading_cache_info(&mut self) {
    self.cache_info_enabled = true;
  }

  /// Only use this for `deno info`.
  fn get_local_path(&self, specifier: &Url) -> Option<PathBuf> {
    // TODO(@kitsonk) fix when deno_graph does not query cache for synthetic
    // modules
    if specifier.scheme() == "flags" {
      None
    } else if specifier.scheme() == "file" {
      deno_path_util::url_to_file_path(specifier).ok()
    } else {
      self.global_http_cache.local_path_for_url(specifier).ok()
    }
  }
}

impl<
    TBlobStore: BlobStore + 'static,
    TSys: DenoGraphLoaderSys + 'static,
    THttpClient: HttpClient + 'static,
  > Loader for DenoGraphLoader<TBlobStore, TSys, THttpClient>
{
  fn get_cache_info(&self, specifier: &Url) -> Option<CacheInfo> {
    if !self.cache_info_enabled {
      return None;
    }

    let local = self.get_local_path(specifier)?;
    if self.sys.fs_is_file_no_err(&local) {
      Some(CacheInfo { local: Some(local) })
    } else {
      None
    }
  }

  fn load(
    &self,
    specifier: &Url,
    options: deno_graph::source::LoadOptions,
  ) -> LoadFuture {
    use deno_graph::source::CacheSetting as LoaderCacheSetting;

    if specifier.scheme() == "file"
      && specifier.path().contains("/node_modules/")
    {
      // The specifier might be in a completely different symlinked tree than
      // what the node_modules url is in (ex. `/my-project-1/node_modules`
      // symlinked to `/my-project-2/node_modules`), so first we checked if the path
      // is in a node_modules dir to avoid needlessly canonicalizing, then now compare
      // against the canonicalized specifier.
      let specifier = node_resolver::resolve_specifier_into_node_modules(
        &self.sys, specifier,
      );
      if self.in_npm_pkg_checker.in_npm_package(&specifier) {
        return Box::pin(std::future::ready(Ok(Some(
          LoadResponse::External { specifier },
        ))));
      }
    }

    let file_fetcher = self.file_fetcher.clone();
    let file_header_overrides = self.file_header_overrides.clone();
    let permissions = self.permissions.clone();
    let specifier = specifier.clone();
    let is_statically_analyzable = !options.was_dynamic_root;

    async move {
      let maybe_cache_setting = match options.cache_setting {
        LoaderCacheSetting::Use => None,
        LoaderCacheSetting::Reload => {
          if matches!(file_fetcher.cache_setting(), CacheSetting::Only) {
            return Err(load_error(JsErrorBox::generic(
              "Could not resolve version constraint using only cached data. Try running again without --cached-only"
            )));
          }
          Some(CacheSetting::ReloadAll)
        }
        LoaderCacheSetting::Only => Some(CacheSetting::Only),
      };
      file_fetcher
        .fetch_no_follow(
          &specifier,
          match &permissions {
            Some(permissions) => {
              FetchPermissionsOptionRef::Restricted(permissions,
              if is_statically_analyzable {
                CheckSpecifierKind::Static
              } else {
                CheckSpecifierKind::Dynamic
              })
            },
            None => FetchPermissionsOptionRef::AllowAll,
          },
          FetchNoFollowOptions {
            local: FetchLocalOptions {
              // only include the mtime in dynamic branches because we only
              // need to know about it then in order to tell whether to reload
              // or not
              include_mtime: options.in_dynamic_branch,
            },
            maybe_auth: None,
            maybe_accept: None,
            maybe_cache_setting: maybe_cache_setting.as_ref(),
            maybe_checksum: options.maybe_checksum.as_ref(),
          })
        .await
        .map(|file_or_redirect| {
          match file_or_redirect {
            FileOrRedirect::File(file) => {
              let maybe_headers =
              match (file.maybe_headers, file_header_overrides.get(&specifier)) {
                (Some(headers), Some(overrides)) => {
                  Some(headers.into_iter().chain(overrides.clone()).collect())
                }
                (Some(headers), None) => Some(headers),
                (None, Some(overrides)) => Some(overrides.clone()),
                (None, None) => None,
              };
            Ok(Some(LoadResponse::Module {
              specifier: file.url,
              maybe_headers,
              mtime: file.mtime,
              content: file.source,
            }))
            },
            FileOrRedirect::Redirect(redirect_specifier) => {
              Ok(Some(LoadResponse::Redirect {
                specifier: redirect_specifier,
              }))
            },
          }
        })
        .unwrap_or_else(|err| {
          let err = err.into_kind();
          match err {
            FetchNoFollowErrorKind::FetchNoFollow(err) => {
              use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind::*;
              let err = err.into_kind();
              match err {
                NotFound(_) => Ok(None),
                UrlToFilePath { .. } |
                ReadingBlobUrl { .. } |
                ReadingFile { .. } |
                FetchingRemote { .. } |
                ClientError { .. } |
                NoRemote { .. } |
                DataUrlDecode { .. } |
                RedirectResolution { .. } |
                CacheRead { .. } |
                CacheSave  { .. } |
                UnsupportedScheme  { .. } |
                RedirectHeaderParse { .. } |
                InvalidHeader { .. } => Err(load_error(JsErrorBox::from_err(err))),
                NotCached { .. } => {
                  if options.cache_setting == LoaderCacheSetting::Only {
                    Ok(None)
                  } else {
                    Err(load_error(JsErrorBox::from_err(err)))
                  }
                },
                ChecksumIntegrity(err) => {
                  // convert to the equivalent deno_graph error so that it
                  // enhances it if this is passed to deno_graph
                  Err(
                    deno_graph::source::LoadError::ChecksumIntegrity(deno_graph::source::ChecksumIntegrityError {
                      actual: err.actual,
                      expected: err.expected,
                    }),
                  )
                }
              }
            },
            FetchNoFollowErrorKind::PermissionCheck(permission_check_error) => Err(load_error(JsErrorBox::from_err(permission_check_error))),
          }
        })
    }
    .boxed_local()
  }
}

fn load_error(err: JsErrorBox) -> deno_graph::source::LoadError {
  #[allow(clippy::disallowed_types)] // ok, deno_graph requires an Arc
  let err = std::sync::Arc::new(err);
  deno_graph::source::LoadError::Other(err)
}

fn validate_scheme(specifier: &Url) -> Result<(), UnsupportedSchemeError> {
  match deno_cache_dir::file_fetcher::is_valid_scheme(specifier.scheme()) {
    true => Ok(()),
    false => Err(UnsupportedSchemeError {
      scheme: specifier.scheme().to_string(),
      url: specifier.clone(),
    }),
  }
}
