// Copyright 2018-2025 the Deno authors. MIT license.

use std::convert::Infallible;
use std::future::Future;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV6;
use std::path::PathBuf;

use bytes::Bytes;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use http_body_util::combinators::UnsyncBoxBody;
use hyper::body::Incoming;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;

use super::custom_headers;
use super::empty_body;
use super::hyper_utils::HandlerOutput;
use super::run_server;
use super::string_body;
use super::ServerKind;
use super::ServerOptions;
use crate::npm;

pub fn public_npm_registry(port: u16) -> Vec<LocalBoxFuture<'static, ()>> {
  run_npm_server(port, "npm registry server error", {
    move |req| async move {
      handle_req_for_registry(req, &npm::PUBLIC_TEST_NPM_REGISTRY).await
    }
  })
}

const PRIVATE_NPM_REGISTRY_AUTH_TOKEN: &str = "private-reg-token";
const PRIVATE_NPM_REGISTRY_2_AUTH_TOKEN: &str = "private-reg-token2";

// `deno:land` encoded using base64
const PRIVATE_NPM_REGISTRY_AUTH_BASE64: &str = "ZGVubzpsYW5k";
// `deno:land2` encoded using base64
const PRIVATE_NPM_REGISTRY_2_AUTH_BASE64: &str = "ZGVubzpsYW5kMg==";

pub fn private_npm_registry1(port: u16) -> Vec<LocalBoxFuture<'static, ()>> {
  run_npm_server(
    port,
    "npm private registry server error",
    private_npm_registry1_handler,
  )
}

pub fn private_npm_registry2(port: u16) -> Vec<LocalBoxFuture<'static, ()>> {
  run_npm_server(
    port,
    "npm private registry server error",
    private_npm_registry2_handler,
  )
}

pub fn private_npm_registry3(port: u16) -> Vec<LocalBoxFuture<'static, ()>> {
  run_npm_server(
    port,
    "npm private registry server error",
    private_npm_registry3_handler,
  )
}

fn run_npm_server<F, S>(
  port: u16,
  error_msg: &'static str,
  handler: F,
) -> Vec<LocalBoxFuture<'static, ()>>
where
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  let npm_registry_addr = SocketAddr::from(([127, 0, 0, 1], port));
  let ipv6_loopback = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
  let npm_registry_ipv6_addr =
    SocketAddr::V6(SocketAddrV6::new(ipv6_loopback, port, 0, 0));
  vec![
    run_npm_server_for_addr(npm_registry_addr, error_msg, handler)
      .boxed_local(),
    // necessary because the npm binary will sometimes resolve localhost to ::1
    run_npm_server_for_addr(npm_registry_ipv6_addr, error_msg, handler)
      .boxed_local(),
  ]
}

async fn run_npm_server_for_addr<F, S>(
  addr: SocketAddr,
  error_msg: &'static str,
  handler: F,
) where
  F: Fn(Request<hyper::body::Incoming>) -> S + Copy + 'static,
  S: Future<Output = HandlerOutput> + 'static,
{
  run_server(
    ServerOptions {
      addr,
      kind: ServerKind::Auto,
      error_msg,
    },
    handler,
  )
  .await
}

async fn private_npm_registry1_handler(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let auth = req
    .headers()
    .get("authorization")
    .and_then(|x| x.to_str().ok())
    .unwrap_or_default();
  if auth != format!("Bearer {}", PRIVATE_NPM_REGISTRY_AUTH_TOKEN)
    && auth != format!("Basic {}", PRIVATE_NPM_REGISTRY_AUTH_BASE64)
  {
    return Ok(
      Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(empty_body())
        .unwrap(),
    );
  }

  handle_req_for_registry(req, &npm::PRIVATE_TEST_NPM_REGISTRY_1).await
}

async fn private_npm_registry2_handler(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let auth = req
    .headers()
    .get("authorization")
    .and_then(|x| x.to_str().ok())
    .unwrap_or_default();
  if auth != format!("Bearer {}", PRIVATE_NPM_REGISTRY_2_AUTH_TOKEN)
    && auth != format!("Basic {}", PRIVATE_NPM_REGISTRY_2_AUTH_BASE64)
  {
    return Ok(
      Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(empty_body())
        .unwrap(),
    );
  }

  handle_req_for_registry(req, &npm::PRIVATE_TEST_NPM_REGISTRY_2).await
}

async fn private_npm_registry3_handler(
  req: Request<hyper::body::Incoming>,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  // No auth for this registry
  handle_req_for_registry(req, &npm::PRIVATE_TEST_NPM_REGISTRY_3).await
}

async fn handle_req_for_registry(
  req: Request<Incoming>,
  test_npm_registry: &npm::TestNpmRegistry,
) -> Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error> {
  let root_dir = test_npm_registry.root_dir();

  // serve the registry package files
  let uri_path = req.uri().path();
  let mut file_path = root_dir.to_path_buf();
  file_path.push(uri_path[1..].replace("%2f", "/").replace("%2F", "/"));

  // serve if the filepath exists
  if let Ok(file) = tokio::fs::read(&file_path).await {
    let file_resp = custom_headers(uri_path, file);
    return Ok(file_resp);
  }

  // otherwise try to serve from the registry
  if let Some(resp) =
    try_serve_npm_registry(uri_path, file_path.clone(), test_npm_registry).await
  {
    return resp;
  }

  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(empty_body())
    .map_err(|e| e.into())
}

