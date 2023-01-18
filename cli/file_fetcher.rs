// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CacheSetting;
use crate::auth_tokens::AuthToken;
use crate::auth_tokens::AuthTokens;
use crate::cache::HttpCache;
use crate::colors;
use crate::http_util;
use crate::http_util::resolve_redirect_from_response;
use crate::http_util::CacheSemantics;
use crate::http_util::HeadersMap;
use crate::http_util::HttpClient;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::UpdateGuard;
use crate::util::text_encoding;

use data_url::DataUrl;
use deno_ast::MediaType;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_fetch::reqwest::header::HeaderValue;
use deno_runtime::deno_fetch::reqwest::header::ACCEPT;
use deno_runtime::deno_fetch::reqwest::header::AUTHORIZATION;
use deno_runtime::deno_fetch::reqwest::header::IF_NONE_MATCH;
use deno_runtime::deno_fetch::reqwest::StatusCode;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::PermissionsContainer;
use log::debug;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

pub const SUPPORTED_SCHEMES: [&str; 5] =
  ["data", "blob", "file", "http", "https"];

/// A structure representing a source file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
  /// The path to the local version of the source file.  For local files this
  /// will be the direct path to that file.  For remote files, it will be the
  /// path to the file in the HTTP cache.
  pub local: PathBuf,
  /// For remote files, if there was an `X-TypeScript-Type` header, the parsed
  /// out value of that header.
  pub maybe_types: Option<String>,
  /// The resolved media type for the file.
  pub media_type: MediaType,
  /// The source of the file as a string.
  pub source: Arc<str>,
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,

  pub maybe_headers: Option<HashMap<String, String>>,
}

/// Simple struct implementing in-process caching to prevent multiple
/// fs reads/net fetches for same file.
#[derive(Debug, Clone, Default)]
struct FileCache(Arc<Mutex<HashMap<ModuleSpecifier, File>>>);

impl FileCache {
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<File> {
    let cache = self.0.lock();
    cache.get(specifier).cloned()
  }

  pub fn insert(&self, specifier: ModuleSpecifier, file: File) -> Option<File> {
    let mut cache = self.0.lock();
    cache.insert(specifier, file)
  }
}

/// Fetch a source file from the local file system.
fn fetch_local(specifier: &ModuleSpecifier) -> Result<File, AnyError> {
  let local = specifier.to_file_path().map_err(|_| {
    uri_error(format!("Invalid file path.\n  Specifier: {}", specifier))
  })?;
  let bytes = fs::read(&local)?;
  let charset = text_encoding::detect_charset(&bytes).to_string();
  let source = get_source_from_bytes(bytes, Some(charset))?;
  let media_type = MediaType::from(specifier);

  Ok(File {
    local,
    maybe_types: None,
    media_type,
    source: source.into(),
    specifier: specifier.clone(),
    maybe_headers: None,
  })
}

/// Returns the decoded body and content-type of a provided
/// data URL.
pub fn get_source_from_data_url(
  specifier: &ModuleSpecifier,
) -> Result<(String, String), AnyError> {
  let data_url = DataUrl::process(specifier.as_str())
    .map_err(|e| uri_error(format!("{:?}", e)))?;
  let mime = data_url.mime_type();
  let charset = mime.get_parameter("charset").map(|v| v.to_string());
  let (bytes, _) = data_url
    .decode_to_vec()
    .map_err(|e| uri_error(format!("{:?}", e)))?;
  Ok((get_source_from_bytes(bytes, charset)?, format!("{}", mime)))
}

/// Given a vector of bytes and optionally a charset, decode the bytes to a
/// string.
pub fn get_source_from_bytes(
  bytes: Vec<u8>,
  maybe_charset: Option<String>,
) -> Result<String, AnyError> {
  let source = if let Some(charset) = maybe_charset {
    text_encoding::convert_to_utf8(&bytes, &charset)?.to_string()
  } else {
    String::from_utf8(bytes)?
  };

  Ok(source)
}

/// Return a validated scheme for a given module specifier.
fn get_validated_scheme(
  specifier: &ModuleSpecifier,
) -> Result<String, AnyError> {
  let scheme = specifier.scheme();
  if !SUPPORTED_SCHEMES.contains(&scheme) {
    Err(generic_error(format!(
      "Unsupported scheme \"{}\" for module \"{}\". Supported schemes: {:#?}",
      scheme, specifier, SUPPORTED_SCHEMES
    )))
  } else {
    Ok(scheme.to_string())
  }
}

/// Resolve a media type and optionally the charset from a module specifier and
/// the value of a content type header.
pub fn map_content_type(
  specifier: &ModuleSpecifier,
  maybe_content_type: Option<String>,
) -> (MediaType, Option<String>) {
  if let Some(content_type) = maybe_content_type {
    let mut content_types = content_type.split(';');
    let content_type = content_types.next().unwrap();
    let media_type = MediaType::from_content_type(specifier, content_type);
    let charset = content_types
      .map(str::trim)
      .find_map(|s| s.strip_prefix("charset="))
      .map(String::from);

    (media_type, charset)
  } else {
    (MediaType::from(specifier), None)
  }
}

/// A structure for resolving, fetching and caching source files.
#[derive(Debug, Clone)]
pub struct FileFetcher {
  auth_tokens: AuthTokens,
  allow_remote: bool,
  cache: FileCache,
  cache_setting: CacheSetting,
  pub http_cache: HttpCache,
  http_client: HttpClient,
  blob_store: BlobStore,
  download_log_level: log::Level,
  progress_bar: Option<ProgressBar>,
}

impl FileFetcher {
  pub fn new(
    http_cache: HttpCache,
    cache_setting: CacheSetting,
    allow_remote: bool,
    http_client: HttpClient,
    blob_store: BlobStore,
    progress_bar: Option<ProgressBar>,
  ) -> Result<Self, AnyError> {
    Ok(Self {
      auth_tokens: AuthTokens::new(env::var("DENO_AUTH_TOKENS").ok()),
      allow_remote,
      cache: Default::default(),
      cache_setting,
      http_cache,
      http_client,
      blob_store,
      download_log_level: log::Level::Info,
      progress_bar,
    })
  }

