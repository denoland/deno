// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods, reason = "test code")]

use std::time::SystemTime;

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