fn handle_custom_npm_registry_path(
  scope_name: &str,
  path: &str,
  test_npm_registry: &npm::TestNpmRegistry,
) -> Result<Option<Response<UnsyncBoxBody<Bytes, Infallible>>>, anyhow::Error> {
  let mut parts = path
    .split('/')
    .filter(|p| !p.is_empty())
    .collect::<Vec<_>>();
  let remainder = parts.split_off(1);
  let name = parts[0];
  let package_name = format!("{}/{}", scope_name, name);

  if remainder.len() == 1 {
    if let Some(file_bytes) = test_npm_registry
      .tarball_bytes(&package_name, remainder[0].trim_end_matches(".tgz"))?
    {
      let file_resp = custom_headers("file.tgz", file_bytes);
      return Ok(Some(file_resp));
    }
  } else if remainder.is_empty() {
    if let Some(registry_file) =
      test_npm_registry.registry_file(&package_name)?
    {
      let file_resp = custom_headers("registry.json", registry_file);
      return Ok(Some(file_resp));
    }
  }

  Ok(None)
}

fn should_download_npm_packages() -> bool {
  // when this env var is set, it will download and save npm packages
  // to the tests/registry/npm directory
  std::env::var("DENO_TEST_UTIL_UPDATE_NPM") == Ok("1".to_string())
}

async fn try_serve_npm_registry(
  uri_path: &str,
  mut testdata_file_path: PathBuf,
  test_npm_registry: &npm::TestNpmRegistry,
) -> Option<Result<Response<UnsyncBoxBody<Bytes, Infallible>>, anyhow::Error>> {
  if let Some((scope_name, package_name_with_path)) = test_npm_registry
    .get_test_scope_and_package_name_with_path_from_uri_path(uri_path)
  {
    // serve all requests to the `DENOTEST_SCOPE_NAME` or `DENOTEST2_SCOPE_NAME`
    // using the file system at that path
    match handle_custom_npm_registry_path(
      scope_name,
      package_name_with_path,
      test_npm_registry,
    ) {
      Ok(Some(response)) => return Some(Ok(response)),
      Ok(None) => {} // ignore, not found
      Err(err) => {
        return Some(
          Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(string_body(&format!("{err:#}")))
            .map_err(|e| e.into()),
        );
      }
    }
  } else {
    // otherwise, serve based on registry.json and tgz files
    let is_tarball = uri_path.ends_with(".tgz");
    if !is_tarball {
      testdata_file_path.push("registry.json");
    }
    if let Ok(file) = tokio::fs::read(&testdata_file_path).await {
      let file_resp = custom_headers(uri_path, file);
      return Some(Ok(file_resp));
    } else if should_download_npm_packages() {
      if let Err(err) = download_npm_registry_file(
        test_npm_registry,
        uri_path,
        &testdata_file_path,
        is_tarball,
      )
      .await
      {
        return Some(
          Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(string_body(&format!("{err:#}")))
            .map_err(|e| e.into()),
        );
      };

      // serve the file
      if let Ok(file) = tokio::fs::read(&testdata_file_path).await {
        let file_resp = custom_headers(uri_path, file);
        return Some(Ok(file_resp));
      }
    }
  }

  None
}

// Replaces URL of public npm registry (`https://registry.npmjs.org/`) with
// the test registry (`http://localhost:4260`).
//
// These strings end up in `registry.json` files for each downloaded package
// that are stored in `tests/testdata/` directory.
//
// If another npm test registry wants to use them, it should replace
// these values with appropriate URL when serving.
fn replace_default_npm_registry_url_with_test_npm_registry_url(
  text: String,
  npm_registry: &npm::TestNpmRegistry,
  package_name: &str,
) -> String {
  let package_name = percent_encoding::percent_decode_str(package_name)
    .decode_utf8()
    .unwrap();
  text.replace(
    &format!("https://registry.npmjs.org/{}/-/", package_name),
    &npm_registry.package_url(&package_name),
  )
}

async fn download_npm_registry_file(
  test_npm_registry: &npm::TestNpmRegistry,
  uri_path: &str,
  testdata_file_path: &PathBuf,
  is_tarball: bool,
) -> Result<(), anyhow::Error> {
  let uri_path = uri_path.trim_start_matches('/');
  let url_parts = uri_path.split('/').collect::<Vec<_>>();
  let package_name = if url_parts[0].starts_with('@') {
    url_parts.into_iter().take(2).collect::<Vec<_>>().join("/")
  } else {
    url_parts.into_iter().take(1).collect::<Vec<_>>().join("/")
  };
  let url = if is_tarball {
    let file_name = testdata_file_path.file_name().unwrap().to_string_lossy();
    format!("https://registry.npmjs.org/{package_name}/-/{file_name}")
  } else {
    format!("https://registry.npmjs.org/{package_name}")
  };
  let client = reqwest::Client::new();
  let response = client.get(url).send().await?;
  let bytes = response.bytes().await?;
  let bytes = if is_tarball {
    bytes.to_vec()
  } else {
    replace_default_npm_registry_url_with_test_npm_registry_url(
      String::from_utf8(bytes.to_vec()).unwrap(),
      test_npm_registry,
      &package_name,
    )
    .into_bytes()
  };
  std::fs::create_dir_all(testdata_file_path.parent().unwrap())?;
  std::fs::write(testdata_file_path, bytes)?;
  Ok(())
}
