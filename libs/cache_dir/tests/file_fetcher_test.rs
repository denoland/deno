// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods, reason = "test code")]

use std::time::SystemTime;

use deno_cache_dir::Checksum;
use deno_cache_dir::file_fetcher::AuthTokens;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::CachedOrRedirect;
use deno_cache_dir::file_fetcher::FetchLocalOptions;
use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind;
use deno_cache_dir::file_fetcher::FetchNoFollowOptions;
use deno_cache_dir::file_fetcher::FileFetcher;
use deno_cache_dir::file_fetcher::FileFetcherOptions;
use deno_cache_dir::file_fetcher::FileOrRedirect;
use deno_cache_dir::file_fetcher::HttpClient;
use deno_cache_dir::file_fetcher::NullBlobStore;
use deno_cache_dir::file_fetcher::NullMemoryFiles;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_cache_dir::memory::MemoryHttpCache;
use deno_maybe_sync::new_rc;
use http::HeaderMap;
use sys_traits::FsCreateDirAll;
use sys_traits::FsWrite;
use sys_traits::impls::InMemorySys;
use url::Url;

#[tokio::test]
async fn test_file_fetcher_redirects() {
  #[derive(Debug)]
  struct TestHttpClient;

  #[async_trait::async_trait(?Send)]
  impl HttpClient for TestHttpClient {
    async fn send_no_follow(
      &self,
      _url: &Url,
      _headers: HeaderMap,
    ) -> Result<SendResponse, SendError> {
      Ok(SendResponse::Redirect(HeaderMap::new()))
    }
  }

  let sys = InMemorySys::default();
  let file_fetcher = create_file_fetcher(sys.clone(), TestHttpClient);
  let result = file_fetcher
    .fetch_no_follow(
      &Url::parse("http://localhost/bad_redirect").unwrap(),
      FetchNoFollowOptions::default(),
    )
    .await;

  match result.unwrap_err().into_kind() {
    FetchNoFollowErrorKind::RedirectHeaderParse(err) => {
      assert_eq!(err.request_url.as_str(), "http://localhost/bad_redirect");
    }
    err => unreachable!("{:?}", err),
  }

  let time = SystemTime::now();
  sys.set_time(Some(time));
  sys.fs_create_dir_all("/").unwrap();
  sys.fs_write("/some_path.ts", "text").unwrap();

  for include_mtime in [true, false] {
    let result = file_fetcher
      .fetch_no_follow(
        &Url::parse("file:///some_path.ts").unwrap(),
        FetchNoFollowOptions {
          local: FetchLocalOptions { include_mtime },
          ..Default::default()
        },
      )
      .await;
    match result.unwrap() {
      FileOrRedirect::File(file) => {
        assert_eq!(file.mtime, if include_mtime { Some(time) } else { None });
        assert_eq!(file.source.to_vec(), b"text");
      }
      FileOrRedirect::Redirect(_) => unreachable!(),
    }
  }

  sys.fs_create_dir_all("/dir").unwrap();
  let dir_url = Url::parse("file:///dir").unwrap();
  let result = file_fetcher
    .fetch_no_follow(&dir_url, FetchNoFollowOptions::default())
    .await;
  let err = result.unwrap_err();
  assert_eq!(
    err.to_string(),
    "[ERR_UNSUPPORTED_DIR_IMPORT] Directory import 'file:///dir' is not supported resolving ES modules"
  );
  match err.into_kind() {
    FetchNoFollowErrorKind::UnsupportedDirImport(err) => {
      assert_eq!(err.url, dir_url);
    }
    err => unreachable!("{err:?}"),
  }
}