  /// Sets the log level to use when outputting the download message.
  pub fn set_download_log_level(&mut self, level: log::Level) {
    self.download_log_level = level;
  }

  /// Creates a `File` structure for a remote file.
  fn build_remote_file(
    &self,
    specifier: &ModuleSpecifier,
    bytes: Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<File, AnyError> {
    let local =
      self
        .http_cache
        .get_cache_filename(specifier)
        .ok_or_else(|| {
          generic_error("Cannot convert specifier to cached filename.")
        })?;
    let maybe_content_type = headers.get("content-type").cloned();
    let (media_type, maybe_charset) =
      map_content_type(specifier, maybe_content_type);
    let source = get_source_from_bytes(bytes, maybe_charset)?;
    let maybe_types = match media_type {
      MediaType::JavaScript
      | MediaType::Cjs
      | MediaType::Mjs
      | MediaType::Jsx => headers.get("x-typescript-types").cloned(),
      _ => None,
    };

    Ok(File {
      local,
      maybe_types,
      media_type,
      source: source.into(),
      specifier: specifier.clone(),
      maybe_headers: Some(headers.clone()),
    })
  }

  /// Fetch cached remote file.
  ///
  /// This is a recursive operation if source file has redirections.
  pub fn fetch_cached(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: i64,
  ) -> Result<Option<File>, AnyError> {
    debug!("FileFetcher::fetch_cached - specifier: {}", specifier);
    if redirect_limit < 0 {
      return Err(custom_error("Http", "Too many redirects."));
    }

    let (mut source_file, headers, _) = match self.http_cache.get(specifier) {
      Err(err) => {
        if let Some(err) = err.downcast_ref::<std::io::Error>() {
          if err.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
          }
        }
        return Err(err);
      }
      Ok(cache) => cache,
    };
    if let Some(redirect_to) = headers.get("location") {
      let redirect =
        deno_core::resolve_import(redirect_to, specifier.as_str())?;
      return self.fetch_cached(&redirect, redirect_limit - 1);
    }
    let mut bytes = Vec::new();
    source_file.read_to_end(&mut bytes)?;
    let file = self.build_remote_file(specifier, bytes, &headers)?;

    Ok(Some(file))
  }

  /// Convert a data URL into a file, resulting in an error if the URL is
  /// invalid.
  fn fetch_data_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    debug!("FileFetcher::fetch_data_url() - specifier: {}", specifier);
    match self.fetch_cached(specifier, 0) {
      Ok(Some(file)) => return Ok(file),
      Ok(None) => {}
      Err(err) => return Err(err),
    }

