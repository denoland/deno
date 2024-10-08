// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CacheSetting;
use crate::auth_tokens::AuthTokens;
use crate::cache::HttpCache;
use crate::colors;
use crate::http_util::CacheSemantics;
use crate::http_util::FetchOnceArgs;
use crate::http_util::FetchOnceResult;
use crate::http_util::HttpClientProvider;
use crate::util::progress_bar::ProgressBar;

use deno_ast::MediaType;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::LoaderChecksum;

use deno_path_util::url_to_file_path;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_web::BlobStore;
use log::debug;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

pub const SUPPORTED_SCHEMES: [&str; 5] =
  ["data", "blob", "file", "http", "https"];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextDecodedFile {
  pub media_type: MediaType,
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,
  /// The source of the file.
  pub source: Arc<str>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FileOrRedirect {
  File(File),
  Redirect(ModuleSpecifier),
}

impl FileOrRedirect {
  fn from_deno_cache_entry(
    specifier: &ModuleSpecifier,
    cache_entry: deno_cache_dir::CacheEntry,
  ) -> Result<Self, AnyError> {
    if let Some(redirect_to) = cache_entry.metadata.headers.get("location") {
      let redirect =
        deno_core::resolve_import(redirect_to, specifier.as_str())?;
      Ok(FileOrRedirect::Redirect(redirect))
    } else {
      Ok(FileOrRedirect::File(File {
        specifier: specifier.clone(),
        maybe_headers: Some(cache_entry.metadata.headers),
        source: Arc::from(cache_entry.content),
      }))
    }
  }
}

/// A structure representing a source file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,
  pub maybe_headers: Option<HashMap<String, String>>,
  /// The source of the file.
  pub source: Arc<[u8]>,
}

impl File {
  pub fn resolve_media_type_and_charset(&self) -> (MediaType, Option<&str>) {
    deno_graph::source::resolve_media_type_and_charset_from_headers(
      &self.specifier,
      self.maybe_headers.as_ref(),
    )
  }

  /// Decodes the source bytes into a string handling any encoding rules
  /// for local vs remote files and dealing with the charset.
  pub fn into_text_decoded(self) -> Result<TextDecodedFile, AnyError> {
    // lots of borrow checker fighting here
    let (media_type, maybe_charset) =
      deno_graph::source::resolve_media_type_and_charset_from_headers(
        &self.specifier,
        self.maybe_headers.as_ref(),
      );
    let specifier = self.specifier;
    match deno_graph::source::decode_source(
      &specifier,
      self.source,
      maybe_charset,
    ) {
      Ok(source) => Ok(TextDecodedFile {
        media_type,
        specifier,
        source,
      }),
      Err(err) => {
        Err(err).with_context(|| format!("Failed decoding \"{}\".", specifier))
      }
    }
  }
}

#[derive(Debug, Clone, Default)]
struct MemoryFiles(Arc<Mutex<HashMap<ModuleSpecifier, File>>>);

impl MemoryFiles {
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<File> {
    self.0.lock().get(specifier).cloned()
  }

  pub fn insert(&self, specifier: ModuleSpecifier, file: File) -> Option<File> {
    self.0.lock().insert(specifier, file)
  }

  pub fn clear(&self) {
    self.0.lock().clear();
  }
}

/// Fetch a source file from the local file system.
fn fetch_local(specifier: &ModuleSpecifier) -> Result<File, AnyError> {
  let local = url_to_file_path(specifier).map_err(|_| {
    uri_error(format!("Invalid file path.\n  Specifier: {specifier}"))
  })?;
  // If it doesnt have a extension, we want to treat it as typescript by default
  let headers = if local.extension().is_none() {
    Some(HashMap::from([(
      "content-type".to_string(),
      "application/typescript".to_string(),
    )]))
  } else {
    None
  };
  let bytes = fs::read(local)?;

  Ok(File {
    specifier: specifier.clone(),
    maybe_headers: headers,
    source: bytes.into(),
  })
}

/// Return a validated scheme for a given module specifier.
fn get_validated_scheme(
  specifier: &ModuleSpecifier,
) -> Result<String, AnyError> {
  let scheme = specifier.scheme();
  if !SUPPORTED_SCHEMES.contains(&scheme) {
    Err(generic_error(format!(
      "Unsupported scheme \"{scheme}\" for module \"{specifier}\". Supported schemes: {SUPPORTED_SCHEMES:#?}"
    )))
  } else {
    Ok(scheme.to_string())
  }
}