#[tokio::test]
async fn test_file_fetcher_ensure_cached() {
  #[derive(Debug)]
  struct TestHttpClient;

  #[async_trait::async_trait(?Send)]
  impl HttpClient for TestHttpClient {
    async fn send_no_follow(
      &self,
      url: &Url,
      _headers: HeaderMap,
    ) -> Result<SendResponse, SendError> {
      if url.path() == "/redirect" {
        let mut header_map = HeaderMap::new();
        header_map.insert(http::header::LOCATION, "/home".parse().unwrap());
        Ok(SendResponse::Redirect(header_map))
      } else {
        Ok(SendResponse::Success(
          HeaderMap::new(),
          "hello".to_string().into_bytes(),
        ))
      }
    }
  }

  let sys = InMemorySys::default();
  let file_fetcher = create_file_fetcher(sys.clone(), TestHttpClient);
  {
    let result = file_fetcher
      .ensure_cached_no_follow(
        &Url::parse("http://localhost/redirect").unwrap(),
        FetchNoFollowOptions::default(),
      )
      .await;
    assert_eq!(
      result.unwrap(),
      CachedOrRedirect::Redirect(Url::parse("http://localhost/home").unwrap())
    );
  }
  {
    let result = file_fetcher
      .ensure_cached_no_follow(
        &Url::parse("http://localhost/other").unwrap(),
        FetchNoFollowOptions::default(),
      )
      .await;
    assert_eq!(result.unwrap(), CachedOrRedirect::Cached);
  }

  sys.fs_create_dir_all("/").unwrap();
  sys.fs_write("/some_path.ts", "text").unwrap();
  {
    let result = file_fetcher
      .ensure_cached_no_follow(
        &Url::parse("file:///some_path.ts").unwrap(),
        FetchNoFollowOptions::default(),
      )
      .await;
    assert_eq!(result.unwrap(), CachedOrRedirect::Cached);
  }
  {
    let url = Url::parse("file:///not_exists.ts").unwrap();
    let result = file_fetcher
      .ensure_cached_no_follow(&url, FetchNoFollowOptions::default())
      .await;
    match result.unwrap_err().as_kind() {
      FetchNoFollowErrorKind::NotFound(not_found_url) => {
        assert_eq!(url, *not_found_url)
      }
      _ => unreachable!(),
    }
  }
}

fn create_file_fetcher<TClient: HttpClient>(
  sys: InMemorySys,
  client: TClient,
) -> FileFetcher<NullBlobStore, InMemorySys, TClient> {
  FileFetcher::new(
    NullBlobStore,
    sys,
    new_rc(MemoryHttpCache::default()),
    client,
    new_rc(NullMemoryFiles),
    FileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::Use,
      auth_tokens: AuthTokens::new(None),
    },
  )
}

