// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::cache::HttpCache;
use crate::cache::RealDenoCacheEnv;
use crate::colors;
use crate::http_util::get_response_body_with_progress;
use crate::http_util::HttpClientProvider;
use crate::util::progress_bar::ProgressBar;

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
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::LoaderChecksum;

use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_web::BlobStore;
use http::header;
use http::HeaderMap;
use http::StatusCode;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

// NEED TO HANDLE THIS:

// convert to the equivalent deno_graph error so that it
// enhances it if this is passed to deno_graph
// Err(
//   deno_graph::source::ChecksumIntegrityError {
//     actual: err.actual,
//     expected: err.expected,
//   }
//   .into(),
// )

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
        &file.specifier,
        file.maybe_headers.as_ref(),
      );
    let specifier = file.specifier;
    match deno_graph::source::decode_source(
      &specifier,
      file.source,
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
      }

      if response.status().is_client_error() {
        let err = if response.status() == StatusCode::NOT_FOUND {
          SendError::NotFound
        } else {
          SendError::StatusCode(response.status())
        };
        return Err(err);
      }

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

#[derive(Debug, Copy, Clone)]
pub enum FetchPermissionsOptionRef<'a> {
  AllowAll,
  DynamicContainer(&'a PermissionsContainer),
  StaticContainer(&'a PermissionsContainer),
}

pub struct FetchOptions<'a> {
  pub specifier: &'a ModuleSpecifier,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
  pub permissions: FetchPermissionsOptionRef<'a>,
}

pub struct FetchNoFollowOptions<'a> {
  pub specifier: &'a ModuleSpecifier,
  pub maybe_auth: Option<(header::HeaderName, header::HeaderValue)>,
  pub maybe_accept: Option<&'a str>,
  pub maybe_cache_setting: Option<&'a CacheSetting>,
  pub maybe_checksum: Option<&'a LoaderChecksum>,
}

type DenoCacheDirFileFetcher = deno_cache_dir::file_fetcher::FileFetcher<
  BlobStoreAdapter,
  RealDenoCacheEnv,
  HttpClientAdapter,
>;

/// A structure for resolving, fetching and caching source files.
#[derive(Debug)]
pub struct CliFileFetcher {
  file_fetcher: DenoCacheDirFileFetcher,
  memory_files: Arc<MemoryFiles>,
}

impl CliFileFetcher {
  pub fn new(
    http_cache: Arc<dyn HttpCache>,
    cache_setting: CacheSetting,
    allow_remote: bool,
    http_client_provider: Arc<HttpClientProvider>,
    blob_store: Arc<BlobStore>,
    progress_bar: Option<ProgressBar>,
    download_log_level: log::Level,
  ) -> Self {
    let memory_files = Arc::new(MemoryFiles::default());
    let file_fetcher = DenoCacheDirFileFetcher::new(
      BlobStoreAdapter(blob_store),
      RealDenoCacheEnv,
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
        auth_tokens: AuthTokens::new(env::var("DENO_AUTH_TOKENS").ok()),
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
        FetchPermissionsOptionRef::StaticContainer(permissions),
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
      .fetch_with_options(FetchOptions {
        specifier,
        maybe_auth,
        maybe_accept: None,
        maybe_cache_setting: None,
        permissions,
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
    let mut maybe_auth = options.maybe_auth;
    for _ in 0..=max_redirect {
      match options.permissions {
        FetchPermissionsOptionRef::AllowAll => {
          // allow
        }
        FetchPermissionsOptionRef::StaticContainer(permissions) => {
          permissions.check_specifier(
            &specifier,
            deno_runtime::deno_permissions::CheckSpecifierKind::Static,
          )?;
        }
        FetchPermissionsOptionRef::DynamicContainer(permissions) => {
          permissions.check_specifier(
            &specifier,
            deno_runtime::deno_permissions::CheckSpecifierKind::Dynamic,
          )?;
        }
      }
      match self
        .fetch_no_follow_with_options(FetchNoFollowOptions {
          specifier: &specifier,
          maybe_auth: maybe_auth.clone(),
          maybe_accept: options.maybe_accept,
          maybe_cache_setting: options.maybe_cache_setting,
          maybe_checksum: None,
        })
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

    Err(custom_error("Http", "Too many redirects."))
  }

  /// Fetches without following redirects.
  pub async fn fetch_no_follow_with_options(
    &self,
    options: FetchNoFollowOptions<'_>,
  ) -> Result<FileOrRedirect, FetchNoFollowError> {
    self
      .file_fetcher
      .fetch_no_follow(
        options.specifier,
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
  ) -> (CliFileFetcher, TempDir) {
    let (file_fetcher, temp_dir, _) =
      setup_with_blob_store(cache_setting, maybe_temp_dir);
    (file_fetcher, temp_dir)
  }

  fn setup_with_blob_store(
    cache_setting: CacheSetting,
    maybe_temp_dir: Option<TempDir>,
  ) -> (CliFileFetcher, TempDir, Arc<BlobStore>) {
    let temp_dir = maybe_temp_dir.unwrap_or_default();
    let location = temp_dir.path().join("remote").to_path_buf();
    let blob_store: Arc<BlobStore> = Default::default();
    let file_fetcher = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(location, RealDenoCacheEnv)),
      cache_setting,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      blob_store.clone(),
      None,
      log::Level::Info,
    );
    (file_fetcher, temp_dir, blob_store)
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
    let (file_fetcher, _) = setup(CacheSetting::ReloadAll, None);
    let result: Result<File, AnyError> = file_fetcher
      .fetch_with_options_and_max_redirect(
        FetchOptions {
          specifier,
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: Some(file_fetcher.cache_setting()),
          permissions: FetchPermissionsOptionRef::AllowAll,
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
    let file_fetcher = CliFileFetcher::new(
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
      let file_fetcher = CliFileFetcher::new(
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
      let file_fetcher = CliFileFetcher::new(
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
      let file_fetcher = CliFileFetcher::new(
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
      let file_fetcher = CliFileFetcher::new(
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
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: Some(&file_fetcher.cache_setting),
        },
        FetchPermissionsOptionRef::AllowAll,
        2,
      )
      .await;
    assert!(result.is_ok());

    let result = file_fetcher
      .fetch_with_options_and_max_redirect(
        FetchOptions {
          specifier: &specifier,
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: Some(&file_fetcher.cache_setting),
        },
        FetchPermissionsOptionRef::AllowAll,
        1,
      )
      .await;
    assert!(result.is_err());

    let result = file_fetcher.fetch_cached_or_local(&specifier, 2);
    assert!(result.is_ok());

    let result = file_fetcher.fetch_cached_or_local(&specifier, 1);
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
    let file_fetcher = CliFileFetcher::new(
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
    let file_fetcher_01 = CliFileFetcher::new(
      Arc::new(GlobalHttpCache::new(location.clone(), RealDenoCacheEnv)),
      CacheSetting::Only,
      true,
      Arc::new(HttpClientProvider::new(None, None)),
      Default::default(),
      None,
    );
    let file_fetcher_02 = CliFileFetcher::new(
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
    file_fetcher: &CliFileFetcher,
    url: &ModuleSpecifier,
  ) -> String {
    let cache_key = file_fetcher.http_cache.cache_item_key(url).unwrap();
    let bytes = file_fetcher
      .http_cache
      .get(&cache_key, None)
      .unwrap()
      .unwrap()
      .content;
    String::from_utf8(bytes.into_owned()).unwrap()
  }

  #[track_caller]
  fn get_location_header_from_cache(
    file_fetcher: &CliFileFetcher,
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
