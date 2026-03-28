// Copyright 2018-2026 the Deno authors. MIT license.

#![cfg(feature = "graph")]

use std::collections::HashMap;
use std::path::PathBuf;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::HttpClient;
use deno_cache_dir::file_fetcher::NullBlobStore;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_graph::source::CacheSetting as LoaderCacheSetting;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_graph::source::LoaderChecksum;
use deno_resolver::factory::ConfigDiscoveryOption;
use deno_resolver::factory::WorkspaceFactory;
use deno_resolver::factory::WorkspaceFactoryOptions;
use deno_resolver::file_fetcher::DenoGraphLoader;
use deno_resolver::file_fetcher::DenoGraphLoaderOptions;
use deno_resolver::file_fetcher::PermissionedFileFetcher;
use deno_resolver::file_fetcher::PermissionedFileFetcherOptions;
use deno_resolver::loader::MemoryFiles;
use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
use deno_resolver::npm::DenoInNpmPackageChecker;
use sys_traits::impls::InMemorySys;
use url::Url;

#[derive(Debug)]
struct SuccessfulTestHttpClient;

#[async_trait::async_trait(?Send)]
impl HttpClient for SuccessfulTestHttpClient {
  async fn send_no_follow(
    &self,
    _url: &Url,
    _headers: http::HeaderMap,
  ) -> Result<SendResponse, SendError> {
    let mut headers = http::HeaderMap::new();
    headers.insert(
      http::header::CONTENT_TYPE,
      "application/typescript".parse().unwrap(),
    );
    Ok(SendResponse::Success(
      headers,
      b"export const value = 1;\n".to_vec(),
    ))
  }
}

fn create_test_loader<THttpClient: HttpClient + 'static>(
  sys: InMemorySys,
  http_client: THttpClient,
) -> DenoGraphLoader<NullBlobStore, InMemorySys, THttpClient> {
  let cwd = get_cwd();
  let factory = WorkspaceFactory::new(
    sys.clone(),
    cwd.join("project"),
    WorkspaceFactoryOptions {
      maybe_custom_deno_dir_root: Some(cwd.join("deno_dir")),
      config_discovery: ConfigDiscoveryOption::Disabled,
      ..Default::default()
    },
  );
  let global_http_cache = factory.global_http_cache().unwrap().clone();
  let memory_files = deno_maybe_sync::new_rc(MemoryFiles::default());
  let file_fetcher = deno_maybe_sync::new_rc(PermissionedFileFetcher::new(
    NullBlobStore,
    deno_maybe_sync::new_rc(deno_cache_dir::GlobalOrLocalHttpCache::from(
      global_http_cache.clone(),
    )),
    http_client,
    memory_files,
    sys.clone(),
    PermissionedFileFetcherOptions {
      allow_remote: true,
      cache_setting: CacheSetting::Use,
    },
  ));
  DenoGraphLoader::new(
    file_fetcher,
    global_http_cache,
    DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm),
    sys,
    DenoGraphLoaderOptions {
      file_header_overrides: HashMap::new(),
      permissions: None,
      reporter: None,
      include_npm_sources: false,
    },
  )
}

fn load_options(
  cache_setting: LoaderCacheSetting,
  maybe_checksum: Option<LoaderChecksum>,
) -> deno_graph::source::LoadOptions {
  deno_graph::source::LoadOptions {
    in_dynamic_branch: false,
    was_dynamic_root: false,
    cache_setting,
    maybe_checksum,
  }
}

fn stale_checksum() -> LoaderChecksum {
  LoaderChecksum::new(
    "0000000000000000000000000000000000000000000000000000000000000000"
      .to_string(),
  )
}

fn get_cwd() -> PathBuf {
  if cfg!(windows) {
    PathBuf::from("K:\\folder\\")
  } else {
    PathBuf::from("/")
  }
}

#[test]
fn reload_ignores_stale_remote_checksum() {
  let cwd = get_cwd();
  let sys = InMemorySys::new_with_cwd(&cwd);
  let loader = create_test_loader(sys, SuccessfulTestHttpClient);
  let specifier = Url::parse("http://localhost/mod.ts").unwrap();

  let mut pool = futures::executor::LocalPool::new();
  let result = pool
    .run_until(loader.load(
      &specifier,
      load_options(LoaderCacheSetting::Reload, Some(stale_checksum())),
    ))
    .unwrap();

  match result {
    Some(LoadResponse::Module {
      content: loaded_content,
      specifier: loaded_specifier,
      ..
    }) => {
      assert_eq!(loaded_specifier, specifier);
      assert_eq!(&*loaded_content, b"export const value = 1;\n");
    }
    other => panic!("expected Module response, got {:?}", other),
  }
}

#[test]
fn use_still_validates_remote_checksum() {
  let cwd = get_cwd();
  let sys = InMemorySys::new_with_cwd(&cwd);
  let loader = create_test_loader(sys, SuccessfulTestHttpClient);
  let specifier = Url::parse("http://localhost/mod.ts").unwrap();

  let mut pool = futures::executor::LocalPool::new();
  let result = pool.run_until(loader.load(
    &specifier,
    load_options(LoaderCacheSetting::Use, Some(stale_checksum())),
  ));

  assert!(result.is_err());
}
