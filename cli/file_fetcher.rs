// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use boxed_error::Boxed;
use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::AuthTokens;
use deno_cache_dir::file_fetcher::BlobData;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::FetchNoFollowError;
use deno_cache_dir::file_fetcher::File;
use deno_cache_dir::file_fetcher::FileFetcherOptions;
use deno_cache_dir::file_fetcher::FileOrRedirect;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_cache_dir::file_fetcher::TooManyRedirectsError;
use deno_cache_dir::file_fetcher::UnsupportedSchemeError;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_error::JsError;
use deno_graph::source::LoaderChecksum;
use deno_runtime::deno_permissions::CheckSpecifierKind;
use deno_runtime::deno_permissions::PermissionCheckError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_web::BlobStore;
use http::header;
use http::HeaderMap;
use http::StatusCode;
use thiserror::Error;

use crate::cache::HttpCache;
use crate::colors;
use crate::http_util::get_response_body_with_progress;
use crate::http_util::HttpClientProvider;
use crate::sys::CliSys;
use crate::util::progress_bar::ProgressBar;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextDecodedFile {
  pub media_type: MediaType,
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,
  /// The source of the file.
  pub source: Arc<str>,
}

impl TextDecodedFile {
  /// Decodes the source bytes into a string handling any encoding rules
  /// for local vs remote files and dealing with the charset.
  pub fn decode(file: File) -> Result<Self, AnyError> {
    let (media_type, maybe_charset) =
      deno_graph::source::resolve_media_type_and_charset_from_headers(
        &file.url,
        file.maybe_headers.as_ref(),
      );
    let specifier = file.url;
    let charset = maybe_charset.unwrap_or_else(|| {
      deno_media_type::encoding::detect_charset(&specifier, &file.source)
    });
    match deno_media_type::encoding::decode_arc_source(charset, file.source) {
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

#[derive(Debug)]
struct BlobStoreAdapter(Arc<BlobStore>);

#[async_trait::async_trait(?Send)]
impl deno_cache_dir::file_fetcher::BlobStore for BlobStoreAdapter {
  async fn get(&self, specifier: &Url) -> std::io::Result<Option<BlobData>> {
    let Some(blob) = self.0.get_object_url(specifier.clone()) else {
      return Ok(None);
    };
    Ok(Some(BlobData {
      media_type: blob.media_type.clone(),
      bytes: blob.read_all().await,
    }))
  }
}

#[derive(Debug)]
struct HttpClientAdapter {
  http_client_provider: Arc<HttpClientProvider>,
  download_log_level: log::Level,
  progress_bar: Option<ProgressBar>,
}

#[async_trait::async_trait(?Send)]
impl deno_cache_dir::file_fetcher::HttpClient for HttpClientAdapter {
  async fn send_no_follow(
    &self,
    url: &Url,
    headers: HeaderMap,
  ) -> Result<SendResponse, SendError> {
    async fn handle_request_or_server_error(
      retried: &mut bool,
      specifier: &Url,
      err_str: String,
    ) -> Result<(), ()> {
      // Retry once, and bail otherwise.
      if !*retried {
        *retried = true;
        log::debug!("Import '{}' failed: {}. Retrying...", specifier, err_str);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(())
      } else {
        Err(())
      }
    }

    let mut maybe_progress_guard = None;
    if let Some(pb) = self.progress_bar.as_ref() {
      maybe_progress_guard = Some(pb.update(url.as_str()));
    } else {
      log::log!(
        self.download_log_level,
        "{} {}",
        colors::green("Download"),
        url
      );
    }

    let mut retried = false; // retry intermittent failures
    loop {
      let response = match self
        .http_client_provider
        .get_or_create()
        .map_err(|err| SendError::Failed(err.into()))?
        .send(url, headers.clone())
        .await
      {
        Ok(response) => response,
        Err(crate::http_util::SendError::Send(err)) => {
          if err.is_connect_error() {
            handle_request_or_server_error(&mut retried, url, err.to_string())
              .await
              .map_err(|()| SendError::Failed(err.into()))?;
            continue;
          } else {
            return Err(SendError::Failed(err.into()));
          }
        }
        Err(crate::http_util::SendError::InvalidUri(err)) => {
          return Err(SendError::Failed(err.into()));
        }
      };
      if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(SendResponse::NotModified);
      }

      if let Some(warning) = response.headers().get("X-Deno-Warning") {
        log::warn!(
          "{} {}",
          crate::colors::yellow("Warning"),
          warning.to_str().unwrap()
        );
      }

      if response.status().is_redirection() {
        return Ok(SendResponse::Redirect(response.into_parts().0.headers));
      }

      if response.status().is_server_error() {
        handle_request_or_server_error(
          &mut retried,
          url,
          response.status().to_string(),
        )
        .await
        .map_err(|()| SendError::StatusCode(response.status()))?;
      } else if response.status().is_client_error() {
        let err = if response.status() == StatusCode::NOT_FOUND {
          SendError::NotFound
        } else {
          SendError::StatusCode(response.status())
        };
        return Err(err);
      } else {
        let body_result = get_response_body_with_progress(
          response,
          maybe_progress_guard.as_ref(),
        )
        .await;

        match body_result {
          Ok((headers, body)) => {
            return Ok(SendResponse::Success(headers, body));
          }
          Err(err) => {
            handle_request_or_server_error(&mut retried, url, err.to_string())
              .await
              .map_err(|()| SendError::Failed(err.into()))?;
            continue;
          }
        }
      }
    }
  }
}

#[derive(Debug, Default)]
struct MemoryFiles(Mutex<HashMap<ModuleSpecifier, File>>);

impl MemoryFiles {
  pub fn insert(&self, specifier: ModuleSpecifier, file: File) -> Option<File> {
    self.0.lock().insert(specifier, file)
  }

  pub fn clear(&self) {
    self.0.lock().clear();
  }
}

impl deno_cache_dir::file_fetcher::MemoryFiles for MemoryFiles {
  fn get(&self, specifier: &ModuleSpecifier) -> Option<File> {
    self.0.lock().get(specifier).cloned()
  }
}

#[derive(Debug, Boxed, JsError)]
pub struct CliFetchNoFollowError(pub Box<CliFetchNoFollowErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum CliFetchNoFollowErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  FetchNoFollow(#[from] FetchNoFollowError),
  #[error(transparent)]
  #[class(generic)]
  PermissionCheck(#[from] PermissionCheckError),
}

#[derive(Debug, Copy, Clone)]
pub enum FetchPermissionsOptionRef<'a> {
  AllowAll,
  Restricted(&'a PermissionsContainer, CheckSpecifierKind),
}

#[derive(Debug, Default)]
pub struct FetchOptions<'a> {
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
}

pub struct FetchNoFollowOptions<'a> {
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
  pub maybe_checksum: Option<&'a LoaderChecksum>,
}

type DenoCacheDirFileFetcher = deno_cache_dir::file_fetcher::FileFetcher<
  BlobStoreAdapter,
  CliSys,
  HttpClientAdapter,
>;

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct CliFileFetcher {
  file_fetcher: DenoCacheDirFileFetcher,
  memory_files: Arc<MemoryFiles>,
}

impl CliFileFetcher {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    http_cache: Arc<dyn HttpCache>,
    http_client_provider: Arc<HttpClientProvider>,
    sys: CliSys,
    blob_store: Arc<BlobStore>,
    progress_bar: Option<ProgressBar>,
    allow_remote: bool,
    cache_setting: CacheSetting,
    download_log_level: log::Level,
  ) -> Self {
    let memory_files = Arc::new(MemoryFiles::default());
    let auth_tokens = AuthTokens::new_from_sys(&sys);
    let file_fetcher = DenoCacheDirFileFetcher::new(
      BlobStoreAdapter(blob_store),
      sys,
      http_cache,
      HttpClientAdapter {
        http_client_provider: http_client_provider.clone(),
        download_log_level,
        progress_bar,
      },
      memory_files.clone(),
      FileFetcherOptions {
        allow_remote,
        cache_setting,
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
    specifier: &ModuleSpecifier,
  ) -> Result<File, AnyError> {
    self
      .fetch_inner(specifier, None, FetchPermissionsOptionRef::AllowAll)
      .await
  }

  #[inline(always)]
  pub async fn fetch_bypass_permissions_with_maybe_auth(
    &self,
    specifier: &ModuleSpecifier,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  ) -> Result<File, AnyError> {
    self
      .fetch_inner(specifier, maybe_auth, FetchPermissionsOptionRef::AllowAll)
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
    specifier: &ModuleSpecifier,
    maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
    permissions: FetchPermissionsOptionRef<'_>,
  ) -> Result<File, AnyError> {
    self
      .fetch_with_options(
        specifier,
        permissions,
        FetchOptions {
          maybe_auth,
          maybe_accept: None,
          maybe_cache_setting: None,
        },
      )
      .await
  }

  pub async fn fetch_with_options(
    &self,
    specifier: &ModuleSpecifier,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchOptions<'_>,
  ) -> Result<File, AnyError> {
    self
      .fetch_with_options_and_max_redirect(specifier, permissions, options, 10)
      .await
  }

  async fn fetch_with_options_and_max_redirect(
    &self,
    specifier: &ModuleSpecifier,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchOptions<'_>,
    max_redirect: usize,
  ) -> Result<File, AnyError> {
    let mut specifier = Cow::Borrowed(specifier);
    let mut maybe_auth = options.maybe_auth;
    for _ in 0..=max_redirect {
      match self
        .fetch_no_follow(
          &specifier,
          permissions,
          FetchNoFollowOptions {
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
    specifier: &ModuleSpecifier,
    permissions: FetchPermissionsOptionRef<'_>,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, CliFetchNoFollowError> {
    validate_scheme(specifier).map_err(|err| {
      CliFetchNoFollowErrorKind::FetchNoFollow(err.into()).into_box()
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
          maybe_auth: options.maybe_auth,
          maybe_checksum: options
            .maybe_checksum
            .map(|c| deno_cache_dir::Checksum::new(c.as_str())),
          maybe_accept: options.maybe_accept,
          maybe_cache_setting: options.maybe_cache_setting,
        },
      )
      .await
      .map_err(|err| CliFetchNoFollowErrorKind::FetchNoFollow(err).into_box())
  }

  /// A synchronous way to retrieve a source file, where if the file has already
  /// been cached in memory it will be returned, otherwise for local files will
  /// be read from disk.
  pub fn get_cached_source_or_local(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<File>, AnyError> {
    if specifier.scheme() == "file" {
      Ok(self.file_fetcher.fetch_local(specifier)?)
    } else {
      Ok(self.file_fetcher.fetch_cached(specifier, 10)?)
    }
  }

  /// Insert a temporary module for the file fetcher.
  pub fn insert_memory_files(&self, file: File) -> Option<File> {
    self.memory_files.insert(file.url.clone(), file)
  }

  pub fn clear_memory_files(&self) {
    self.memory_files.clear();
  }
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

#[cfg(test)]
mod tests {
  use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind;
  use deno_cache_dir::file_fetcher::HttpClient;
  use deno_core::resolve_url;
  use deno_runtime::deno_web::Blob;
  use deno_runtime::deno_web::InMemoryBlobPart;
  use test_util::TempDir;

  use super::*;
  use crate::cache::GlobalHttpCache;
  use crate::http_util::HttpClientProvider;

  fn setup(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (CliFileFetcher, TempDir) {
    let (file_fetcher, temp_dir, _) =
      setup_with_blob_store(cache_setting, maybe_temp_dir);
    (file_fetcher, temp_dir)
  }

  fn setup_with_blob_store(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (CliFileFetcher, TempDir, Arc<BlobStore>) {
    let (file_fetcher, temp_dir, blob_store, _) =
      setup_with_blob_store_and_cache(cache_setting, maybe_temp_dir);
    (file_fetcher, temp_dir, blob_store)
  }

  fn setup_with_blob_store_and_cache(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (
    CliFileFetcher,
    TempDir,
    Arc<BlobStore>,
    Arc<GlobalHttpCache>,
  ) {
    let temp_dir = maybe_temp_dir.unwrap_or_default();
    let location = temp_dir.path().join("remote").to_path_buf();
    let blob_store: Arc<BlobStore> = Default::default();
    let cache = Arc::new(GlobalHttpCache::new(CliSys::default(), location));
    let file_fetcher = CliFileFetcher::new(
      cache.clone(),
      Arc::new(HttpClientProvider::new(None, None)),
      CliSys::default(),
      blob_store.clone(),
      None,
      true,
      cache_setting,
      log::Level::Info,
    );
    (file_fetcher, temp_dir, blob_store, cache)
  }

  async fn test_fetch(specifier: &ModuleSpecifier) -> (File, CliFileFetcher) {
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result = file_fetcher.fetch_bypass_permissions(specifier).await;
    assert!(result.is_ok());
    (result.unwrap(), file_fetcher)
  }

  async fn test_fetch_options_remote(
    specifier: &ModuleSpecifier,
  ) -> (File, HashMap<String, String>) {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _, _, http_cache) =
      setup_with_blob_store_and_cache(CacheSetting::ReloadAll, None);
    let result: Result<File, AnyError> = file_fetcher
      .fetch_with_options_and_max_redirect(
        specifier,
        FetchPermissionsOptionRef::AllowAll,
        Default::default(),
        1,
      )
      .await;
    let cache_key = http_cache.cache_item_key(specifier).unwrap();
    (
      result.unwrap(),
      http_cache.read_headers(&cache_key).unwrap().unwrap(),
    )
  }

  // this test used to test how the file fetcher decoded strings, but
  // now we're using it as a bit of an integration test with the functionality
  // in deno_graph
  async fn test_fetch_remote_encoded(
    fixture: &str,
    expected_charset: &str,
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
      deno_media_type::encoding::decode_arc_source(
        maybe_charset.unwrap_or_else(|| {
          deno_media_type::encoding::detect_charset(&specifier, &file.source)
        }),
        file.source
      )
      .unwrap()
      .as_ref(),
      expected
    );
    assert_eq!(media_type, MediaType::TypeScript);
    assert_eq!(
      headers.get("content-type").unwrap(),
      &format!("application/typescript;charset={expected_charset}")
    );
  }

  async fn test_fetch_local_encoded(charset: &str, expected: String) {
    let p = test_util::testdata_path().join(format!("encoding/{charset}.ts"));
    let specifier = ModuleSpecifier::from_file_path(p).unwrap();
    let (file, _) = test_fetch(&specifier).await;
    assert_eq!(
      deno_media_type::encoding::decode_arc_source(
        deno_media_type::encoding::detect_charset(&specifier, &file.source),
        file.source
      )
      .unwrap()
      .as_ref(),
      expected
    );
  }

  #[tokio::test]
  async fn test_insert_cached() {
    let (file_fetcher, temp_dir) = setup(CacheSetting::Use, None);
    let local = temp_dir.path().join("a.ts");
    let specifier = ModuleSpecifier::from_file_path(&local).unwrap();
    let file = File {
      source: Arc::from("some source code".as_bytes()),
      url: specifier.clone(),
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
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
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
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
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
    let (file_fetcher, temp_dir, _, http_cache) =
      setup_with_blob_store_and_cache(CacheSetting::Use, None);
    let (file_fetcher_01, _) = setup(CacheSetting::Use, Some(temp_dir.clone()));
    let (file_fetcher_02, _, _, http_cache_02) =
      setup_with_blob_store_and_cache(
        CacheSetting::Use,
        Some(temp_dir.clone()),
      );
    let specifier =
      ModuleSpecifier::parse("http://localhost:4545/subdir/mod2.ts").unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::TypeScript);

    let cache_item_key = http_cache.cache_item_key(&specifier).unwrap();
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "text/javascript".to_string());
    http_cache
      .set(&specifier, headers.clone(), file.source.as_bytes())
      .unwrap();

    let result = file_fetcher_01.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    // This validates that when using the cached value, because we modified
    // the value above.
    assert_eq!(file.media_type, MediaType::JavaScript);

    let headers2 = http_cache_02
      .read_headers(&cache_item_key)
      .unwrap()
      .unwrap();
    assert_eq!(headers2.get("content-type").unwrap(), "text/javascript");
    headers = HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    http_cache_02
      .set(&specifier, headers.clone(), file.source.as_bytes())
      .unwrap();

    let result = file_fetcher_02.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
    assert_eq!(
      &*file.source,
      "export { printHello } from \"./print_hello.ts\";\n"
    );
    assert_eq!(file.media_type, MediaType::Json);

    // This creates a totally new instance, simulating another Deno process
    // invocation and indicates to "cache bust".
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(CliSys::default(), location)),
      Arc::new(HttpClientProvider::new(None, None)),
      CliSys::default(),
      Default::default(),
      None,
      true,
      CacheSetting::ReloadAll,
      log::Level::Info,
    );
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
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

    let http_cache =
      Arc::new(GlobalHttpCache::new(CliSys::default(), location.clone()));
    let file_modified_01 = {
      let file_fetcher = CliFileFetcher::new(
        http_cache.clone(),
        Arc::new(HttpClientProvider::new(None, None)),
        CliSys::default(),
        Default::default(),
        None,
        true,
        CacheSetting::Use,
        log::Level::Info,
      );

      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());
      let cache_key = http_cache.cache_item_key(&specifier).unwrap();
      (
        http_cache.read_modified_time(&cache_key).unwrap(),
        http_cache.read_headers(&cache_key).unwrap().unwrap(),
        http_cache.read_download_time(&cache_key).unwrap().unwrap(),
      )
    };

    let file_modified_02 = {
      let file_fetcher = CliFileFetcher::new(
        Arc::new(GlobalHttpCache::new(CliSys::default(), location)),
        Arc::new(HttpClientProvider::new(None, None)),
        CliSys::default(),
        Default::default(),
        None,
        true,
        CacheSetting::Use,
        log::Level::Info,
      );
      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());

      let cache_key = http_cache.cache_item_key(&specifier).unwrap();
      (
        http_cache.read_modified_time(&cache_key).unwrap(),
        http_cache.read_headers(&cache_key).unwrap().unwrap(),
        http_cache.read_download_time(&cache_key).unwrap().unwrap(),
      )
    };

    assert_eq!(file_modified_01, file_modified_02);
  }

  #[tokio::test]
  async fn test_fetch_redirected() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _, _, http_cache) =
      setup_with_blob_store_and_cache(CacheSetting::Use, None);
    let specifier =
      resolve_url("http://localhost:4546/subdir/redirects/redirect1.js")
        .unwrap();
    let redirected_specifier =
      resolve_url("http://localhost:4545/subdir/redirects/redirect1.js")
        .unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.url, redirected_specifier);

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(http_cache.as_ref(), &specifier),
      Some("http://localhost:4545/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &redirected_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(
        http_cache.as_ref(),
        &redirected_specifier
      ),
      None,
    );
  }