    if self.cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "Specifier not found in cache: \"{}\", --cached-only is specified.",
          specifier
        ),
      ));
    }

    let (source, content_type) = get_source_from_data_url(specifier)?;
    let (media_type, _) =
      map_content_type(specifier, Some(content_type.clone()));

    let local =
      self
        .http_cache
        .get_cache_filename(specifier)
        .ok_or_else(|| {
          generic_error("Cannot convert specifier to cached filename.")
        })?;
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), content_type);
    self
      .http_cache
      .set(specifier, headers.clone(), source.as_bytes())?;

    Ok(File {
      local,
      maybe_types: None,
      media_type,
      source: source.into(),
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
    })
  }

  /// Get a blob URL.
  async fn fetch_blob_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    debug!("FileFetcher::fetch_blob_url() - specifier: {}", specifier);
    match self.fetch_cached(specifier, 0) {
      Ok(Some(file)) => return Ok(file),
      Ok(None) => {}
      Err(err) => return Err(err),
    }

    if self.cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "Specifier not found in cache: \"{}\", --cached-only is specified.",
          specifier
        ),
      ));
    }

    let blob = {
      let blob_store = self.blob_store.borrow();
      blob_store
        .get_object_url(specifier.clone())
        .ok_or_else(|| {
          custom_error(
            "NotFound",
            format!("Blob URL not found: \"{}\".", specifier),
          )
        })?
    };

    let content_type = blob.media_type.clone();
    let bytes = blob.read_all().await?;

    let (media_type, maybe_charset) =
      map_content_type(specifier, Some(content_type.clone()));
    let source = get_source_from_bytes(bytes, maybe_charset)?;

    let local =
      self
        .http_cache
        .get_cache_filename(specifier)
        .ok_or_else(|| {
          generic_error("Cannot convert specifier to cached filename.")
        })?;
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), content_type);
    self
      .http_cache
      .set(specifier, headers.clone(), source.as_bytes())?;

    Ok(File {
      local,
      maybe_types: None,
      media_type,
      source: source.into(),
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
    })
  }

  /// Asynchronously fetch remote source file specified by the URL following
  /// redirects.
  ///
  /// **Note** this is a recursive method so it can't be "async", but needs to
  /// return a `Pin<Box<..>>`.
  fn fetch_remote(
    &self,
    specifier: &ModuleSpecifier,
    permissions: PermissionsContainer,
    redirect_limit: i64,
    maybe_accept: Option<String>,
  ) -> Pin<Box<dyn Future<Output = Result<File, AnyError>> + Send>> {
    debug!("FileFetcher::fetch_remote() - specifier: {}", specifier);
    if redirect_limit < 0 {
      return futures::future::err(custom_error("Http", "Too many redirects."))
        .boxed();
    }

    if let Err(err) = permissions.check_specifier(specifier) {
      return futures::future::err(err).boxed();
    }

    if self.should_use_cache(specifier) {
      match self.fetch_cached(specifier, redirect_limit) {
        Ok(Some(file)) => {
          return futures::future::ok(file).boxed();
        }
        Ok(None) => {}
        Err(err) => {
          return futures::future::err(err).boxed();
        }
      }
    }

    if self.cache_setting == CacheSetting::Only {
      return futures::future::err(custom_error(
        "NotCached",
        format!(
          "Specifier not found in cache: \"{}\", --cached-only is specified.",
          specifier
        ),
      ))
      .boxed();
    }

    let mut maybe_progress_guard = None;
    if let Some(pb) = self.progress_bar.as_ref() {
      maybe_progress_guard = Some(pb.update(specifier.as_str()));
    } else {
      log::log!(
        self.download_log_level,
        "{} {}",
        colors::green("Download"),
        specifier
      );
    }

    let maybe_etag = match self.http_cache.get(specifier) {
      Ok((_, headers, _)) => headers.get("etag").cloned(),
      _ => None,
    };
    let maybe_auth_token = self.auth_tokens.get(specifier);
    let specifier = specifier.clone();
    let client = self.http_client.clone();
    let file_fetcher = self.clone();
    // A single pass of fetch either yields code or yields a redirect.
    async move {
      let result = match fetch_once(
        &client,
        FetchOnceArgs {
          url: specifier.clone(),
          maybe_accept: maybe_accept.clone(),
          maybe_etag,
          maybe_auth_token,
          maybe_progress_guard: maybe_progress_guard.as_ref(),
        },
      )
      .await?
      {
        FetchOnceResult::NotModified => {
          let file = file_fetcher.fetch_cached(&specifier, 10)?.unwrap();
          Ok(file)
        }
        FetchOnceResult::Redirect(redirect_url, headers) => {
          file_fetcher.http_cache.set(&specifier, headers, &[])?;
          file_fetcher
            .fetch_remote(
              &redirect_url,
              permissions,
              redirect_limit - 1,
              maybe_accept,
            )
            .await
        }
        FetchOnceResult::Code(bytes, headers) => {
          file_fetcher
            .http_cache
            .set(&specifier, headers.clone(), &bytes)?;
          let file =
            file_fetcher.build_remote_file(&specifier, bytes, &headers)?;
          Ok(file)
        }
      };
      drop(maybe_progress_guard);
      result
    }
    .boxed()
  }

  /// Returns if the cache should be used for a given specifier.
  fn should_use_cache(&self, specifier: &ModuleSpecifier) -> bool {
    match &self.cache_setting {
      CacheSetting::ReloadAll => false,
      CacheSetting::Use | CacheSetting::Only => true,
      CacheSetting::RespectHeaders => {
        if let Ok((_, headers, cache_time)) = self.http_cache.get(specifier) {
          let cache_semantics =
            CacheSemantics::new(headers, cache_time, SystemTime::now());
          cache_semantics.should_use()
        } else {
          false
        }
      }
      CacheSetting::ReloadSome(list) => {
        let mut url = specifier.clone();
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

  /// Fetch a source file and asynchronously return it.
  pub async fn fetch(
    &self,
    specifier: &ModuleSpecifier,
    permissions: PermissionsContainer,
  ) -> Result<File, AnyError> {
    debug!("FileFetcher::fetch() - specifier: {}", specifier);
    self.fetch_with_accept(specifier, permissions, None).await
  }

  pub async fn fetch_with_accept(
    &self,
    specifier: &ModuleSpecifier,
    permissions: PermissionsContainer,
    maybe_accept: Option<&str>,
  ) -> Result<File, AnyError> {
    let scheme = get_validated_scheme(specifier)?;
    permissions.check_specifier(specifier)?;
    if let Some(file) = self.cache.get(specifier) {
      Ok(file)
    } else if scheme == "file" {
      // we do not in memory cache files, as this would prevent files on the
      // disk changing effecting things like workers and dynamic imports.
      fetch_local(specifier)
    } else if scheme == "data" {
      let result = self.fetch_data_url(specifier);
      if let Ok(file) = &result {
        self.cache.insert(specifier.clone(), file.clone());
      }
      result
    } else if scheme == "blob" {
      let result = self.fetch_blob_url(specifier).await;
      if let Ok(file) = &result {
        self.cache.insert(specifier.clone(), file.clone());
      }
      result
    } else if !self.allow_remote {
      Err(custom_error(
        "NoRemote",
        format!("A remote specifier was requested: \"{}\", but --no-remote is specified.", specifier),
      ))
    } else {
      let result = self
        .fetch_remote(
          specifier,
          permissions,
          10,
          maybe_accept.map(String::from),
        )
        .await;
      if let Ok(file) = &result {
        self.cache.insert(specifier.clone(), file.clone());
      }
      result
    }
  }

  pub fn get_local_path(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    // TODO(@kitsonk) fix when deno_graph does not query cache for synthetic
    // modules
    if specifier.scheme() == "flags" {
      None
    } else if specifier.scheme() == "file" {
      specifier.to_file_path().ok()
    } else {
      self.http_cache.get_cache_filename(specifier)
    }
  }

  /// Get the location of the current HTTP cache associated with the fetcher.
  pub fn get_http_cache_location(&self) -> PathBuf {
    self.http_cache.location.clone()
  }

  /// A synchronous way to retrieve a source file, where if the file has already
  /// been cached in memory it will be returned, otherwise for local files will
  /// be read from disk.
  pub fn get_source(&self, specifier: &ModuleSpecifier) -> Option<File> {
    let maybe_file = self.cache.get(specifier);
    if maybe_file.is_none() {
      let is_local = specifier.scheme() == "file";
      if is_local {
        if let Ok(file) = fetch_local(specifier) {
          return Some(file);
        }
      }
      None
    } else {
      maybe_file
    }
  }

  /// Insert a temporary module into the in memory cache for the file fetcher.
  pub fn insert_cached(&self, file: File) -> Option<File> {
    self.cache.insert(file.specifier.clone(), file)
  }
}

#[derive(Debug, Eq, PartialEq)]
enum FetchOnceResult {
  Code(Vec<u8>, HeadersMap),
  NotModified,
  Redirect(Url, HeadersMap),
}

#[derive(Debug)]
struct FetchOnceArgs<'a> {
  pub url: Url,
  pub maybe_accept: Option<String>,
  pub maybe_etag: Option<String>,
  pub maybe_auth_token: Option<AuthToken>,
  pub maybe_progress_guard: Option<&'a UpdateGuard>,
}

/// Asynchronously fetches the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(ResultPayload).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
async fn fetch_once<'a>(
  http_client: &HttpClient,
  args: FetchOnceArgs<'a>,
) -> Result<FetchOnceResult, AnyError> {
  let mut request = http_client.get_no_redirect(args.url.clone());

  if let Some(etag) = args.maybe_etag {
    let if_none_match_val = HeaderValue::from_str(&etag)?;
    request = request.header(IF_NONE_MATCH, if_none_match_val);
  }
  if let Some(auth_token) = args.maybe_auth_token {
    let authorization_val = HeaderValue::from_str(&auth_token.to_string())?;
    request = request.header(AUTHORIZATION, authorization_val);
  }
  if let Some(accept) = args.maybe_accept {
    let accepts_val = HeaderValue::from_str(&accept)?;
    request = request.header(ACCEPT, accepts_val);
  }
  let response = request.send().await?;

  if response.status() == StatusCode::NOT_MODIFIED {
    return Ok(FetchOnceResult::NotModified);
  }

  let mut result_headers = HashMap::new();
  let response_headers = response.headers();

  if let Some(warning) = response_headers.get("X-Deno-Warning") {
    log::warn!(
      "{} {}",
      crate::colors::yellow("Warning"),
      warning.to_str().unwrap()
    );
  }

  for key in response_headers.keys() {
    let key_str = key.to_string();
    let values = response_headers.get_all(key);
    let values_str = values
      .iter()
      .map(|e| e.to_str().unwrap().to_string())
      .collect::<Vec<String>>()
      .join(",");
    result_headers.insert(key_str, values_str);
  }

  if response.status().is_redirection() {
    let new_url = resolve_redirect_from_response(&args.url, &response)?;
    return Ok(FetchOnceResult::Redirect(new_url, result_headers));
  }

  if response.status().is_client_error() || response.status().is_server_error()
  {
    let err = if response.status() == StatusCode::NOT_FOUND {
      custom_error(
        "NotFound",
        format!("Import '{}' failed, not found.", args.url),
      )
    } else {
      generic_error(format!(
        "Import '{}' failed: {}",
        args.url,
        response.status()
      ))
    };
    return Err(err);
  }

  let body = http_util::get_response_body_with_progress(
    response,
    args.maybe_progress_guard,
  )
  .await?;

  Ok(FetchOnceResult::Code(body, result_headers))
}

