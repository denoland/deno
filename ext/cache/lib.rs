// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod sqlite;
use deno_core::ByteString;
pub use sqlite::SqliteBackedCache;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
pub struct CreateCache<C: Cache + 'static>(pub Arc<dyn Fn() -> C>);

pub fn init<CA: Cache + 'static>(
  maybe_create_cache: Option<CreateCache<CA>>,
) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec!["deno_webidl", "deno_web", "deno_url", "deno_fetch"])
    .js(include_js_files!(
      prefix "deno:ext/cache",
      "01_cache.js",
    ))
    .ops(vec![
      op_cache_storage_open::decl::<CA>(),
      op_cache_storage_has::decl::<CA>(),
      op_cache_storage_delete::decl::<CA>(),
      op_cache_put::decl::<CA>(),
      op_cache_match::decl::<CA>(),
      op_cache_delete::decl::<CA>(),
    ])
    .state(move |state| {
      if let Some(create_cache) = maybe_create_cache.clone() {
        state.put(create_cache);
      }
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_cache.d.ts")
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CachePutRequest {
  pub cache_id: i64,
  pub request_url: String,
  pub request_headers: Vec<(ByteString, ByteString)>,
  pub response_headers: Vec<(ByteString, ByteString)>,
  pub response_has_body: bool,
  pub response_status: u16,
  pub response_status_text: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CacheMatchRequest {
  pub cache_id: i64,
  pub request_url: String,
  pub request_headers: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMatchResponse(CacheMatchResponseMeta, Option<ResourceId>);

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMatchResponseMeta {
  pub response_status: u16,
  pub response_status_text: String,
  pub request_headers: Vec<(ByteString, ByteString)>,
  pub response_headers: Vec<(ByteString, ByteString)>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CacheDeleteRequest {
  pub cache_id: i64,
  pub request_url: String,
}

#[async_trait]
pub trait Cache: Clone {
  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError>;
  async fn storage_has(&self, cache_name: String) -> Result<bool, AnyError>;
  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError>;

  async fn put(
    &self,
    request_response: CachePutRequest,
  ) -> Result<Option<Rc<dyn Resource>>, AnyError>;
  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<Rc<dyn Resource>>)>,
    AnyError,
  >;
  async fn delete(&self, request: CacheDeleteRequest)
    -> Result<bool, AnyError>;
}

#[op]
pub async fn op_cache_storage_open<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<i64, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_open(cache_name).await
}

#[op]
pub async fn op_cache_storage_has<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_has(cache_name).await
}

#[op]
pub async fn op_cache_storage_delete<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_delete(cache_name).await
}

#[op]
pub async fn op_cache_put<CA>(
  state: Rc<RefCell<OpState>>,
  request_response: CachePutRequest,
) -> Result<Option<ResourceId>, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  match cache.put(request_response).await? {
    Some(resource) => {
      let rid = state.borrow_mut().resource_table.add_rc_dyn(resource);
      Ok(Some(rid))
    }
    None => Ok(None),
  }
}

#[op]
pub async fn op_cache_match<CA>(
  state: Rc<RefCell<OpState>>,
  request: CacheMatchRequest,
) -> Result<Option<CacheMatchResponse>, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  match cache.r#match(request).await? {
    Some((meta, None)) => Ok(Some(CacheMatchResponse(meta, None))),
    Some((meta, Some(resource))) => {
      let rid = state.borrow_mut().resource_table.add_rc_dyn(resource);
      Ok(Some(CacheMatchResponse(meta, Some(rid))))
    }
    None => Ok(None),
  }
}

#[op]
pub async fn op_cache_delete<CA>(
  state: Rc<RefCell<OpState>>,
  request: CacheDeleteRequest,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = get_cache::<CA>(&state)?;
  cache.delete(request).await
}

pub fn get_cache<CA>(state: &Rc<RefCell<OpState>>) -> Result<CA, AnyError>
where
  CA: Cache + 'static,
{
  let mut state = state.borrow_mut();
  if let Some(cache) = state.try_borrow::<CA>() {
    Ok(cache.clone())
  } else {
    let create_cache = state.borrow::<CreateCache<CA>>().clone();
    let cache = create_cache.0();
    state.put(cache);
    Ok(state.borrow::<CA>().clone())
  }
}

