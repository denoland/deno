// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use boxed_error::Boxed;
use data_url::DataUrl;
use deno_error::JsError;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_media_type::MediaType;
use deno_path_util::url_to_file_path;
use http::header;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::IF_NONE_MATCH;
use http::header::LOCATION;
use log::debug;
use sys_traits::FsFileMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsOpen;
use sys_traits::OpenOptions;
use sys_traits::SystemTimeNow;
use thiserror::Error;
use url::Url;

use self::http_util::CacheSemantics;
use crate::CacheEntry;
use crate::CacheReadFileError;
use crate::Checksum;
use crate::ChecksumIntegrityError;
use crate::cache::HttpCacheRc;
use crate::common::HeadersMap;

mod auth_tokens;
mod http_util;

pub use auth_tokens::AuthDomain;
pub use auth_tokens::AuthToken;
pub use auth_tokens::AuthTokenData;
pub use auth_tokens::AuthTokens;
pub use http::HeaderMap;
pub use http::HeaderName;
pub use http::HeaderValue;
pub use http::StatusCode;

/// Indicates how cached source files should be handled.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CacheSetting {
  /// Only the cached files should be used.  Any files not in the cache will
  /// error.  This is the equivalent of `--cached-only` in the CLI.
  Only,
  /// No cached source files should be used, and all files should be reloaded.
  /// This is the equivalent of `--reload` in the CLI.
  ReloadAll,
  /// Only some cached resources should be used.  This is the equivalent of
  /// `--reload=jsr:@std/http/file-server` or
  /// `--reload=jsr:@std/http/file-server,jsr:@std/assert/assert-equals`.
  ReloadSome(Vec<String>),
  /// The usability of a cached value is determined by analyzing the cached
  /// headers and other metadata associated with a cached response, reloading
  /// any cached "non-fresh" cached responses.
  RespectHeaders,
  /// The cached source files should be used for local modules.  This is the
  /// default behavior of the CLI.
  Use,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileOrRedirect {
  File(File),
  Redirect(Url),
}