#[cfg(test)]
mod tests {
  use crate::cache::CachedUrlMetadata;
  use crate::http_util::HttpClient;
  use crate::version;

  use super::*;
  use deno_core::error::get_custom_error_class;
  use deno_core::resolve_url;
  use deno_core::resolve_url_or_path;
  use deno_core::url::Url;
  use deno_runtime::deno_fetch::create_http_client;
  use deno_runtime::deno_web::Blob;
  use deno_runtime::deno_web::InMemoryBlobPart;
  use std::fs::read;
  use test_util::TempDir;

  fn setup(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (FileFetcher, TempDir) {
    let (file_fetcher, temp_dir, _) =
      setup_with_blob_store(cache_setting, maybe_temp_dir);
    (file_fetcher, temp_dir)
  }

  fn setup_with_blob_store(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (FileFetcher, TempDir, BlobStore) {
    let temp_dir = maybe_temp_dir.unwrap_or_default();
    let location = temp_dir.path().join("deps");
    let blob_store = BlobStore::default();
    let file_fetcher = FileFetcher::new(
      HttpCache::new(&location),
      cache_setting,
      true,
      HttpClient::new(None, None).unwrap(),
      blob_store.clone(),
      None,
    )
    .unwrap();
    (file_fetcher, temp_dir, blob_store)
  }

  macro_rules! file_url {
    ($path:expr) => {
      if cfg!(target_os = "windows") {
        concat!("file:///C:", $path)
      } else {
        concat!("file://", $path)
      }
    };
  }

  async fn test_fetch(specifier: &ModuleSpecifier) -> (File, FileFetcher) {
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result = file_fetcher
      .fetch(specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    (result.unwrap(), file_fetcher)
  }

  async fn test_fetch_remote(
    specifier: &ModuleSpecifier,
  ) -> (File, HashMap<String, String>) {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result: Result<File, AnyError> = file_fetcher
      .fetch_remote(specifier, PermissionsContainer::allow_all(), 1, None)
      .await;
    assert!(result.is_ok());
    let (_, headers, _) = file_fetcher.http_cache.get(specifier).unwrap();
    (result.unwrap(), headers)
  }

  async fn test_fetch_remote_encoded(
    fixture: &str,
    charset: &str,
    expected: &str,
  ) {
    let url_str = format!("http://127.0.0.1:4545/encoding/{}", fixture);
    let specifier = resolve_url(&url_str).unwrap();
    let (file, headers) = test_fetch_remote(&specifier).await;
    assert_eq!(&*file.source, expected);
    assert_eq!(file.media_type, MediaType::TypeScript);
    assert_eq!(
      headers.get("content-type").unwrap(),
      &format!("application/typescript;charset={}", charset)
    );
  }

  async fn test_fetch_local_encoded(charset: &str, expected: String) {
    let p = test_util::testdata_path().join(format!("encoding/{}.ts", charset));
    let specifier = resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let (file, _) = test_fetch(&specifier).await;
    assert_eq!(&*file.source, expected);
  }

  #[test]
  fn test_get_validated_scheme() {
    let fixtures = vec![
      ("https://deno.land/x/mod.ts", true, "https"),
      ("http://deno.land/x/mod.ts", true, "http"),
      ("file:///a/b/c.ts", true, "file"),
      ("file:///C:/a/b/c.ts", true, "file"),
      ("data:,some%20text", true, "data"),
      ("ftp://a/b/c.ts", false, ""),
      ("mailto:dino@deno.land", false, ""),
    ];

    for (specifier, is_ok, expected) in fixtures {
      let specifier = resolve_url_or_path(specifier).unwrap();
      let actual = get_validated_scheme(&specifier);
      assert_eq!(actual.is_ok(), is_ok);
      if is_ok {
        assert_eq!(actual.unwrap(), expected);
      }
    }
  }

  #[test]
  fn test_map_content_type() {
    let fixtures = vec![
      // Extension only
      (file_url!("/foo/bar.ts"), None, MediaType::TypeScript, None),
      (file_url!("/foo/bar.tsx"), None, MediaType::Tsx, None),
      (file_url!("/foo/bar.d.cts"), None, MediaType::Dcts, None),
      (file_url!("/foo/bar.d.mts"), None, MediaType::Dmts, None),
      (file_url!("/foo/bar.d.ts"), None, MediaType::Dts, None),
      (file_url!("/foo/bar.js"), None, MediaType::JavaScript, None),
      (file_url!("/foo/bar.jsx"), None, MediaType::Jsx, None),
      (file_url!("/foo/bar.json"), None, MediaType::Json, None),
      (file_url!("/foo/bar.wasm"), None, MediaType::Wasm, None),
      (file_url!("/foo/bar.cjs"), None, MediaType::Cjs, None),
      (file_url!("/foo/bar.mjs"), None, MediaType::Mjs, None),
      (file_url!("/foo/bar.cts"), None, MediaType::Cts, None),
      (file_url!("/foo/bar.mts"), None, MediaType::Mts, None),
      (file_url!("/foo/bar"), None, MediaType::Unknown, None),
      // Media type no extension
      (
        "https://deno.land/x/mod",
        Some("application/typescript".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/typescript".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("video/vnd.dlna.mpeg-tts".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("video/mp2t".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("application/x-typescript".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("application/javascript".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/javascript".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("application/ecmascript".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/ecmascript".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("application/x-javascript".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("application/node".to_string()),
        MediaType::JavaScript,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/jsx".to_string()),
        MediaType::Jsx,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/tsx".to_string()),
        MediaType::Tsx,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/json".to_string()),
        MediaType::Json,
        None,
      ),
      (
        "https://deno.land/x/mod",
        Some("text/json; charset=utf-8".to_string()),
        MediaType::Json,
        Some("utf-8".to_string()),
      ),
      // Extension with media type
      (
        "https://deno.land/x/mod.ts",
        Some("text/plain".to_string()),
        MediaType::TypeScript,
        None,
      ),
      (
        "https://deno.land/x/mod.ts",
        Some("foo/bar".to_string()),
        MediaType::Unknown,
        None,
      ),
      (
        "https://deno.land/x/mod.tsx",
        Some("application/typescript".to_string()),
        MediaType::Tsx,
        None,
      ),
      (
        "https://deno.land/x/mod.tsx",
        Some("application/javascript".to_string()),
        MediaType::Tsx,
        None,
      ),
      (
        "https://deno.land/x/mod.jsx",
        Some("application/javascript".to_string()),
        MediaType::Jsx,
        None,
      ),
      (
        "https://deno.land/x/mod.jsx",
        Some("application/x-typescript".to_string()),
        MediaType::Jsx,
        None,
      ),
      (
        "https://deno.land/x/mod.d.ts",
        Some("application/javascript".to_string()),
        MediaType::Dts,
        None,
      ),
      (
        "https://deno.land/x/mod.d.ts",
        Some("text/plain".to_string()),
        MediaType::Dts,
        None,
      ),
      (
        "https://deno.land/x/mod.d.ts",
        Some("application/x-typescript".to_string()),
        MediaType::Dts,
        None,
      ),
    ];

    for (specifier, maybe_content_type, media_type, maybe_charset) in fixtures {
      let specifier = resolve_url_or_path(specifier).unwrap();
      assert_eq!(
        map_content_type(&specifier, maybe_content_type),
        (media_type, maybe_charset)
      );
    }
  }

  #[tokio::test]
  async fn test_insert_cached() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let local = temp_dir.path().join("a.ts");
    let specifier =
      resolve_url_or_path(local.as_os_str().to_str().unwrap()).unwrap();
    let file = File {
      local,
      maybe_types: None,
      media_type: MediaType::TypeScript,
      source: "some source code".into(),
      specifier: specifier.clone(),
      maybe_headers: None,
    };
    file_fetcher.insert_cached(file.clone());

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let result_file = result.unwrap();
    assert_eq!(result_file, file);
  }

  #[tokio::test]
  async fn test_get_source() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4548/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let maybe_file = file_fetcher.get_source(&specifier);
    assert!(maybe_file.is_some());
    let file = maybe_file.unwrap();
    assert_eq!(&*file.source, "export const redirect = 1;\n");
    assert_eq!(
      file.specifier,
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap()
    );
  }

  #[test]
  fn test_get_http_cache_location() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let expected = temp_dir.path().join("deps");
    let actual = file_fetcher.get_http_cache_location();
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn test_fetch_data_url() {
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export const a = \"a\";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);
    assert_eq!(file.maybe_types, None);
    assert_eq!(file.specifier, specifier);
  }

  #[tokio::test]
  async fn test_fetch_blob_url() {
    let (file_fetcher, _, blob_store) =
      setup_with_blob_store(CacheSetting::Use, None);

    let bytes =
      "export const a = \"a\";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n"
        .as_bytes()
        .to_vec();

    let specifier = blob_store.insert_object_url(
      Blob {
        media_type: "application/typescript".to_string(),
        parts: vec![Arc::new(InMemoryBlobPart::from(bytes))],
      },
      None,
    );

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export const a = \"a\";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);
    assert_eq!(file.maybe_types, None);
    assert_eq!(file.specifier, specifier);
  }

  #[tokio::test]
  async fn test_fetch_complex() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let (file_fetcher_01, _) = setup(CacheSetting::Use, Some(temp_dir.clone()));
    let (file_fetcher_02, _) = setup(CacheSetting::Use, Some(temp_dir.clone()));
    let specifier =
      resolve_url_or_path("http://localhost:4545/subdir/mod2.ts").unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);

    let cache_filename = file_fetcher
      .http_cache
      .get_cache_filename(&specifier)
      .unwrap();
    let mut metadata = CachedUrlMetadata::read(&cache_filename).unwrap();
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "text/javascript".to_string());
    metadata.write(&cache_filename).unwrap();

    let result = file_fetcher_01
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    // This validates that when using the cached value, because we modified
    // the value above.
    assert_eq!(file.media_type, MediaType::JavaScript);

    let (_, headers, _) = file_fetcher_02.http_cache.get(&specifier).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/javascript");
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "application/json".to_string());
    metadata.write(&cache_filename).unwrap();