// Regression test for https://github.com/denoland/deno/issues/15189.
// When a URL with a query string redirects to a target URL, subsequent
// fetches must hit the cache for both the redirect entry and the target.
#[tokio::test]
async fn test_fetch_query_redirect_to_dts_is_cached() {
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;

  #[derive(Debug, Clone, Default)]
  struct TestHttpClient {
    #[allow(clippy::disallowed_types, reason = "arc wrapper type")]
    request_count: deno_maybe_sync::MaybeArc<AtomicUsize>,
  }

  #[async_trait::async_trait(?Send)]
  impl HttpClient for TestHttpClient {
    async fn send_no_follow(
      &self,
      url: &Url,
      _headers: HeaderMap,
    ) -> Result<SendResponse, SendError> {
      self.request_count.fetch_add(1, Ordering::SeqCst);
      if url.path() == "/swagger-client" && url.query() == Some("dts") {
        let mut header_map = HeaderMap::new();
        header_map.insert(
          http::header::LOCATION,
          "/-/swagger-client@v3.18.5-Nt8AmAsJyCx6gdL9U9cP/dist=es2019,mode=types/index.d.ts"
            .parse()
            .unwrap(),
        );
        return Ok(SendResponse::Redirect(header_map));
      }
      let mut header_map = HeaderMap::new();
      header_map
        .insert("content-type", "application/typescript".parse().unwrap());
      Ok(SendResponse::Success(
        header_map,
        b"export const foo: string;\n".to_vec(),
      ))
    }
  }

  let sys = InMemorySys::default();
  let client = TestHttpClient::default();
  let request_count = client.request_count.clone();
  let file_fetcher = create_file_fetcher(sys, client);
  let entry_url =
    Url::parse("https://cdn.skypack.dev/swagger-client?dts").unwrap();
  let dts_url = Url::parse("https://cdn.skypack.dev/-/swagger-client@v3.18.5-Nt8AmAsJyCx6gdL9U9cP/dist=es2019,mode=types/index.d.ts").unwrap();

  // First run: follow the redirect manually, since fetch_no_follow does not.
  match file_fetcher
    .fetch_no_follow(&entry_url, FetchNoFollowOptions::default())
    .await
    .unwrap()
  {
    FileOrRedirect::Redirect(url) => assert_eq!(url, dts_url),
    FileOrRedirect::File(_) => unreachable!(),
  }
  match file_fetcher
    .fetch_no_follow(&dts_url, FetchNoFollowOptions::default())
    .await
    .unwrap()
  {
    FileOrRedirect::File(file) => {
      assert_eq!(&*file.source, b"export const foo: string;\n")
    }
    FileOrRedirect::Redirect(_) => unreachable!(),
  }
  assert_eq!(request_count.load(Ordering::SeqCst), 2);

  // Second run: both lookups must come from the cache, no extra requests.
  match file_fetcher
    .fetch_no_follow(&entry_url, FetchNoFollowOptions::default())
    .await
    .unwrap()
  {
    FileOrRedirect::Redirect(url) => assert_eq!(url, dts_url),
    FileOrRedirect::File(_) => unreachable!(),
  }
  match file_fetcher
    .fetch_no_follow(&dts_url, FetchNoFollowOptions::default())
    .await
    .unwrap()
  {
    FileOrRedirect::File(file) => {
      assert_eq!(&*file.source, b"export const foo: string;\n")
    }
    FileOrRedirect::Redirect(_) => unreachable!(),
  }
  assert_eq!(
    request_count.load(Ordering::SeqCst),
    2,
    "second cache pass should not issue any HTTP requests"
  );

  // ensure_cached_no_follow must also return Redirect for the cached entry
  // url, so deno_graph follows it instead of treating the redirect as the
  // module itself.
  assert_eq!(
    file_fetcher
      .ensure_cached_no_follow(&entry_url, FetchNoFollowOptions::default())
      .await
      .unwrap(),
    CachedOrRedirect::Redirect(dts_url.clone()),
  );
  assert_eq!(
    file_fetcher
      .ensure_cached_no_follow(&dts_url, FetchNoFollowOptions::default())
      .await
      .unwrap(),
    CachedOrRedirect::Cached,
  );
  assert_eq!(
    request_count.load(Ordering::SeqCst),
    2,
    "ensure_cached_no_follow must not issue HTTP requests for cached entries"
  );
}