impl FileOrRedirect {
  fn from_deno_cache_entry(
    url: &Url,
    cache_entry: CacheEntry,
  ) -> Result<Self, RedirectResolutionError> {
    if let Some(redirect_to) = cache_entry.metadata.headers.get("location") {
      let redirect =
        url
          .join(redirect_to)
          .map_err(|source| RedirectResolutionError {
            url: url.clone(),
            location: redirect_to.clone(),
            source,
          })?;
      Ok(FileOrRedirect::Redirect(redirect))
    } else {
      Ok(FileOrRedirect::File(File {
        url: url.clone(),
        mtime: None,
        maybe_headers: Some(cache_entry.metadata.headers),
        #[allow(clippy::disallowed_types, reason = "ok for source")]
        source: std::sync::Arc::from(cache_entry.content),
        loaded_from: LoadedFrom::Cache,
      }))
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CachedOrRedirect {
  Cached,
  Redirect(Url),
}

impl From<FileOrRedirect> for CachedOrRedirect {
  fn from(value: FileOrRedirect) -> Self {
    match value {
      FileOrRedirect::File(_) => CachedOrRedirect::Cached,
      FileOrRedirect::Redirect(url) => CachedOrRedirect::Redirect(url),
    }
  }
}

#[allow(clippy::disallowed_types, reason = "ok for source")]
type FileSource = std::sync::Arc<[u8]>;

/// A structure representing a source file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub url: Url,
  pub mtime: Option<SystemTime>,
  pub maybe_headers: Option<HashMap<String, String>>,
  /// The source of the file.
  pub source: FileSource,

  /// Where the file was loaded from.
  pub loaded_from: LoadedFrom,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum LoadedFrom {
  /// The module was loaded from a remote source.
  Remote,
  /// The module was loaded from a local source.
  Local,
  /// The module was loaded from a cache for remote sources.
  Cache,
  /// The source of the module is unknown.
  Unknown,
}

impl File {
  pub fn resolve_media_type_and_charset(&self) -> (MediaType, Option<&str>) {
    deno_media_type::resolve_media_type_and_charset_from_content_type(
      &self.url,
      self
        .maybe_headers
        .as_ref()
        .and_then(|h| h.get("content-type"))
        .map(|v| v.as_str()),
    )
  }
}

#[allow(clippy::disallowed_types, reason = "arc wrapper type")]
pub type MemoryFilesRc = deno_maybe_sync::MaybeArc<dyn MemoryFiles>;

pub trait MemoryFiles: std::fmt::Debug + MaybeSend + MaybeSync {
  fn get(&self, url: &Url) -> Option<File>;
}

/// Implementation of `MemoryFiles` that always returns `None`.
#[derive(Debug, Clone, Default)]
pub struct NullMemoryFiles;

impl MemoryFiles for NullMemoryFiles {
  fn get(&self, _url: &Url) -> Option<File> {
    None
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SendResponse {
  NotModified,
  Redirect(HeaderMap),
  Success(HeaderMap, Vec<u8>),
}

#[derive(Debug)]
pub enum SendError {
  Failed(Box<dyn std::error::Error + Send + Sync>),
  NotFound,
  StatusCode(http::StatusCode),
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Failed resolving redirect from '{url}' to '{location}'.")]
pub struct RedirectResolutionError {
  pub url: Url,
  pub location: String,
  #[source]
  #[inherit]
  pub source: url::ParseError,
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Unable to decode data url.")]
pub struct DataUrlDecodeError {
  #[source]
  source: DataUrlDecodeSourceError,
}

#[derive(Debug, Error, JsError)]
#[class(uri)]
pub enum DataUrlDecodeSourceError {
  #[error(transparent)]
  DataUrl(data_url::DataUrlError),
  #[error(transparent)]
  InvalidBase64(data_url::forgiving_base64::InvalidBase64),
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Failed reading cache entry for '{url}'.")]
pub struct CacheReadError {
  pub url: Url,
  #[source]
  #[inherit]
  pub source: std::io::Error,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed reading location header for '{}'{}", .request_url, .maybe_location.as_ref().map(|location| format!(" to '{}'", location)).unwrap_or_default())]
pub struct RedirectHeaderParseError {
  pub request_url: Url,
  pub maybe_location: Option<String>,
  #[source]
  pub maybe_source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Import '{url}' failed.")]
pub struct FailedReadingLocalFileError {
  pub url: Url,
  #[source]
  #[inherit]
  pub source: std::io::Error,
}

#[derive(Debug, Error, JsError)]
#[class("Http")]
#[error("Fetch '{0}' failed, too many redirects.")]
pub struct TooManyRedirectsError(pub Url);

// this message list additional `npm` and `jsr` schemes, but they should actually be handled
// before `file_fetcher.rs` APIs are even hit.
#[derive(Debug, Error, JsError)]
#[class(type)]
#[error(
  "Unsupported scheme \"{scheme}\" for module \"{url}\". Supported schemes:\n - \"blob\"\n - \"data\"\n - \"file\"\n - \"http\"\n - \"https\"\n - \"jsr\"\n - \"npm\""
)]
pub struct UnsupportedSchemeError {
  pub scheme: String,
  pub url: Url,
}

/// Gets if the provided scheme was valid.
pub fn is_valid_scheme(scheme: &str) -> bool {
  matches!(
    scheme,
    "blob" | "data" | "file" | "http" | "https" | "jsr" | "npm"
  )
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchNoFollowError(pub Box<FetchNoFollowErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchNoFollowErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class("NotFound")]
  #[error("Import '{0}' failed, not found.")]
  NotFound(Url),
  #[class(generic)]
  #[error("Import '{url}' failed.")]
  ReadingBlobUrl {
    url: Url,
    #[source]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  ReadingFile(#[from] FailedReadingLocalFileError),
  #[class(generic)]
  #[error("Import '{url}' failed.")]
  FetchingRemote {
    url: Url,
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
  },
  #[class(generic)]
  #[error("Import '{url}' failed: {status_code}")]
  ClientError {
    url: Url,
    status_code: http::StatusCode,
  },
  #[class("NoRemote")]
  #[error(
    "A remote specifier was requested: \"{0}\", but --no-remote is specified."
  )]
  NoRemote(Url),
  #[class(inherit)]
  #[error(transparent)]
  DataUrlDecode(DataUrlDecodeError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(#[from] ChecksumIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[class(generic)]
  #[error("Failed caching '{url}'.")]
  CacheSave {
    url: Url,
    #[source]
    source: std::io::Error,
  },
  // this message list additional `npm` and `jsr` schemes, but they should actually be handled
  // before `file_fetcher.rs` APIs are even hit.
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedScheme(#[from] UnsupportedSchemeError),
  #[class(type)]
  #[error(transparent)]
  RedirectHeaderParse(#[from] RedirectHeaderParseError),
  #[class("NotCached")]
  #[error(
    "Specifier not found in cache: \"{url}\", --cached-only is specified."
  )]
  NotCached { url: Url },
  #[class(type)]
  #[error("Failed setting header '{name}'.")]
  InvalidHeader {
    name: &'static str,
    #[source]
    source: header::InvalidHeaderValue,
  },
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchCachedError(pub Box<FetchCachedErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchCachedErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  TooManyRedirects(TooManyRedirectsError),
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(#[from] ChecksumIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
}

#[derive(Debug, Boxed, JsError)]
pub struct FetchLocalError(pub Box<FetchLocalErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum FetchLocalErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] deno_path_util::UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  ReadingFile(#[from] FailedReadingLocalFileError),
}

impl From<FetchLocalError> for FetchNoFollowError {
  fn from(err: FetchLocalError) -> Self {
    match err.into_kind() {
      FetchLocalErrorKind::UrlToFilePath(err) => err.into(),
      FetchLocalErrorKind::ReadingFile(err) => err.into(),
    }
  }
}

#[derive(Debug, Boxed, JsError)]
struct FetchCachedNoFollowError(pub Box<FetchCachedNoFollowErrorKind>);

#[derive(Debug, Error, JsError)]
enum FetchCachedNoFollowErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  ChecksumIntegrity(ChecksumIntegrityError),
  #[class(inherit)]
  #[error(transparent)]
  CacheRead(#[from] CacheReadError),
  #[class(inherit)]
  #[error(transparent)]
  RedirectResolution(#[from] RedirectResolutionError),
}

impl From<FetchCachedNoFollowError> for FetchCachedError {
  fn from(err: FetchCachedNoFollowError) -> Self {
    match err.into_kind() {
      FetchCachedNoFollowErrorKind::ChecksumIntegrity(err) => err.into(),
      FetchCachedNoFollowErrorKind::CacheRead(err) => err.into(),
      FetchCachedNoFollowErrorKind::RedirectResolution(err) => err.into(),
    }
  }
}

impl From<FetchCachedNoFollowError> for FetchNoFollowError {
  fn from(err: FetchCachedNoFollowError) -> Self {
    match err.into_kind() {
      FetchCachedNoFollowErrorKind::ChecksumIntegrity(err) => err.into(),
      FetchCachedNoFollowErrorKind::CacheRead(err) => err.into(),
      FetchCachedNoFollowErrorKind::RedirectResolution(err) => err.into(),
    }
  }
}

#[async_trait::async_trait(?Send)]
pub trait HttpClient: std::fmt::Debug + MaybeSend + MaybeSync {
  /// Send a request getting the response.
  ///
  /// The implementation MUST not follow redirects. Return `SendResponse::Redirect`
  /// in that case.
  ///
  /// The implementation may retry the request on failure.
  async fn send_no_follow(
    &self,
    url: &Url,
    headers: HeaderMap,
  ) -> Result<SendResponse, SendError>;
}

#[derive(Debug, Clone)]
pub struct BlobData {
  pub media_type: String,
  pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct NullBlobStore;

#[async_trait::async_trait(?Send)]
impl BlobStore for NullBlobStore {
  async fn get(&self, _url: &Url) -> std::io::Result<Option<BlobData>> {
    Ok(None)
  }
}

#[async_trait::async_trait(?Send)]
pub trait BlobStore: std::fmt::Debug + MaybeSend + MaybeSync {
  async fn get(&self, url: &Url) -> std::io::Result<Option<BlobData>>;
}

#[derive(Debug, Default)]
pub struct FetchNoFollowOptions<'a> {
  pub local: FetchLocalOptions,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_checksum: Option<Checksum<'a>>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
}

#[derive(Debug, Clone, Default)]
pub struct FetchLocalOptions {
  pub include_mtime: bool,
}

#[derive(Debug)]
pub struct FileFetcherOptions {
  pub allow_remote: bool,
  pub cache_setting: CacheSetting,
  pub auth_tokens: AuthTokens,
}

#[sys_traits::auto_impl]
pub trait FileFetcherSys: FsOpen + SystemTimeNow {}

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct FileFetcher<
  TBlobStore: BlobStore,
  TSys: FileFetcherSys,
  THttpClient: HttpClient,
> {
  blob_store: TBlobStore,
  sys: TSys,
  http_cache: HttpCacheRc,
  http_client: THttpClient,
  memory_files: MemoryFilesRc,
  allow_remote: bool,
  cache_setting: CacheSetting,
  auth_tokens: AuthTokens,
}

impl<TBlobStore: BlobStore, TSys: FileFetcherSys, THttpClient: HttpClient>
  FileFetcher<TBlobStore, TSys, THttpClient>
{
  pub fn new(
    blob_store: TBlobStore,
    sys: TSys,
    http_cache: HttpCacheRc,
    http_client: THttpClient,
    memory_files: MemoryFilesRc,
    options: FileFetcherOptions,
  ) -> Self {
    Self {
      blob_store,
      sys,
      http_cache,
      http_client,
      memory_files,
      allow_remote: options.allow_remote,
      auth_tokens: options.auth_tokens,
      cache_setting: options.cache_setting,
    }
  }

  pub fn cache_setting(&self) -> &CacheSetting {
    &self.cache_setting
  }

  /// Fetch cached remote file.
  pub fn fetch_cached(
    &self,
    url: &Url,
    redirect_limit: i64,
  ) -> Result<Option<File>, FetchCachedError> {
    if !matches!(url.scheme(), "http" | "https") {
      return Ok(None);
    }

    let mut url = Cow::Borrowed(url);
    for _ in 0..=redirect_limit {
      match self.fetch_cached_no_follow(&url, None)? {
        Some(FileOrRedirect::File(file)) => {
          return Ok(Some(file));
        }
        Some(FileOrRedirect::Redirect(redirect_url)) => {
          url = Cow::Owned(redirect_url);
        }
        None => {
          return Ok(None);
        }
      }
    }
    Err(
      FetchCachedErrorKind::TooManyRedirects(TooManyRedirectsError(
        url.into_owned(),
      ))
      .into_box(),
    )
  }

  /// Fetches without following redirects.
  ///
  /// You should verify permissions of the specifier before calling this function.
  pub async fn fetch_no_follow(
    &self,
    url: &Url,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    // note: this debug output is used by the tests
    debug!("FileFetcher::fetch_no_follow - specifier: {}", url);
    self
      .fetch_no_follow_with_strategy(&FetchStrategy(self), url, options)
      .await
  }

  /// Ensures the data is cached without following redirects.
  ///
  /// You should verify permissions of the specifier before calling this function.
  pub async fn ensure_cached_no_follow(
    &self,
    url: &Url,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<CachedOrRedirect, FetchNoFollowError> {
    // note: this debug output is used by the tests
    debug!("FileFetcher::ensure_cached_no_follow - specifier: {}", url);
    self
      .fetch_no_follow_with_strategy(&EnsureCachedStrategy(self), url, options)
      .await
  }

  async fn fetch_no_follow_with_strategy<
    TStrategy: FetchOrEnsureCacheStrategy,
  >(
    &self,
    strategy: &TStrategy,
    url: &Url,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<TStrategy::ReturnValue, FetchNoFollowError> {
    let scheme = url.scheme();
    if let Some(file) = self.memory_files.get(url) {
      Ok(strategy.handle_memory_file(file))
    } else if scheme == "file" {
      match strategy.handle_local(url, &options.local)? {
        Some(file) => Ok(file),
        None => Err(FetchNoFollowErrorKind::NotFound(url.clone()).into_box()),
      }
    } else if scheme == "data" {
      strategy
        .handle_data_url(url)
        .map_err(|e| FetchNoFollowErrorKind::DataUrlDecode(e).into_box())
    } else if scheme == "blob" {
      strategy.handle_blob_url(url).await
    } else if scheme == "https" || scheme == "http" {
      if !self.allow_remote {
        Err(FetchNoFollowErrorKind::NoRemote(url.clone()).into_box())
      } else {
        self
          .fetch_remote_no_follow(
            strategy,
            url,
            options.maybe_accept,
            options.maybe_cache_setting.unwrap_or(&self.cache_setting),
            options.maybe_checksum,
            options.maybe_auth,
          )
          .await
      }
    } else {
      Err(
        FetchNoFollowErrorKind::UnsupportedScheme(UnsupportedSchemeError {
          scheme: scheme.to_string(),
          url: url.clone(),
        })
        .into_box(),
      )
    }
  }

  fn fetch_cached_no_follow(
    &self,
    url: &Url,
    maybe_checksum: Option<Checksum<'_>>,
  ) -> Result<Option<FileOrRedirect>, FetchCachedNoFollowError> {
    debug!("FileFetcher::fetch_cached_no_follow - specifier: {}", url);

    let cache_key =
      self
        .http_cache
        .cache_item_key(url)
        .map_err(|source| CacheReadError {
          url: url.clone(),
          source,
        })?;
    match self.http_cache.get(&cache_key, maybe_checksum) {
      Ok(Some(entry)) => {
        Ok(Some(FileOrRedirect::from_deno_cache_entry(url, entry)?))
      }
      Ok(None) => Ok(None),
      Err(CacheReadFileError::Io(source)) => Err(
        FetchCachedNoFollowErrorKind::CacheRead(CacheReadError {
          url: url.clone(),
          source,
        })
        .into_box(),
      ),
      Err(CacheReadFileError::ChecksumIntegrity(err)) => {
        Err(FetchCachedNoFollowErrorKind::ChecksumIntegrity(*err).into_box())
      }
    }
  }

  /// Convert a data URL into a file, resulting in an error if the URL is
  /// invalid.
  fn fetch_data_url(&self, url: &Url) -> Result<File, DataUrlDecodeError> {
    fn parse(
      url: &Url,
    ) -> Result<(Vec<u8>, HashMap<String, String>), DataUrlDecodeError> {
      let url = DataUrl::process(url.as_str()).map_err(|source| {
        DataUrlDecodeError {
          source: DataUrlDecodeSourceError::DataUrl(source),
        }
      })?;
      let (bytes, _) =
        url.decode_to_vec().map_err(|source| DataUrlDecodeError {
          source: DataUrlDecodeSourceError::InvalidBase64(source),
        })?;
      let headers = HashMap::from([(
        "content-type".to_string(),
        url.mime_type().to_string(),
      )]);
      Ok((bytes, headers))
    }

    debug!("FileFetcher::fetch_data_url() - specifier: {}", url);
    let (bytes, headers) = parse(url)?;
    Ok(File {
      url: url.clone(),
      mtime: None,
      maybe_headers: Some(headers),
      loaded_from: LoadedFrom::Local,
      #[allow(clippy::disallowed_types, reason = "ok for source")]
      source: std::sync::Arc::from(bytes),
    })
  }

  /// Get a blob URL.
  async fn fetch_blob_url(
    &self,
    url: &Url,
  ) -> Result<File, FetchNoFollowError> {
    debug!("FileFetcher::fetch_blob_url() - specifier: {}", url);
    let blob = self
      .blob_store
      .get(url)
      .await
      .map_err(|err| FetchNoFollowErrorKind::ReadingBlobUrl {
        url: url.clone(),
        source: err,
      })?
      .ok_or_else(|| FetchNoFollowErrorKind::NotFound(url.clone()))?;

    let headers =
      HashMap::from([("content-type".to_string(), blob.media_type.clone())]);

    Ok(File {
      url: url.clone(),
      mtime: None,
      maybe_headers: Some(headers),
      loaded_from: LoadedFrom::Local,
      #[allow(clippy::disallowed_types, reason = "ok for source")]
      source: std::sync::Arc::from(blob.bytes),
    })
  }

  async fn fetch_remote_no_follow<TStrategy: FetchOrEnsureCacheStrategy>(
    &self,
    strategy: &TStrategy,
    url: &Url,
    maybe_accept: Option<&str>,
    cache_setting: &CacheSetting,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<TStrategy::ReturnValue, FetchNoFollowError> {
    debug!("FileFetcher::fetch_remote_no_follow - specifier: {}", url);

    if self.should_use_cache(url, cache_setting)
      && let Some(value) =
        strategy.handle_fetch_cached_no_follow(url, maybe_checksum)?
    {
      return Ok(value);
    }

    if *cache_setting == CacheSetting::Only {
      return Err(
        FetchNoFollowErrorKind::NotCached { url: url.clone() }.into_box(),
      );
    }

    strategy
      .handle_fetch_remote_no_follow_no_cache(
        url,
        maybe_accept,
        maybe_checksum,
        maybe_auth,
      )
      .await
  }

  async fn fetch_remote_no_follow_no_cache(
    &self,
    url: &Url,
    maybe_accept: Option<&str>,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    let maybe_etag_cache_entry = self
      .http_cache
      .cache_item_key(url)
      .ok()
      .and_then(|key| self.http_cache.get(&key, maybe_checksum).ok().flatten())
      .and_then(|mut cache_entry| {
        cache_entry
          .metadata
          .headers
          .remove("etag")
          .map(|etag| (cache_entry, etag))
      });

    let maybe_auth_token = self.auth_tokens.get(url);
    match self
      .send_request(SendRequestArgs {
        url,
        maybe_accept,
        maybe_auth: maybe_auth.clone(),
        maybe_auth_token,
        maybe_etag: maybe_etag_cache_entry
          .as_ref()
          .map(|(_, etag)| etag.as_str()),
      })
      .await?
    {
      SendRequestResponse::NotModified => {
        let (cache_entry, _) = maybe_etag_cache_entry.unwrap();
        FileOrRedirect::from_deno_cache_entry(url, cache_entry).map_err(|err| {
          FetchNoFollowErrorKind::RedirectResolution(err).into_box()
        })
      }
      SendRequestResponse::Redirect(redirect_url, headers) => {
        self.http_cache.set(url, headers, &[]).map_err(|source| {
          FetchNoFollowErrorKind::CacheSave {
            url: url.clone(),
            source,
          }
        })?;
        Ok(FileOrRedirect::Redirect(redirect_url))
      }
      SendRequestResponse::Code(bytes, headers) => {
        self.http_cache.set(url, headers.clone(), &bytes).map_err(
          |source| FetchNoFollowErrorKind::CacheSave {
            url: url.clone(),
            source,
          },
        )?;
        if let Some(checksum) = &maybe_checksum {
          checksum
            .check(url, &bytes)
            .map_err(|err| FetchNoFollowErrorKind::ChecksumIntegrity(*err))?;
        }
        Ok(FileOrRedirect::File(File {
          url: url.clone(),
          mtime: None,
          maybe_headers: Some(headers),
          #[allow(clippy::disallowed_types, reason = "ok for source")]
          source: std::sync::Arc::from(bytes),
          loaded_from: LoadedFrom::Remote,
        }))
      }
    }
  }

  /// Returns if the cache should be used for a given specifier.
  fn should_use_cache(&self, url: &Url, cache_setting: &CacheSetting) -> bool {
    match cache_setting {
      CacheSetting::ReloadAll => false,
      CacheSetting::Use | CacheSetting::Only => true,
      CacheSetting::RespectHeaders => {
        let Ok(cache_key) = self.http_cache.cache_item_key(url) else {
          return false;
        };
        let Ok(Some(headers)) = self.http_cache.read_headers(&cache_key) else {
          return false;
        };
        let Ok(Some(download_time)) =
          self.http_cache.read_download_time(&cache_key)
        else {
          return false;
        };
        let cache_semantics =
          CacheSemantics::new(headers, download_time, self.sys.sys_time_now());
        cache_semantics.should_use()
      }
      CacheSetting::ReloadSome(list) => {
        let mut url = url.clone();
        url.set_fragment(None);
        if list.iter().any(|x| x == url.as_str()) {
          return false;
        }
        url.set_query(None);
        let mut path = PathBuf::from(url.as_str());
        loop {
          if list.contains(&path.to_str().unwrap().to_string()) {
            return false;
          }
          if !path.pop() {
            break;
          }
        }
        true
      }
    }
  }

  /// Asynchronously fetches the given HTTP URL one pass only.
  /// If no redirect is present and no error occurs,
  /// yields Code(ResultPayload).
  /// If redirect occurs, does not follow and
  /// yields Redirect(url).
  async fn send_request(
    &self,
    args: SendRequestArgs<'_>,
  ) -> Result<SendRequestResponse, FetchNoFollowError> {
    let mut headers = HeaderMap::with_capacity(3);

    if let Some(etag) = args.maybe_etag {
      let if_none_match_val =
        HeaderValue::from_str(etag).map_err(|source| {
          FetchNoFollowErrorKind::InvalidHeader {
            name: "etag",
            source,
          }
        })?;
      headers.insert(IF_NONE_MATCH, if_none_match_val);
    }
    if let Some(auth_token) = args.maybe_auth_token {
      let authorization_val = HeaderValue::from_str(&auth_token.to_string())
        .map_err(|source| FetchNoFollowErrorKind::InvalidHeader {
          name: "authorization",
          source,
        })?;
      headers.insert(AUTHORIZATION, authorization_val);
    } else if let Some((header, value)) = args.maybe_auth {
      headers.insert(header, value);
    }
    if let Some(accept) = args.maybe_accept {
      let accepts_val = HeaderValue::from_str(accept).map_err(|source| {
        FetchNoFollowErrorKind::InvalidHeader {
          name: "accept",
          source,
        }
      })?;
      headers.insert(ACCEPT, accepts_val);
    }
    match self.http_client.send_no_follow(args.url, headers).await {
      Ok(resp) => match resp {
        SendResponse::NotModified => Ok(SendRequestResponse::NotModified),
        SendResponse::Redirect(headers) => {
          let new_url = resolve_redirect_from_headers(args.url, &headers)
            .map_err(|err| {
              FetchNoFollowErrorKind::RedirectHeaderParse(*err).into_box()
            })?;
          Ok(SendRequestResponse::Redirect(
            new_url,
            response_headers_to_headers_map(headers),
          ))
        }
        SendResponse::Success(headers, body) => Ok(SendRequestResponse::Code(
          body,
          response_headers_to_headers_map(headers),
        )),
      },
      Err(err) => match err {
        SendError::Failed(err) => Err(
          FetchNoFollowErrorKind::FetchingRemote {
            url: args.url.clone(),
            source: err,
          }
          .into_box(),
        ),
        SendError::NotFound => {
          Err(FetchNoFollowErrorKind::NotFound(args.url.clone()).into_box())
        }
        SendError::StatusCode(status_code) => Err(
          FetchNoFollowErrorKind::ClientError {
            url: args.url.clone(),
            status_code,
          }
          .into_box(),
        ),
      },
    }
  }

  /// Fetch a source file from the local file system.
  pub fn fetch_local(
    &self,
    url: &Url,
    options: &FetchLocalOptions,
  ) -> Result<Option<File>, FetchLocalError> {
    let local = url_to_file_path(url)?;
    let Some(file) = self.handle_open_file(url, &local)? else {
      return Ok(None);
    };
    match self.fetch_local_inner(file, url, &local, options) {
      Ok(file) => Ok(Some(file)),
      Err(err) => Err(
        FetchLocalErrorKind::ReadingFile(FailedReadingLocalFileError {
          url: url.clone(),
          source: err,
        })
        .into_box(),
      ),
    }
  }

  fn handle_open_file(
    &self,
    url: &Url,
    path: &Path,
  ) -> Result<Option<TSys::File>, FetchLocalError> {
    match self.sys.fs_open(path, &OpenOptions::new_read()) {
      Ok(file) => Ok(Some(file)),
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(err) => Err(
        FetchLocalErrorKind::ReadingFile(FailedReadingLocalFileError {
          url: url.clone(),
          source: err,
        })
        .into_box(),
      ),
    }
  }

  fn fetch_local_inner(
    &self,
    mut file: TSys::File,
    url: &Url,
    path: &Path,
    options: &FetchLocalOptions,
  ) -> std::io::Result<File> {
    let mtime = if options.include_mtime {
      file.fs_file_metadata().and_then(|m| m.modified()).ok()
    } else {
      None
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    // If it doesnt have a extension, we want to treat it as typescript by default
    let headers = if path.extension().is_none() {
      Some(HashMap::from([(
        "content-type".to_string(),
        "application/typescript".to_string(),
      )]))
    } else {
      None
    };
    Ok(File {
      url: url.clone(),
      mtime,
      maybe_headers: headers,
      loaded_from: LoadedFrom::Local,
      source: bytes.into(),
    })
  }
}

#[async_trait::async_trait(?Send)]
trait FetchOrEnsureCacheStrategy {
  type ReturnValue;

  fn handle_memory_file(&self, file: File) -> Self::ReturnValue;

  fn handle_local(
    &self,
    url: &Url,
    options: &FetchLocalOptions,
  ) -> Result<Option<Self::ReturnValue>, FetchLocalError>;

  fn handle_data_url(
    &self,
    url: &Url,
  ) -> Result<Self::ReturnValue, DataUrlDecodeError>;

  async fn handle_blob_url(
    &self,
    url: &Url,
  ) -> Result<Self::ReturnValue, FetchNoFollowError>;

  fn handle_fetch_cached_no_follow(
    &self,
    url: &Url,
    maybe_checksum: Option<Checksum<'_>>,
  ) -> Result<Option<Self::ReturnValue>, FetchCachedNoFollowError>;

  async fn handle_fetch_remote_no_follow_no_cache(
    &self,
    url: &Url,
    maybe_accept: Option<&str>,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<Self::ReturnValue, FetchNoFollowError>;
}

struct FetchStrategy<
  'a,
  TBlobStore: BlobStore,
  TSys: FileFetcherSys,
  THttpClient: HttpClient,
>(&'a FileFetcher<TBlobStore, TSys, THttpClient>);

#[async_trait::async_trait(?Send)]
impl<TBlobStore: BlobStore, TSys: FileFetcherSys, THttpClient: HttpClient>
  FetchOrEnsureCacheStrategy
  for FetchStrategy<'_, TBlobStore, TSys, THttpClient>
{
  type ReturnValue = FileOrRedirect;

  fn handle_memory_file(&self, file: File) -> FileOrRedirect {
    FileOrRedirect::File(file)
  }

  fn handle_local(
    &self,
    url: &Url,
    options: &FetchLocalOptions,
  ) -> Result<Option<FileOrRedirect>, FetchLocalError> {
    self
      .0
      .fetch_local(url, options)
      .map(|maybe_value| maybe_value.map(FileOrRedirect::File))
  }

  fn handle_data_url(
    &self,
    url: &Url,
  ) -> Result<FileOrRedirect, DataUrlDecodeError> {
    self.0.fetch_data_url(url).map(FileOrRedirect::File)
  }

  async fn handle_blob_url(
    &self,
    url: &Url,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    self.0.fetch_blob_url(url).await.map(FileOrRedirect::File)
  }

  fn handle_fetch_cached_no_follow(
    &self,
    url: &Url,
    maybe_checksum: Option<Checksum<'_>>,
  ) -> Result<Option<FileOrRedirect>, FetchCachedNoFollowError> {
    self.0.fetch_cached_no_follow(url, maybe_checksum)
  }

  async fn handle_fetch_remote_no_follow_no_cache(
    &self,
    url: &Url,
    maybe_accept: Option<&str>,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    self
      .0
      .fetch_remote_no_follow_no_cache(
        url,
        maybe_accept,
        maybe_checksum,
        maybe_auth,
      )
      .await
  }
}

struct EnsureCachedStrategy<
  'a,
  TBlobStore: BlobStore,
  TSys: FileFetcherSys,
  THttpClient: HttpClient,
>(&'a FileFetcher<TBlobStore, TSys, THttpClient>);

#[async_trait::async_trait(?Send)]
impl<TBlobStore: BlobStore, TSys: FileFetcherSys, THttpClient: HttpClient>
  FetchOrEnsureCacheStrategy
  for EnsureCachedStrategy<'_, TBlobStore, TSys, THttpClient>
{
  type ReturnValue = CachedOrRedirect;

  fn handle_memory_file(&self, _file: File) -> CachedOrRedirect {
    CachedOrRedirect::Cached
  }

  fn handle_local(
    &self,
    url: &Url,
    _options: &FetchLocalOptions,
  ) -> Result<Option<CachedOrRedirect>, FetchLocalError> {
    let path = url_to_file_path(url)?;
    let maybe_file = self.0.handle_open_file(url, &path)?;
    Ok(maybe_file.map(|_| CachedOrRedirect::Cached))
  }

  fn handle_data_url(
    &self,
    _url: &Url,
  ) -> Result<CachedOrRedirect, DataUrlDecodeError> {
    Ok(CachedOrRedirect::Cached)
  }

  async fn handle_blob_url(
    &self,
    _url: &Url,
  ) -> Result<CachedOrRedirect, FetchNoFollowError> {
    Ok(CachedOrRedirect::Cached)
  }

  fn handle_fetch_cached_no_follow(
    &self,
    url: &Url,
    _maybe_checksum: Option<Checksum<'_>>,
  ) -> Result<Option<CachedOrRedirect>, FetchCachedNoFollowError> {
    // We don't take into account the checksum here because we assume
    // the bytes were verified when initially downloading the data
    // from the remote server. This is to prevent loading the data into
    // memory.
    if self.0.http_cache.contains(url) {
      Ok(Some(CachedOrRedirect::Cached))
    } else {
      Ok(None)
    }
  }

  async fn handle_fetch_remote_no_follow_no_cache(
    &self,
    url: &Url,
    maybe_accept: Option<&str>,
    maybe_checksum: Option<Checksum<'_>>,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<CachedOrRedirect, FetchNoFollowError> {
    self
      .0
      .fetch_remote_no_follow_no_cache(
        url,
        maybe_accept,
        maybe_checksum,
        maybe_auth,
      )
      .await
      .map(|file_or_redirect| file_or_redirect.into())
  }
}

fn response_headers_to_headers_map(response_headers: HeaderMap) -> HeadersMap {
  let mut result_headers = HashMap::with_capacity(response_headers.len());
  // todo(dsherret): change to consume to avoid allocations
  for key in response_headers.keys() {
    let key_str = key.to_string();
    let values = response_headers.get_all(key);
    // todo(dsherret): this seems very strange storing them comma separated
    // like this... what happens if a value contains a comma?
    let values_str = values
      .iter()
      .filter_map(|e| Some(e.to_str().ok()?.to_string()))
      .collect::<Vec<String>>()
      .join(",");
    result_headers.insert(key_str, values_str);
  }
  result_headers
}

pub fn resolve_redirect_from_headers(
  request_url: &Url,
  headers: &HeaderMap,
) -> Result<Url, Box<RedirectHeaderParseError>> {
  if let Some(location) = headers.get(LOCATION) {
    let location_string = location.to_str().map_err(|source| {
      Box::new(RedirectHeaderParseError {
        request_url: request_url.clone(),
        maybe_location: None,
        maybe_source: Some(source.into()),
      })
    })?;
    log::debug!("Redirecting to {:?}...", &location_string);
    resolve_url_from_location(request_url, location_string).map_err(|source| {
      Box::new(RedirectHeaderParseError {
        request_url: request_url.clone(),
        maybe_location: Some(location_string.to_string()),
        maybe_source: Some(source),
      })
    })
  } else {
    Err(Box::new(RedirectHeaderParseError {
      request_url: request_url.clone(),
      maybe_location: None,
      maybe_source: None,
    }))
  }
}

/// Construct the next uri based on base uri and location header fragment
/// See <https://tools.ietf.org/html/rfc3986#section-4.2>
fn resolve_url_from_location(
  base_url: &Url,
  location: &str,
) -> Result<Url, Box<dyn std::error::Error + Send + Sync>> {
  // todo(dsherret): these shouldn't unwrap
  if location.starts_with("http://") || location.starts_with("https://") {
    // absolute uri
    Ok(Url::parse(location)?)
  } else if location.starts_with("//") {
    // "//" authority path-abempty
    Ok(Url::parse(&format!("{}:{}", base_url.scheme(), location))?)
  } else if location.starts_with('/') {
    // path-absolute
    Ok(base_url.join(location)?)
  } else {
    // assuming path-noscheme | path-empty
    let base_url_path_str = base_url.path().to_owned();
    // Pop last part or url (after last slash)
    let segs: Vec<&str> = base_url_path_str.rsplitn(2, '/').collect();
    let new_path = format!("{}/{}", segs.last().unwrap_or(&""), location);
    Ok(base_url.join(&new_path)?)
  }
}

#[derive(Debug)]
struct SendRequestArgs<'a> {
  pub url: &'a Url,
  pub maybe_accept: Option<&'a str>,
  pub maybe_etag: Option<&'a str>,
  pub maybe_auth_token: Option<&'a AuthToken>,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
}

#[derive(Debug, Eq, PartialEq)]
enum SendRequestResponse {
  Code(Vec<u8>, HeadersMap),
  NotModified,
  Redirect(Url, HeadersMap),
}

#[cfg(test)]
mod test {
  use url::Url;

  use crate::file_fetcher::resolve_url_from_location;

  #[test]
  fn test_resolve_url_from_location_full_1() {
    let url = "http://deno.land".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "http://golang.org").unwrap();
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_full_2() {
    let url = "https://deno.land".parse::<Url>().unwrap();
    let new_uri =
      resolve_url_from_location(&url, "https://golang.org").unwrap();
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_relative_1() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri =
      resolve_url_from_location(&url, "//rust-lang.org/en-US").unwrap();
    assert_eq!(new_uri.host_str().unwrap(), "rust-lang.org");
    assert_eq!(new_uri.path(), "/en-US");
  }

  #[test]
  fn test_resolve_url_from_location_relative_2() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "/y").unwrap();
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/y");
  }

  #[test]
  fn test_resolve_url_from_location_relative_3() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "z").unwrap();
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/z");
  }
}