    let result = file_fetcher_02
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::Json);

    // This creates a totally new instance, simulating another Deno process
    // invocation and indicates to "cache bust".
    let location = temp_dir.path().join("deps");
    let file_fetcher = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::ReloadAll,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);
  }

  #[tokio::test]
  async fn test_fetch_uses_cache() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("deps");
    let file_fetcher_01 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let specifier =
      resolve_url("http://localhost:4545/subdir/mismatch_ext.ts").unwrap();
    let cache_filename = file_fetcher_01
      .http_cache
      .get_cache_filename(&specifier)
      .unwrap();

    let result = file_fetcher_01
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let metadata_filename = CachedUrlMetadata::filename(&cache_filename);
    let metadata_file = fs::File::open(metadata_filename).unwrap();
    let metadata_file_metadata = metadata_file.metadata().unwrap();
    let metadata_file_modified_01 = metadata_file_metadata.modified().unwrap();

    let file_fetcher_02 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let result = file_fetcher_02
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let metadata_filename = CachedUrlMetadata::filename(&cache_filename);
    let metadata_file = fs::File::open(metadata_filename).unwrap();
    let metadata_file_metadata = metadata_file.metadata().unwrap();
    let metadata_file_modified_02 = metadata_file_metadata.modified().unwrap();

    assert_eq!(metadata_file_modified_01, metadata_file_modified_02);
  }

  #[tokio::test]
  async fn test_fetch_redirected() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4546/subdir/redirects/redirect1.js")
        .unwrap();
    let cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&specifier)
      .unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&redirected_specifier)
      .unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_specifier);

    assert_eq!(
      fs::read_to_string(cached_filename).unwrap(),
      "",
      "redirected files should have empty cached contents"
    );
    let (_, headers, _) = file_fetcher.http_cache.get(&specifier).unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "http://localhost:4545/subdir/redirects/redirect1.js"
    );

    assert_eq!(
      fs::read_to_string(redirected_cached_filename).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers, _) =
      file_fetcher.http_cache.get(&redirected_specifier).unwrap();
    assert!(headers.get("location").is_none());
  }

  #[tokio::test]
  async fn test_fetch_multiple_redirects() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4548/subdir/redirects/redirect1.js")
        .unwrap();
    let cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&specifier)
      .unwrap();
    let redirected_01_specifier =
      resolve_url("http://localhost:4546/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_01_cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&redirected_01_specifier)
      .unwrap();
    let redirected_02_specifier =
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_02_cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&redirected_02_specifier)
      .unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_02_specifier);

    assert_eq!(
      fs::read_to_string(cached_filename).unwrap(),
      "",
      "redirected files should have empty cached contents"
    );
    let (_, headers, _) = file_fetcher.http_cache.get(&specifier).unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "http://localhost:4546/subdir/redirects/redirect1.js"
    );

    assert_eq!(
      fs::read_to_string(redirected_01_cached_filename).unwrap(),
      "",
      "redirected files should have empty cached contents"
    );
    let (_, headers, _) = file_fetcher
      .http_cache
      .get(&redirected_01_specifier)
      .unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "http://localhost:4545/subdir/redirects/redirect1.js"
    );

    assert_eq!(
      fs::read_to_string(redirected_02_cached_filename).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers, _) = file_fetcher
      .http_cache
      .get(&redirected_02_specifier)
      .unwrap();
    assert!(headers.get("location").is_none());
  }

  #[tokio::test]
  async fn test_fetch_uses_cache_with_redirects() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("deps");
    let file_fetcher_01 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let specifier =
      resolve_url("http://localhost:4548/subdir/mismatch_ext.ts").unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4546/subdir/mismatch_ext.ts").unwrap();
    let redirected_cache_filename = file_fetcher_01
      .http_cache
      .get_cache_filename(&redirected_specifier)
      .unwrap();

    let result = file_fetcher_01
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let metadata_filename =
      CachedUrlMetadata::filename(&redirected_cache_filename);
    let metadata_file = fs::File::open(metadata_filename).unwrap();
    let metadata_file_metadata = metadata_file.metadata().unwrap();
    let metadata_file_modified_01 = metadata_file_metadata.modified().unwrap();

    let file_fetcher_02 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let result = file_fetcher_02
      .fetch(&redirected_specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let metadata_filename =
      CachedUrlMetadata::filename(&redirected_cache_filename);
    let metadata_file = fs::File::open(metadata_filename).unwrap();
    let metadata_file_metadata = metadata_file.metadata().unwrap();
    let metadata_file_modified_02 = metadata_file_metadata.modified().unwrap();

    assert_eq!(metadata_file_modified_01, metadata_file_modified_02);
  }

  #[tokio::test]
  async fn test_fetcher_limits_redirects() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4548/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher
      .fetch_remote(&specifier, PermissionsContainer::allow_all(), 2, None)
      .await;
    assert!(result.is_ok());

    let result = file_fetcher
      .fetch_remote(&specifier, PermissionsContainer::allow_all(), 1, None)
      .await;
    assert!(result.is_err());

    let result = file_fetcher.fetch_cached(&specifier, 2);
    assert!(result.is_ok());

    let result = file_fetcher.fetch_cached(&specifier, 1);
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_fetch_same_host_redirect() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier = resolve_url(
      "http://localhost:4550/REDIRECT/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&specifier)
      .unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4550/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_cached_filename = file_fetcher
      .http_cache
      .get_cache_filename(&redirected_specifier)
      .unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_specifier);

    assert_eq!(
      fs::read_to_string(cached_filename).unwrap(),
      "",
      "redirected files should have empty cached contents"
    );
    let (_, headers, _) = file_fetcher.http_cache.get(&specifier).unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "/subdir/redirects/redirect1.js"
    );

    assert_eq!(
      fs::read_to_string(redirected_cached_filename).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers, _) =
      file_fetcher.http_cache.get(&redirected_specifier).unwrap();
    assert!(headers.get("location").is_none());
  }

  #[tokio::test]
  async fn test_fetch_no_remote() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("deps");
    let file_fetcher = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      false,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(get_custom_error_class(&err), Some("NoRemote"));
    assert_eq!(err.to_string(), "A remote specifier was requested: \"http://localhost:4545/run/002_hello.ts\", but --no-remote is specified.");
  }

  #[tokio::test]
  async fn test_fetch_cache_only() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("deps");
    let file_fetcher_01 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Only,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let file_fetcher_02 = FileFetcher::new(
      HttpCache::new(&location),
      CacheSetting::Use,
      true,
      HttpClient::new(None, None).unwrap(),
      BlobStore::default(),
      None,
    )
    .unwrap();
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher_01
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(get_custom_error_class(&err), Some("NotCached"));
    assert_eq!(err.to_string(), "Specifier not found in cache: \"http://localhost:4545/run/002_hello.ts\", --cached-only is specified.");

    let result = file_fetcher_02
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());

    let result = file_fetcher_01
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_fetch_local_bypasses_file_cache() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let fixture_path = temp_dir.path().join("mod.ts");
    let specifier =
      resolve_url_or_path(&fixture_path.to_string_lossy()).unwrap();
    fs::write(fixture_path.clone(), r#"console.log("hello deno");"#).unwrap();
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(&*file.source, r#"console.log("hello deno");"#);

    fs::write(fixture_path, r#"console.log("goodbye deno");"#).unwrap();
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(&*file.source, r#"console.log("goodbye deno");"#);
  }

  #[tokio::test]
  async fn test_respect_cache_revalidates() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let specifier =
      ModuleSpecifier::parse("http://localhost:4545/dynamic").unwrap();
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let first = file.source;

    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let second = file.source;

    assert_ne!(first, second);
  }

  #[tokio::test]
  async fn test_respect_cache_still_fresh() {
    let _g = test_util::http_server();
    let temp_dir = TempDir::new();
    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let specifier =
      ModuleSpecifier::parse("http://localhost:4545/dynamic_cache").unwrap();
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let first = file.source;

    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let result = file_fetcher
      .fetch(&specifier, PermissionsContainer::allow_all())
      .await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let second = file.source;

    assert_eq!(first, second);
  }

  #[tokio::test]
  async fn test_fetch_local_utf_16be() {
    let expected = String::from_utf8(
      b"\xEF\xBB\xBFconsole.log(\"Hello World\");\x0A".to_vec(),
    )
    .unwrap();
    test_fetch_local_encoded("utf-16be", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_local_utf_16le() {
    let expected = String::from_utf8(
      b"\xEF\xBB\xBFconsole.log(\"Hello World\");\x0A".to_vec(),
    )
    .unwrap();
    test_fetch_local_encoded("utf-16le", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_local_utf8_with_bom() {
    let expected = String::from_utf8(
      b"\xEF\xBB\xBFconsole.log(\"Hello World\");\x0A".to_vec(),
    )
    .unwrap();
    test_fetch_local_encoded("utf-8", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_javascript_with_types() {
    let specifier =
      resolve_url_or_path("http://127.0.0.1:4545/xTypeScriptTypes.js").unwrap();
    let (file, _) = test_fetch_remote(&specifier).await;
    assert_eq!(
      file.maybe_types,
      Some("./xTypeScriptTypes.d.ts".to_string())
    );
  }

  #[tokio::test]
  async fn test_fetch_remote_jsx_with_types() {
    let specifier =
      resolve_url_or_path("http://127.0.0.1:4545/xTypeScriptTypes.jsx")
        .unwrap();
    let (file, _) = test_fetch_remote(&specifier).await;
    assert_eq!(file.media_type, MediaType::Jsx,);
    assert_eq!(
      file.maybe_types,
      Some("./xTypeScriptTypes.d.ts".to_string())
    );
  }

  #[tokio::test]
  async fn test_fetch_remote_typescript_with_types() {
    let specifier =
      resolve_url_or_path("http://127.0.0.1:4545/xTypeScriptTypes.ts").unwrap();
    let (file, _) = test_fetch_remote(&specifier).await;
    assert_eq!(file.maybe_types, None);
  }

  #[tokio::test]
  async fn test_fetch_remote_utf16_le() {
    let expected =
      std::str::from_utf8(b"\xEF\xBB\xBFconsole.log(\"Hello World\");\x0A")
        .unwrap();
    test_fetch_remote_encoded("utf-16le.ts", "utf-16le", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_utf16_be() {
    let expected =
      std::str::from_utf8(b"\xEF\xBB\xBFconsole.log(\"Hello World\");\x0A")
        .unwrap();
    test_fetch_remote_encoded("utf-16be.ts", "utf-16be", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_window_1255() {
    let expected = "console.log(\"\u{5E9}\u{5DC}\u{5D5}\u{5DD} \
                   \u{5E2}\u{5D5}\u{5DC}\u{5DD}\");\u{A}";
    test_fetch_remote_encoded("windows-1255", "windows-1255", expected).await;
  }

  fn create_test_client() -> HttpClient {
    HttpClient::from_client(
      create_http_client(
        "test_client".to_string(),
        None,
        vec![],
        None,
        None,
        None,
      )
      .unwrap(),
    )
  }

  #[tokio::test]
  async fn test_fetch_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/assets/fixture.json").unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(headers.get("content-type").unwrap(), "application/json");
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_gzip() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/gziped")
      .unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('gzip')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_etag() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4545/etag_script.ts").unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url: url.clone(),
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/typescript"
      );
      assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");
    } else {
      panic!();
    }

    let res = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: Some("33a64df551425fcc55e".to_string()),
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/brotli")
      .unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('brotli');");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_accept() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4545/echo_accept").unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: Some("application/json".to_string()),
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, _)) = result {
      assert_eq!(body, r#"{"accept":"application/json"}"#.as_bytes());
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_once_with_redirect() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("http://127.0.0.1:4546/assets/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url =
      Url::parse("http://localhost:4545/assets/fixture.json").unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Redirect(url, _)) = result {
      assert_eq!(url, target_url);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_string() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url = Url::parse("https://localhost:5545/assets/fixture.json").unwrap();

    let client = HttpClient::from_client(
      create_http_client(
        version::get_user_agent(),
        None,
        vec![read(
          test_util::testdata_path()
            .join("tls/RootCA.pem")
            .to_str()
            .unwrap(),
        )
        .unwrap()],
        None,
        None,
        None,
      )
      .unwrap(),
    );
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(headers.get("content-type").unwrap(), "application/json");
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_default_certificate_store() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server with a valid mozilla root CA cert.
    let url = Url::parse("https://deno.land").unwrap();
    let client = HttpClient::from_client(
      create_http_client(
        version::get_user_agent(),
        None, // This will load mozilla certs by default
        vec![],
        None,
        None,
        None,
      )
      .unwrap(),
    );

    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;

    println!("{:?}", result);
    if let Ok(FetchOnceResult::Code(body, _headers)) = result {
      assert!(!body.is_empty());
    } else {
      panic!();
    }
  }

  // TODO(@justinmchase): Windows should verify certs too and fail to make this request without ca certs
  #[cfg(not(windows))]
  #[tokio::test]
  #[ignore] // https://github.com/denoland/deno/issues/12561
  async fn test_fetch_with_empty_certificate_store() {
    use deno_runtime::deno_tls::rustls::RootCertStore;

    let _http_server_guard = test_util::http_server();
    // Relies on external http server with a valid mozilla root CA cert.
    let url = Url::parse("https://deno.land").unwrap();
    let client = HttpClient::new(
      Some(RootCertStore::empty()), // no certs loaded at all
      None,
    )
    .unwrap();

    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;

    if let Ok(FetchOnceResult::Code(_body, _headers)) = result {
      // This test is expected to fail since to CA certs have been loaded
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_gzip() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/run/import_compression/gziped")
        .unwrap();
    let client = HttpClient::from_client(
      create_http_client(
        version::get_user_agent(),
        None,
        vec![read(
          test_util::testdata_path()
            .join("tls/RootCA.pem")
            .to_str()
            .unwrap(),
        )
        .unwrap()],
        None,
        None,
        None,
      )
      .unwrap(),
    );
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('gzip')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_with_etag() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("https://localhost:5545/etag_script.ts").unwrap();
    let client = HttpClient::from_client(
      create_http_client(
        version::get_user_agent(),
        None,
        vec![read(
          test_util::testdata_path()
            .join("tls/RootCA.pem")
            .to_str()
            .unwrap(),
        )
        .unwrap()],
        None,
        None,
        None,
      )
      .unwrap(),
    );
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url: url.clone(),
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('etag')");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/typescript"
      );
      assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }

    let res = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: Some("33a64df551425fcc55e".to_string()),
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    assert_eq!(res.unwrap(), FetchOnceResult::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_with_cafile_brotli() {
    let _http_server_guard = test_util::http_server();
    // Relies on external http server. See target/debug/test_server
    let url =
      Url::parse("https://localhost:5545/run/import_compression/brotli")
        .unwrap();
    let client = HttpClient::from_client(
      create_http_client(
        version::get_user_agent(),
        None,
        vec![read(
          test_util::testdata_path()
            .join("tls/RootCA.pem")
            .to_str()
            .unwrap(),
        )
        .unwrap()],
        None,
        None,
        None,
      )
      .unwrap(),
    );
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    if let Ok(FetchOnceResult::Code(body, headers)) = result {
      assert!(!body.is_empty());
      assert_eq!(String::from_utf8(body).unwrap(), "console.log('brotli');");
      assert_eq!(
        headers.get("content-type").unwrap(),
        "application/javascript"
      );
      assert_eq!(headers.get("etag"), None);
      assert_eq!(headers.get("x-typescript-types"), None);
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn bad_redirect() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:4545/bad_redirect";
    let url = Url::parse(url_str).unwrap();
    let client = create_test_client();
    let result = fetch_once(
      &client,
      FetchOnceArgs {
        url,
        maybe_accept: None,
        maybe_etag: None,
        maybe_auth_token: None,
        maybe_progress_guard: None,
      },
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Check that the error message contains the original URL
    assert!(err.to_string().contains(url_str));
  }
}