// Regression test for https://github.com/denoland/deno/issues/25404.
// A URL responding with 404 must be negatively cached so it's not
// re-requested on every process start, until the entry expires or
// the cache is reloaded.
#[tokio::test]
async fn test_fetch_not_found_is_negatively_cached() {
  use std::sync::atomic::AtomicBool;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;
  use std::time::Duration;
  use std::time::UNIX_EPOCH;

  use deno_cache_dir::memory::MemoryHttpCache;

  #[derive(Debug, Clone, Default)]
  struct TestHttpClient {
    #[allow(clippy::disallowed_types, reason = "arc wrapper type")]
    request_count: deno_maybe_sync::MaybeArc<AtomicUsize>,
    #[allow(clippy::disallowed_types, reason = "arc wrapper type")]
    respond_success: deno_maybe_sync::MaybeArc<AtomicBool>,
  }

  #[async_trait::async_trait(?Send)]
  impl HttpClient for TestHttpClient {
    async fn send_no_follow(
      &self,
      _url: &Url,
      _headers: HeaderMap,
    ) -> Result<SendResponse, SendError> {
      self.request_count.fetch_add(1, Ordering::SeqCst);
      if self.respond_success.load(Ordering::SeqCst) {
        let mut header_map = HeaderMap::new();
        header_map
          .insert("content-type", "application/typescript".parse().unwrap());
        Ok(SendResponse::Success(header_map, b"export {};\n".to_vec()))
      } else {
        Err(SendError::NotFound)
      }
    }
  }

  fn assert_not_found(
    result: Result<
      FileOrRedirect,
      deno_cache_dir::file_fetcher::FetchNoFollowError,
    >,
  ) {
    match result.unwrap_err().into_kind() {
      FetchNoFollowErrorKind::NotFound(_) => {}
      err => unreachable!("{err:?}"),
    }
  }

  let sys = InMemorySys::default();
  sys.set_time(Some(UNIX_EPOCH));
  let client = TestHttpClient::default();
  let request_count = client.request_count.clone();
  let respond_success = client.respond_success.clone();
  let file_fetcher = FileFetcher::new(
    NullBlobStore,
    sys.clone(),
    // share the clock between the cache and the file fetcher
    new_rc(MemoryHttpCache::new(sys.clone())),
    client,
    new_rc(NullMemoryFiles),
    FileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::Use,
      auth_tokens: AuthTokens::new(None),
    },
  );
  let url = Url::parse("https://localhost/not_found.ts").unwrap();

  // first fetch issues a request and caches the 404
  assert_not_found(
    file_fetcher
      .fetch_no_follow(&url, FetchNoFollowOptions::default())
      .await,
  );
  assert_eq!(request_count.load(Ordering::SeqCst), 1);

  // subsequent fetches are served from the negative cache
  assert_not_found(
    file_fetcher
      .fetch_no_follow(&url, FetchNoFollowOptions::default())
      .await,
  );
  assert_not_found(
    file_fetcher
      .fetch_no_follow(
        &url,
        FetchNoFollowOptions {
          maybe_checksum: Some(Checksum::new("not matching")),
          ..Default::default()
        },
      )
      .await,
  );
  match file_fetcher
    .ensure_cached_no_follow(&url, FetchNoFollowOptions::default())
    .await
    .unwrap_err()
    .into_kind()
  {
    FetchNoFollowErrorKind::NotFound(_) => {}
    err => unreachable!("{err:?}"),
  }
  assert_eq!(
    request_count.load(Ordering::SeqCst),
    1,
    "a cached 404 should not issue any HTTP requests"
  );

  // fetch_cached reports a cached 404 as not having a usable cached file
  assert_eq!(file_fetcher.fetch_cached(&url, 10).unwrap(), None);

  // reloading bypasses the negative cache
  assert_not_found(
    file_fetcher
      .fetch_no_follow(
        &url,
        FetchNoFollowOptions {
          maybe_cache_setting: Some(&CacheSetting::ReloadAll),
          ..Default::default()
        },
      )
      .await,
  );
  assert_eq!(request_count.load(Ordering::SeqCst), 2);

  // the URL now exists on the server, but the cached 404 is still fresh
  respond_success.store(true, Ordering::SeqCst);
  assert_not_found(
    file_fetcher
      .fetch_no_follow(&url, FetchNoFollowOptions::default())
      .await,
  );
  assert_eq!(request_count.load(Ordering::SeqCst), 2);

  // after the negative cache entry expires, the URL is re-requested
  sys.set_time(Some(UNIX_EPOCH + Duration::from_secs(16 * 60)));
  match file_fetcher
    .fetch_no_follow(&url, FetchNoFollowOptions::default())
    .await
    .unwrap()
  {
    FileOrRedirect::File(file) => {
      assert_eq!(&*file.source, b"export {};\n");
    }
    FileOrRedirect::Redirect(_) => unreachable!(),
  }
  assert_eq!(request_count.load(Ordering::SeqCst), 3);
}