#[derive(Debug, Copy, Clone)]
pub enum FetchPermissionsOptionRef<'a> {
  AllowAll,
  DynamicContainer(&'a PermissionsContainer),
  StaticContainer(&'a PermissionsContainer),
}

pub struct FetchOptions<'a> {
  pub specifier: &'a ModuleSpecifier,
  pub permissions: FetchPermissionsOptionRef<'a>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
}

pub struct FetchNoFollowOptions<'a> {
  pub fetch_options: FetchOptions<'a>,
  /// This setting doesn't make sense to provide for `FetchOptions`
  /// since the required checksum may change for a redirect.
  pub maybe_checksum: Option<&'a LoaderChecksum>,
}

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct FileFetcher {
  auth_tokens: AuthTokens,
  allow_remote: bool,
  memory_files: MemoryFiles,
  cache_setting: CacheSetting,
  http_cache: Arc<dyn HttpCache>,
  http_client_provider: Arc<HttpClientProvider>,
  blob_store: Arc<BlobStore>,
  download_log_level: log::Level,
  progress_bar: Option<ProgressBar>,
}

impl FileFetcher {
  pub fn new(
    http_cache: Arc<dyn HttpCache>,
    cache_setting: CacheSetting,
    allow_remote: bool,
    http_client_provider: Arc<HttpClientProvider>,
    blob_store: Arc<BlobStore>,
    progress_bar: Option<ProgressBar>,
  ) -> Self {
    Self {
      auth_tokens: AuthTokens::new(env::var("DENO_AUTH_TOKENS").ok()),
      allow_remote,
      memory_files: Default::default(),
      cache_setting,
      http_cache,
      http_client_provider,
      blob_store,
      download_log_level: log::Level::Info,
      progress_bar,
    }
  }

  pub fn cache_setting(&self) -> &CacheSetting {
    &self.cache_setting
  }

  /// Sets the log level to use when outputting the download message.
  pub fn set_download_log_level(&mut self, level: log::Level) {
    self.download_log_level = level;
  }

  /// Fetch cached remote file.
  ///
  /// This is a recursive operation if source file has redirections.
  pub fn fetch_cached(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: i64,
  ) -> Result<Option<File>, AnyError> {
    let mut specifier = Cow::Borrowed(specifier);
    for _ in 0..=redirect_limit {
      match self.fetch_cached_no_follow(&specifier, None)? {
        Some(FileOrRedirect::File(file)) => {
          return Ok(Some(file));
        }
        Some(FileOrRedirect::Redirect(redirect_specifier)) => {
          specifier = Cow::Owned(redirect_specifier);
        }
        None => {
          return Ok(None);
        }
      }
    }
    Err(custom_error("Http", "Too many redirects."))
  }

  fn fetch_cached_no_follow(
    &self,
    specifier: &ModuleSpecifier,
    maybe_checksum: Option<&LoaderChecksum>,
  ) -> Result<Option<FileOrRedirect>, AnyError> {
    debug!(
      "FileFetcher::fetch_cached_no_follow - specifier: {}",
      specifier
    );

    let cache_key = self.http_cache.cache_item_key(specifier)?; // compute this once
    let result = self.http_cache.get(
      &cache_key,
      maybe_checksum
        .as_ref()
        .map(|c| deno_cache_dir::Checksum::new(c.as_str())),
    );
    match result {
      Ok(Some(cache_data)) => Ok(Some(FileOrRedirect::from_deno_cache_entry(
        specifier, cache_data,
      )?)),
      Ok(None) => Ok(None),
      Err(err) => match err {
        deno_cache_dir::CacheReadFileError::Io(err) => Err(err.into()),
        deno_cache_dir::CacheReadFileError::ChecksumIntegrity(err) => {
          // convert to the equivalent deno_graph error so that it
          // enhances it if this is passed to deno_graph
          Err(
            deno_graph::source::ChecksumIntegrityError {
              actual: err.actual,
              expected: err.expected,
            }
            .into(),
          )
        }
      },
    }
  }