/// Check if headers, mentioned in the vary header, of query request
/// and cached request are equal.
pub fn vary_header_matches(
  vary_header: &ByteString,
  query_request_headers: &[(ByteString, ByteString)],
  cached_request_headers: &[(ByteString, ByteString)],
) -> bool {
  let vary_header = match std::str::from_utf8(vary_header) {
    Ok(vary_header) => vary_header,
    Err(_) => return false,
  };
  let headers = get_headers_from_vary_header(vary_header);
  for header in headers {
    let query_header = get_header(&header, query_request_headers);
    let cached_header = get_header(&header, cached_request_headers);
    if query_header != cached_header {
      return false;
    }
  }
  true
}

#[test]
fn test_vary_header_matches() {
  let vary_header = ByteString::from("accept-encoding");
  let query_request_headers = vec![(
    ByteString::from("accept-encoding"),
    ByteString::from("gzip"),
  )];
  let cached_request_headers = vec![(
    ByteString::from("accept-encoding"),
    ByteString::from("gzip"),
  )];
  assert!(vary_header_matches(
    &vary_header,
    &query_request_headers,
    &cached_request_headers
  ));
  let vary_header = ByteString::from("accept-encoding");
  let query_request_headers = vec![(
    ByteString::from("accept-encoding"),
    ByteString::from("gzip"),
  )];
  let cached_request_headers =
    vec![(ByteString::from("accept-encoding"), ByteString::from("br"))];
  assert!(!vary_header_matches(
    &vary_header,
    &query_request_headers,
    &cached_request_headers
  ));
}

/// Get headers from the vary header.
pub fn get_headers_from_vary_header(vary_header: &str) -> Vec<String> {
  vary_header
    .split(',')
    .map(|s| s.trim().to_lowercase())
    .collect()
}

#[test]
fn test_get_headers_from_vary_header() {
  let headers = get_headers_from_vary_header("accept-encoding");
  assert_eq!(headers, vec!["accept-encoding"]);
  let headers = get_headers_from_vary_header("accept-encoding, user-agent");
  assert_eq!(headers, vec!["accept-encoding", "user-agent"]);
}

/// Get value for the header with the given name.
pub fn get_header(
  name: &str,
  headers: &[(ByteString, ByteString)],
) -> Option<ByteString> {
  headers
    .iter()
    .find(|(k, _)| {
      if let Ok(k) = std::str::from_utf8(k) {
        k.eq_ignore_ascii_case(name)
      } else {
        false
      }
    })
    .map(|(_, v)| v.to_owned())
}

#[test]
fn test_get_header() {
  let headers = vec![
    (
      ByteString::from("accept-encoding"),
      ByteString::from("gzip"),
    ),
    (
      ByteString::from("content-type"),
      ByteString::from("application/json"),
    ),
    (
      ByteString::from("vary"),
      ByteString::from("accept-encoding"),
    ),
  ];
  let value = get_header("accept-encoding", &headers);
  assert_eq!(value, Some(ByteString::from("gzip")));
  let value = get_header("content-type", &headers);
  assert_eq!(value, Some(ByteString::from("application/json")));
  let value = get_header("vary", &headers);
  assert_eq!(value, Some(ByteString::from("accept-encoding")));
}

/// Serialize headers into bytes.
pub fn serialize_headers(headers: &[(ByteString, ByteString)]) -> Vec<u8> {
  let mut serialized_headers = Vec::new();
  for (name, value) in headers {
    serialized_headers.extend_from_slice(name);
    serialized_headers.extend_from_slice(b"\r\n");
    serialized_headers.extend_from_slice(value);
    serialized_headers.extend_from_slice(b"\r\n");
  }
  serialized_headers
}

/// Deserialize bytes into headers.
pub fn deserialize_headers(
  serialized_headers: &[u8],
) -> Vec<(ByteString, ByteString)> {
  let mut headers = Vec::new();
  let mut piece = None;
  let mut start = 0;
  for (i, byte) in serialized_headers.iter().enumerate() {
    if byte == &b'\r' && serialized_headers.get(i + 1) == Some(&b'\n') {
      if piece.is_none() {
        piece = Some(start..i);
      } else {
        let name = piece.unwrap();
        let value = start..i;
        headers.push((
          ByteString::from(&serialized_headers[name]),
          ByteString::from(&serialized_headers[value]),
        ));
        piece = None;
      }
      start = i + 2;
    }
  }
  assert!(piece.is_none());
  assert_eq!(start, serialized_headers.len());
  headers
}