  #[tokio::test]
  async fn test_fetch_multiple_redirects() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _, _, http_cache) =
      setup_with_blob_store_and_cache(CacheSetting::Use, None);
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
    assert_eq!(file.url, redirected_02_specifier);

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(http_cache.as_ref(), &specifier),
      Some("http://localhost:4546/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &redirected_01_specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(
        http_cache.as_ref(),
        &redirected_01_specifier
      ),
      Some("http://localhost:4545/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &redirected_02_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(
        http_cache.as_ref(),
        &redirected_02_specifier
      ),
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
    let http_cache =
      Arc::new(GlobalHttpCache::new(CliSys::default(), location.clone()));

    let metadata_file_modified_01 = {
      let file_fetcher = CliFileFetcher::new(
        http_cache.clone(),
        Arc::new(HttpClientProvider::new(None, None)),
        CliSys::default(),
        Default::default(),
        None,
        true,
        CacheSetting::Use,
        log::Level::Info,
      );

      let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
      assert!(result.is_ok());

      let cache_key = http_cache.cache_item_key(&redirected_specifier).unwrap();
      (
        http_cache.read_modified_time(&cache_key).unwrap(),
        http_cache.read_headers(&cache_key).unwrap().unwrap(),
        http_cache.read_download_time(&cache_key).unwrap().unwrap(),
      )
    };

    let metadata_file_modified_02 = {
      let file_fetcher = CliFileFetcher::new(
        http_cache.clone(),
        Arc::new(HttpClientProvider::new(None, None)),
        CliSys::default(),
        Default::default(),
        None,
        true,
        CacheSetting::Use,
        log::Level::Info,
      );
      let result = file_fetcher
        .fetch_bypass_permissions(&redirected_specifier)
        .await;
      assert!(result.is_ok());

      let cache_key = http_cache.cache_item_key(&redirected_specifier).unwrap();
      (
        http_cache.read_modified_time(&cache_key).unwrap(),
        http_cache.read_headers(&cache_key).unwrap().unwrap(),
        http_cache.read_download_time(&cache_key).unwrap().unwrap(),
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
        &specifier,
        FetchPermissionsOptionRef::AllowAll,
        Default::default(),
        2,
      )
      .await;
    assert!(result.is_ok());

    let result = file_fetcher
      .fetch_with_options_and_max_redirect(
        &specifier,
        FetchPermissionsOptionRef::AllowAll,
        Default::default(),
        1,
      )
      .await;
    assert!(result.is_err());

    let result = file_fetcher.file_fetcher.fetch_cached(&specifier, 2);
    assert!(result.is_ok());

    let result = file_fetcher.file_fetcher.fetch_cached(&specifier, 1);
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_fetch_same_host_redirect() {
    let _http_server_guard = test_util::http_server();
    let (file_fetcher, _, _, http_cache) =
      setup_with_blob_store_and_cache(CacheSetting::Use, None);
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
    assert_eq!(file.url, redirected_specifier);

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &specifier),
      "",
      "redirected files should have empty cached contents"
    );
    assert_eq!(
      get_location_header_from_cache(http_cache.as_ref(), &specifier),
      Some("/subdir/redirects/redirect1.js".to_string()),
    );

    assert_eq!(
      get_text_from_cache(http_cache.as_ref(), &redirected_specifier),
      "export const redirect = 1;\n"
    );
    assert_eq!(
      get_location_header_from_cache(
        http_cache.as_ref(),
        &redirected_specifier
      ),
      None
    );
  }

  #[tokio::test]
  async fn test_fetch_no_remote() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(CliSys::default(), location)),
      Arc::new(HttpClientProvider::new(None, None)),
      CliSys::default(),
      Default::default(),
      None,
      false,
      CacheSetting::Use,
      log::Level::Info,
    );
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.downcast::<CliFetchNoFollowError>().unwrap().into_kind();
    match err {
      CliFetchNoFollowErrorKind::FetchNoFollow(err) => {
        let err = err.into_kind();
        match &err {
          FetchNoFollowErrorKind::NoRemote { .. } => {
            assert_eq!(err.to_string(), "A remote specifier was requested: \"http://localhost:4545/run/002_hello.ts\", but --no-remote is specified.");
          }
          _ => unreachable!(),
        }
      }
      _ => unreachable!(),
    }
  }

  #[tokio::test]
  async fn test_fetch_cache_only() {
    let _http_server_guard = test_util::http_server();
    let temp_dir = TempDir::new();
    let location = temp_dir.path().join("remote").to_path_buf();
    let file_fetcher_01 = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(CliSys::default(), location.clone())),
      Arc::new(HttpClientProvider::new(None, None)),
      CliSys::default(),
      Default::default(),
      None,
      true,
      CacheSetting::Only,
      log::Level::Info,
    );
    let file_fetcher_02 = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(CliSys::default(), location)),
      Arc::new(HttpClientProvider::new(None, None)),
      CliSys::default(),
      Default::default(),
      None,
      true,
      CacheSetting::Use,
      log::Level::Info,
    );
    let specifier =
      resolve_url("http://localhost:4545/run/002_hello.ts").unwrap();

    let result = file_fetcher_01.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.downcast::<CliFetchNoFollowError>().unwrap().into_kind();
    match err {
      CliFetchNoFollowErrorKind::FetchNoFollow(err) => {
        let err = err.into_kind();
        match &err {
          FetchNoFollowErrorKind::NotCached { .. } => {
            assert_eq!(err.to_string(), "Specifier not found in cache: \"http://localhost:4545/run/002_hello.ts\", --cached-only is specified.");
          }
          _ => unreachable!(),
        }
      }
      _ => unreachable!(),
    }

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
    fixture_path.write(r#"console.log("hello deno");"#);
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
    assert_eq!(&*file.source, r#"console.log("hello deno");"#);

    fixture_path.write(r#"console.log("goodbye deno");"#);
    let result = file_fetcher.fetch_bypass_permissions(&specifier).await;
    assert!(result.is_ok());
    let file = TextDecodedFile::decode(result.unwrap()).unwrap();
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

  fn create_http_client_adapter() -> HttpClientAdapter {
    HttpClientAdapter {
      http_client_provider: Arc::new(HttpClientProvider::new(None, None)),
      download_log_level: log::Level::Info,
      progress_bar: None,
    }
  }

  #[tokio::test]
  async fn test_fetch_string() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4545/assets/fixture.json").unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, HeaderMap::new()).await;
    if let Ok(SendResponse::Success(headers, body)) = result {
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
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/gziped")
      .unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, HeaderMap::new()).await;
    if let Ok(SendResponse::Success(headers, body)) = result {
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
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, HeaderMap::new()).await;
    if let Ok(SendResponse::Success(headers, body)) = result {
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

    let mut headers = HeaderMap::new();
    headers.insert("if-none-match", "33a64df551425fcc55e".parse().unwrap());
    let res = client.send_no_follow(&url, headers).await;
    assert_eq!(res.unwrap(), SendResponse::NotModified);
  }

  #[tokio::test]
  async fn test_fetch_brotli() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4545/run/import_compression/brotli")
      .unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, HeaderMap::new()).await;
    if let Ok(SendResponse::Success(headers, body)) = result {
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
    let url = Url::parse("http://127.0.0.1:4545/echo_accept").unwrap();
    let client = create_http_client_adapter();
    let mut headers = HeaderMap::new();
    headers.insert("accept", "application/json".parse().unwrap());
    let result = client.send_no_follow(&url, headers).await;
    if let Ok(SendResponse::Success(_, body)) = result {
      assert_eq!(body, r#"{"accept":"application/json"}"#.as_bytes());
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn test_fetch_no_follow_with_redirect() {
    let _http_server_guard = test_util::http_server();
    let url = Url::parse("http://127.0.0.1:4546/assets/fixture.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url =
      Url::parse("http://localhost:4545/assets/fixture.json").unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, Default::default()).await;
    if let Ok(SendResponse::Redirect(headers)) = result {
      assert_eq!(headers.get("location").unwrap(), target_url.as_str());
    } else {
      panic!();
    }
  }

  #[tokio::test]
  async fn server_error() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:4545/server_error";
    let url = Url::parse(url_str).unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, Default::default()).await;

    if let Err(SendError::StatusCode(status)) = result {
      assert_eq!(status, 500);
    } else {
      panic!("{:?}", result);
    }
  }

  #[tokio::test]
  async fn request_error() {
    let _g = test_util::http_server();
    let url_str = "http://127.0.0.1:9999/";
    let url = Url::parse(url_str).unwrap();
    let client = create_http_client_adapter();
    let result = client.send_no_follow(&url, Default::default()).await;

    assert!(matches!(result, Err(SendError::Failed(_))));
  }

  #[track_caller]
  fn get_text_from_cache(
    http_cache: &dyn HttpCache,
    url: &ModuleSpecifier,
  ) -> String {
    let cache_key = http_cache.cache_item_key(url).unwrap();
    let bytes = http_cache.get(&cache_key, None).unwrap().unwrap().content;
    String::from_utf8(bytes.into_owned()).unwrap()
  }

  #[track_caller]
  fn get_location_header_from_cache(
    http_cache: &dyn HttpCache,
    url: &ModuleSpecifier,
  ) -> Option<String> {
    let cache_key = http_cache.cache_item_key(url).unwrap();
    http_cache
      .read_headers(&cache_key)
      .unwrap()
      .unwrap()
      .remove("location")
  }
}