  /// Convert a data URL into a file, resulting in an error if the URL is
  /// invalid.
  fn fetch_data_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    debug!("FileFetcher::fetch_data_url() - specifier: {}", specifier);
    let data_url = deno_graph::source::RawDataUrl::parse(specifier)?;
    let (bytes, headers) = data_url.into_bytes_and_headers();
    Ok(File {
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
      source: Arc::from(bytes),
    })
  }

  /// Get a blob URL.
  async fn fetch_blob_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    debug!("FileFetcher::fetch_blob_url() - specifier: {}", specifier);
    let blob = self
      .blob_store
      .get_object_url(specifier.clone())
      .ok_or_else(|| {
        custom_error(
          "NotFound",
          format!("Blob URL not found: \"{specifier}\"."),
        )
      })?;

    let bytes = blob.read_all().await?;
    let headers =
      HashMap::from([("content-type".to_string(), blob.media_type.clone())]);

    Ok(File {
      specifier: specifier.clone(),
      maybe_headers: Some(headers),
      source: Arc::from(bytes),
    })
  }

  async fn fetch_remote_no_follow(
    &self,
    specifier: &ModuleSpecifier,
    maybe_accept: Option<&str>,
    cache_setting: &CacheSetting,
    maybe_checksum: Option<&LoaderChecksum>,
  ) -> Result<FileOrRedirect, AnyError> {
    debug!(
      "FileFetcher::fetch_remote_no_follow - specifier: {}",
      specifier
    );

    if self.should_use_cache(specifier, cache_setting) {
      if let Some(file_or_redirect) =
        self.fetch_cached_no_follow(specifier, maybe_checksum)?
      {
        return Ok(file_or_redirect);
      }
    }

    if *cache_setting == CacheSetting::Only {
      return Err(custom_error(
        "NotCached",
        format!(
          "Specifier not found in cache: \"{specifier}\", --cached-only is specified."
        ),
      ));
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

    let maybe_etag_cache_entry = self
      .http_cache
      .cache_item_key(specifier)
      .ok()
      .and_then(|key| {
        self
          .http_cache
          .get(
            &key,
            maybe_checksum
              .as_ref()
              .map(|c| deno_cache_dir::Checksum::new(c.as_str())),
          )
          .ok()
          .flatten()
      })
      .and_then(|cache_entry| {
        cache_entry
          .metadata
          .headers
          .get("etag")
          .cloned()
          .map(|etag| (cache_entry, etag))
      });
    let maybe_auth_token = self.auth_tokens.get(specifier);

    async fn handle_request_or_server_error(
      retried: &mut bool,
      specifier: &Url,
      err_str: String,
    ) -> Result<(), AnyError> {
      // Retry once, and bail otherwise.
      if !*retried {
        *retried = true;
        log::debug!("Import '{}' failed: {}. Retrying...", specifier, err_str);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(())
      } else {
        Err(generic_error(format!(
          "Import '{}' failed: {}",
          specifier, err_str
        )))
      }
    }

    let mut retried = false; // retry intermittent failures
    let result = loop {
      let result = match self
        .http_client_provider
        .get_or_create()?
        .fetch_no_follow(FetchOnceArgs {
          url: specifier.clone(),
          maybe_accept: maybe_accept.map(ToOwned::to_owned),
          maybe_etag: maybe_etag_cache_entry
            .as_ref()
            .map(|(_, etag)| etag.clone()),
          maybe_auth_token: maybe_auth_token.clone(),
          maybe_progress_guard: maybe_progress_guard.as_ref(),
        })
        .await?
      {
        FetchOnceResult::NotModified => {
          let (cache_entry, _) = maybe_etag_cache_entry.unwrap();
          FileOrRedirect::from_deno_cache_entry(specifier, cache_entry)
        }
        FetchOnceResult::Redirect(redirect_url, headers) => {
          self.http_cache.set(specifier, headers, &[])?;
          Ok(FileOrRedirect::Redirect(redirect_url))
        }
        FetchOnceResult::Code(bytes, headers) => {
          self.http_cache.set(specifier, headers.clone(), &bytes)?;
          if let Some(checksum) = &maybe_checksum {
            checksum.check_source(&bytes)?;
          }
          Ok(FileOrRedirect::File(File {
            specifier: specifier.clone(),
            maybe_headers: Some(headers),
            source: Arc::from(bytes),
          }))
        }
        FetchOnceResult::RequestError(err) => {
          handle_request_or_server_error(&mut retried, specifier, err).await?;
          continue;
        }
        FetchOnceResult::ServerError(status) => {
          handle_request_or_server_error(
            &mut retried,
            specifier,
            status.to_string(),
          )
          .await?;
          continue;
        }
      };
      break result;
    };

    drop(maybe_progress_guard);
    result
  }

  /// Returns if the cache should be used for a given specifier.
  fn should_use_cache(
    &self,
    specifier: &ModuleSpecifier,
    cache_setting: &CacheSetting,
  ) -> bool {
    match cache_setting {
      CacheSetting::ReloadAll => false,
      CacheSetting::Use | CacheSetting::Only => true,
      CacheSetting::RespectHeaders => {
        let Ok(cache_key) = self.http_cache.cache_item_key(specifier) else {
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
          CacheSemantics::new(headers, download_time, SystemTime::now());
        cache_semantics.should_use()
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

  #[inline(always)]
  pub async fn fetch_bypass_permissions(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    self
      .fetch_inner(specifier, FetchPermissionsOptionRef::AllowAll)
      .await
  }

  /// Fetch a source file and asynchronously return it.
  #[inline(always)]
  pub async fn fetch(
    &self,
    specifier: &ModuleSpecifier,
    permissions: &PermissionsContainer,
  ) -> Result<File, AnyError> {
    self
      .fetch_inner(
        specifier,
        FetchPermissionsOptionRef::StaticContainer(permissions),
      )
      .await
  }

  async fn fetch_inner(
    &self,
    specifier: &ModuleSpecifier,
    permissions: FetchPermissionsOptionRef<'_>,
  ) -> Result<File, AnyError> {
    self
      .fetch_with_options(FetchOptions {
        specifier,
        permissions,
        maybe_accept: None,
        maybe_cache_setting: None,
      })
      .await
  }

  pub async fn fetch_with_options(
    &self,
    options: FetchOptions<'_>,
  ) -> Result<File, AnyError> {
    self.fetch_with_options_and_max_redirect(options, 10).await
  }

  async fn fetch_with_options_and_max_redirect(
    &self,
    options: FetchOptions<'_>,
    max_redirect: usize,
  ) -> Result<File, AnyError> {
    let mut specifier = Cow::Borrowed(options.specifier);
    for _ in 0..=max_redirect {
      match self
        .fetch_no_follow_with_options(FetchNoFollowOptions {
          fetch_options: FetchOptions {
            specifier: &specifier,
            permissions: options.permissions,
            maybe_accept: options.maybe_accept,
            maybe_cache_setting: options.maybe_cache_setting,
          },
          maybe_checksum: None,
        })
        .await?
      {
        FileOrRedirect::File(file) => {
          return Ok(file);
        }
        FileOrRedirect::Redirect(redirect_specifier) => {
          specifier = Cow::Owned(redirect_specifier);
        }
      }
    }

    Err(custom_error("Http", "Too many redirects."))
  }

  /// Fetches without following redirects.
  pub async fn fetch_no_follow_with_options(
    &self,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, AnyError> {
    let maybe_checksum = options.maybe_checksum;
    let options = options.fetch_options;
    let specifier = options.specifier;
    // note: this debug output is used by the tests
    debug!(
      "FileFetcher::fetch_no_follow_with_options - specifier: {}",
      specifier
    );
    let scheme = get_validated_scheme(specifier)?;
    match options.permissions {
      FetchPermissionsOptionRef::AllowAll => {
        // allow
      }
      FetchPermissionsOptionRef::StaticContainer(permissions) => {
        permissions.check_specifier(
          specifier,
          deno_runtime::deno_permissions::CheckSpecifierKind::Static,
        )?;
      }
      FetchPermissionsOptionRef::DynamicContainer(permissions) => {
        permissions.check_specifier(
          specifier,
          deno_runtime::deno_permissions::CheckSpecifierKind::Dynamic,
        )?;
      }
    }
    if let Some(file) = self.memory_files.get(specifier) {
      Ok(FileOrRedirect::File(file))
    } else if scheme == "file" {
      // we do not in memory cache files, as this would prevent files on the
      // disk changing effecting things like workers and dynamic imports.
      fetch_local(specifier).map(FileOrRedirect::File)
    } else if scheme == "data" {
      self.fetch_data_url(specifier).map(FileOrRedirect::File)
    } else if scheme == "blob" {
      self
        .fetch_blob_url(specifier)
        .await
        .map(FileOrRedirect::File)
    } else if !self.allow_remote {
      Err(custom_error(
        "NoRemote",
        format!("A remote specifier was requested: \"{specifier}\", but --no-remote is specified."),
      ))
    } else {
      self
        .fetch_remote_no_follow(
          specifier,
          options.maybe_accept,
          options.maybe_cache_setting.unwrap_or(&self.cache_setting),
          maybe_checksum,
        )
        .await
    }
  }

  /// A synchronous way to retrieve a source file, where if the file has already
  /// been cached in memory it will be returned, otherwise for local files will
  /// be read from disk.
  pub fn get_source(&self, specifier: &ModuleSpecifier) -> Option<File> {
    let maybe_file = self.memory_files.get(specifier);
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

  /// Insert a temporary module for the file fetcher.
  pub fn insert_memory_files(&self, file: File) -> Option<File> {
    self.memory_files.insert(file.specifier.clone(), file)
  }

  pub fn clear_memory_files(&self) {
    self.memory_files.clear();
  }
}

#[cfg(test)]
mod tests {
  use crate::cache::GlobalHttpCache;
  use crate::cache::RealDenoCacheEnv;
  use crate::http_util::HttpClientProvider;

  use super::*;
  use deno_core::error::get_custom_error_class;
  use deno_core::resolve_url;
  use deno_runtime::deno_web::Blob;
  use deno_runtime::deno_web::InMemoryBlobPart;
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
  ) -> (FileFetcher, TempDir, Arc<BlobStore>) {
    let temp_dir = maybe_temp_dir.unwrap_or_default();
    let location = temp_dir.path().join("remote").to_path_buf();
    let blob_store: Arc<BlobStore> = Default::default();
    let file_fetcher = FileFetcher::new(
      Arc::new(GlobalHttpCache::new(location, RealDenoCacheEnv)),
      cache_setting,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      blob_store.clone(),
      None,
    );
    (file_fetcher, temp_dir, blob_store)
  }

  async fn test_fetch(specifier: &ModuleSpecifier) -> (File, FileFetcher) {
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result = file_fetcher.fetch_bypass_permissions(specifier).await;
    assert!(result.is_ok());
    (result.unwrap(), file_fetcher)
  }

  async fn test_fetch_options_remote(
    specifier: &ModuleSpecifier,
  ) -> (File, HashMap<String, String>) {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result: Result<File, AnyError> = file_fetcher
      .fetch_with_options_and_max_redirect(
        FetchOptions {
          specifier,
          permissions: FetchPermissionsOptionRef::AllowAll,
          maybe_accept: None,
          maybe_cache_setting: Some(&file_fetcher.cache_setting),
        },
        1,
      )
      .await;
    let cache_key = file_fetcher.http_cache.cache_item_key(specifier).unwrap();
    (
      result.unwrap(),
      file_fetcher
        .http_cache
        .read_headers(&cache_key)
        .unwrap()
        .unwrap(),
    )
  }

  // this test used to test how the file fetcher decoded strings, but
  // now we're using it as a bit of an integration test with the functionality
  // in deno_graph
  async fn test_fetch_remote_encoded(
    fixture: &str,
    charset: &str,
    expected: &str,
  ) {
    let url_str = format!("http://127.0.0.1:4545/encoding/{fixture}");
    let specifier = resolve_url(&url_str).unwrap();
    let (file, headers) = test_fetch_options_remote(&specifier).await;
    let (media_type, maybe_charset) =
      deno_graph::source::resolve_media_type_and_charset_from_headers(
        &specifier,
        Some(&headers),
      );
    assert_eq!(
      deno_graph::source::decode_source(&specifier, file.source, maybe_charset)
        .unwrap()
        .as_ref(),
      expected
    );
    assert_eq!(media_type, MediaType::TypeScript);
    assert_eq!(
      headers.get("content-type").unwrap(),
      &format!("application/typescript;charset={charset}")
    );
  }

  async fn test_fetch_local_encoded(charset: &str, expected: String) {
    let p = test_util::testdata_path().join(format!("encoding/{charset}.ts"));
    let specifier = ModuleSpecifier::from_file_path(p).unwrap();
    let (file, _) = test_fetch(&specifier).await;
    assert_eq!(
      deno_graph::source::decode_source(&specifier, file.source, None)
        .unwrap()
        .as_ref(),
      expected
    );
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
      let specifier = ModuleSpecifier::parse(specifier).unwrap();
      let actual = get_validated_scheme(&specifier);
      assert_eq!(actual.is_ok(), is_ok);
      if is_ok {
        assert_eq!(actual.unwrap(), expected);
      }
    }
  }

  #[tokio::test]
  async fn test_insert_cached() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let local = temp_dir.path().join("a.ts");
    let specifier = ModuleSpecifier::from_file_path(&local).unwrap();
    let file = File {
      source: Arc::from("some source code".as_bytes()),
      specifier: specifier.clone(),
      maybe_headers: Some(HashMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )])),
    };
    file_fetcher.insert_memory_files(file.clone());

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let result_file = result.unwrap();
    assert_eq!(result_file, file);
  }

  #[tokio::test]
  async fn test_fetch_data_url() {
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier = resolve_url("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=").unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(
      &*file.source,
      "export const a = \"a\";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);
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

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(
      &*file.source,
      "export const a = \"a\";\n\nexport enum A {\n  A,\n  B,\n  C,\n}\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);
    assert_eq!(file.specifier, specifier);
  }

  #[tokio::test]
  async fn test_fetch_complex() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let (file_fetcher_01, _) = setup(CacheSetting::Use, Some(temp_dir.clone()));
    let (file_fetcher_02, _) = setup(CacheSetting::Use, Some(temp_dir.clone()));
    let specifier =
      ModuleSpecifier::parse("http://localhost:4545/subdir/mod2.ts").unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);

    let cache_item_key =
      file_fetcher.http_cache.cache_item_key(&specifier).unwrap();
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "text/javascript".to_string());
    file_fetcher
      .http_cache
      .set(&specifier, headers.clone(), file.source.as_bytes())
      .unwrap();

    let result = file_fetcher_01.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    // This validates that when using the cached value, because we modified
    // the value above.
    assert_eq!(file.media_type, MediaType::JavaScript);

    let headers2 = file_fetcher_02
      .http_cache
      .read_headers(&cache_item_key)
      .unwrap()
      .unwrap();
    assert_eq!(headers2.get("content-type").unwrap(), "text/javascript");
    headers = HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    file_fetcher_02
      .http_cache
      .set(&specifier, headers.clone(), file.source.as_bytes())
      .unwrap();

    let result = file_fetcher_02.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::Json);

    // This creates a totally new instance, simulating another Deno process
    // invocation and indicates to "cache bust".
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher = FileFetcher::new(
      Arc::new(GlobalHttpCache::new(
        location,
        crate::cache::RealDenoCacheEnv,
      )),
      CacheSetting::ReloadAll,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      Default::default(),
      None,
    );
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
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
    let location = temp_dir.path().join("remote").to_path_buf();
    let specifier =
      resolve_url("http://localhost:4545/subdir/mismatch_ext.ts").unwrap();

    let file_modified_01 = {
      let file_fetcher = FileFetcher::new(
        Arc::new(GlobalHttpCache::new(
          location.clone(),
          crate::cache::RealDenoCacheEnv,
        )),
        CacheSetting::Use,
        true,
        Arc::new(HttpClientProvider::new(None, None)),
        Default::default(),
        None,
      );

      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());
      let cache_key =
        file_fetcher.http_cache.cache_item_key(&specifier).unwrap();
      (
        file_fetcher
          .http_cache
          .read_modified_time(&cache_key)
          .unwrap(),
        file_fetcher
          .http_cache
          .read_headers(&cache_key)
          .unwrap()
          .unwrap(),
        file_fetcher
          .http_cache
          .read_download_time(&cache_key)
          .unwrap()
          .unwrap(),
      )
    };

    let file_modified_02 = {
      let file_fetcher = FileFetcher::new(
        Arc::new(GlobalHttpCache::new(
          location,
          crate::cache::RealDenoCacheEnv,
        )),
        CacheSetting::Use,
        true,
        Arc::new(HttpClientProvider::new(None, None)),
        Default::default(),
        None,
      );
      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());

      let cache_key =
        file_fetcher.http_cache.cache_item_key(&specifier).unwrap();
      (
        file_fetcher
          .http_cache
          .read_modified_time(&cache_key)
          .unwrap(),
        file_fetcher
          .http_cache
          .read_headers(&cache_key)
          .unwrap()
          .unwrap(),
        file_fetcher
          .http_cache
          .read_download_time(&cache_key)
          .unwrap()
          .unwrap(),
      )
    };

    assert_eq!(file_modified_01, file_modified_02);
  }

  #[tokio::test]
  async fn test_fetch_redirected() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4546/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_specifier);

    assert_eq!(
      get_text_from_cache(&file_fetcher, &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &specifier),
      Some("http://localhost:4545/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(&file_fetcher, &redirected_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &redirected_specifier),
      None,
    );
  }

  #[tokio::test]
  async fn test_fetch_multiple_redirects() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _) = setup(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4548/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_01_specifier =
      resolve_url("http://localhost:4546/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_02_specifier =
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_02_specifier);

    assert_eq!(
      get_text_from_cache(&file_fetcher, &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &specifier),
      Some("http://localhost:4546/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(&file_fetcher, &redirected_01_specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &redirected_01_specifier),
      Some("http://localhost:4545/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(&file_fetcher, &redirected_02_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &redirected_02_specifier),
      None,
    );
  }

  #[tokio::test]
  async fn test_fetch_uses_cache_with_redirects() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("remote").to_path_buf();
    let specifier =
      resolve_url("http://localhost:4548/subdir/mismatch_ext.ts").unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4546/subdir/mismatch_ext.ts").unwrap();

    let metadata_file_modified_01 = {
      let file_fetcher = FileFetcher::new(
        Arc::new(GlobalHttpCache::new(
          location.clone(),
          crate::cache::RealDenoCacheEnv,
        )),
        CacheSetting::Use,
        true,
        Arc::new(HttpClientProvider::new(None, None)),
        Default::default(),
        None,
      );

      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());

      let cache_key = file_fetcher
        .http_cache
        .cache_item_key(&redirected_specifier)
        .unwrap();
      (
        file_fetcher
          .http_cache
          .read_modified_time(&cache_key)
          .unwrap(),
        file_fetcher
          .http_cache
          .read_headers(&cache_key)
          .unwrap()
          .unwrap(),
        file_fetcher
          .http_cache
          .read_download_time(&cache_key)
          .unwrap()
          .unwrap(),
      )
    };

    let metadata_file_modified_02 = {
      let file_fetcher = FileFetcher::new(
        Arc::new(GlobalHttpCache::new(
          location,
          crate::cache::RealDenoCacheEnv,
        )),
        CacheSetting::Use,
        true,
        Arc::new(HttpClientProvider::new(None, None)),
        Default::default(),
        None,
      );
      let result = file_fetcher
        .fetch_bypass_permissions(&redirected_specifier)
        .await;
      assert!(result.is_ok());

      let cache_key = file_fetcher
        .http_cache
        .cache_item_key(&redirected_specifier)
        .unwrap();
      (
        file_fetcher
          .http_cache
          .read_modified_time(&cache_key)
          .unwrap(),
        file_fetcher
          .http_cache
          .read_headers(&cache_key)
          .unwrap()
          .unwrap(),
        file_fetcher
          .http_cache
          .read_download_time(&cache_key)
          .unwrap()
          .unwrap(),
      )
    };

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
      .fetch_with_options_and_max_redirect(
        FetchOptions {
          specifier: &specifier,
          permissions: FetchPermissionsOptionRef::AllowAll,
          maybe_accept: None,
          maybe_cache_setting: Some(&file_fetcher.cache_setting),
        },
        2,
      )
      .await;
    assert!(result.is_ok());

    let result = file_fetcher
      .fetch_with_options_and_max_redirect(
        FetchOptions {
          specifier: &specifier,
          permissions: FetchPermissionsOptionRef::AllowAll,
          maybe_accept: None,
          maybe_cache_setting: Some(&file_fetcher.cache_setting),
        },
        1,
      )
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
    let redirected_specifier =
      resolve_url("http://localhost:4550/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.specifier, redirected_specifier);

    assert_eq!(
      get_text_from_cache(&file_fetcher, &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &specifier),
      Some("/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(&file_fetcher, &redirected_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(&file_fetcher, &redirected_specifier),
      None
    );
  }

  #[tokio::test]
  async fn test_fetch_no_remote() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher = FileFetcher::new(
      Arc::new(GlobalHttpCache::new(
        location,
        crate::cache::RealDenoCacheEnv,
      )),
      CacheSetting::Use,
      false,
      Arc::new(HttpClientProvider::new(None, None)),
      Default::default(),
      None,
    );
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(get_custom_error_class(&err), Some("NoRemote"));
    assert_eq!(err.to_string(), "A remote specifier was requested: \"http://localhost:4545/run/002_hello.ts\", but --no-remote is specified.");
  }

  #[tokio::test]
  async fn test_fetch_cache_only() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher_01 = FileFetcher::new(
      Arc::new(GlobalHttpCache::new(location.clone(), RealDenoCacheEnv)),
      CacheSetting::Only,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      Default::default(),
      None,
    );
    let file_fetcher_02 = FileFetcher::new(
      Arc::new(GlobalHttpCache::new(location, RealDenoCacheEnv)),
      CacheSetting::Use,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      Default::default(),
      None,
    );
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher_01.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_string(), "Specifier not found in cache: \"http://localhost:4545/run/002_hello.ts\", --cached-only is specified.");
    assert_eq!(get_custom_error_class(&err), Some("NotCached"));

    let result = file_fetcher_02.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());

    let result = file_fetcher_01.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_fetch_local_bypasses_file_cache() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let fixture_path = temp_dir.path().join("mod.ts");
    let specifier = ModuleSpecifier::from_file_path(&fixture_path).unwrap();
    fs::write(fixture_path.clone(), r#"console.log("hello deno");"#).unwrap();
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
    assert_eq!(&*file.source, r#"console.log("hello deno");"#);

    fs::write(fixture_path, r#"console.log("goodbye deno");"#).unwrap();
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap().into_text_decoded().unwrap();
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
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let first = file.source;

    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
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
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let first = file.source;

    let (file_fetcher, _) =
      setup(CacheSetting::RespectHeaders, Some(temp_dir.clone()));
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    let second = file.source;

    assert_eq!(first, second);
  }

  #[tokio::test]
  async fn test_fetch_local_utf_16be() {
    let expected =
      String::from_utf8(b"console.log(\"Hello World\");\x0A".to_vec()).unwrap();
    test_fetch_local_encoded("utf-16be", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_local_utf_16le() {
    let expected =
      String::from_utf8(b"console.log(\"Hello World\");\x0A".to_vec()).unwrap();
    test_fetch_local_encoded("utf-16le", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_local_utf8_with_bom() {
    let expected =
      String::from_utf8(b"console.log(\"Hello World\");\x0A".to_vec()).unwrap();
    test_fetch_local_encoded("utf-8", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_utf16_le() {
    let expected =
      std::str::from_utf8(b"console.log(\"Hello World\");\x0A").unwrap();
    test_fetch_remote_encoded("utf-16le.ts", "utf-16le", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_utf16_be() {
    let expected =
      std::str::from_utf8(b"console.log(\"Hello World\");\x0A").unwrap();
    test_fetch_remote_encoded("utf-16be.ts", "utf-16be", expected).await;
  }

  #[tokio::test]
  async fn test_fetch_remote_window_1255() {
    let expected = "console.log(\"\u{5E9}\u{5DC}\u{5D5}\u{5DD} \
                   \u{5E2}\u{5D5}\u{5DC}\u{5DD}\");\u{A}";
    test_fetch_remote_encoded("windows-1255", "windows-1255", expected).await;
  }

  #[track_caller]
  fn get_text_from_cache(
    file_fetcher: &FileFetcher,
    url: &ModuleSpecifier,
  ) -> String {
    let cache_key = file_fetcher.http_cache.cache_item_key(url).unwrap();
    let bytes = file_fetcher
      .http_cache
      .get(&cache_key, None)
      .unwrap()
      .unwrap()
      .content;
    String::from_utf8(bytes).unwrap()
  }

  #[track_caller]
  fn get_location_header_from_cache(
    file_fetcher: &FileFetcher,
    url: &ModuleSpecifier,
  ) -> Option<String> {
    let cache_key = file_fetcher.http_cache.cache_item_key(url).unwrap();
    file_fetcher
      .http_cache
      .read_headers(&cache_key)
      .unwrap()
      .unwrap()
      .remove("location")
  }
}
