// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::ByteString;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

mod sqlite;
pub use sqlite::SqliteBackedCache;

#[derive(Clone)]
pub struct CreateCache<C: Cache + 'static>(pub Arc<dyn Fn() -> C>);

deno_core::extension!(deno_cache,
  deps = [ deno_webidl, deno_web, deno_url, deno_fetch ],
  parameters=[CA: Cache],
  ops = [
    op_cache_storage_open<CA>,
    op_cache_storage_has<CA>,
    op_cache_storage_delete<CA>,
    op_cache_put<CA>,
    op_cache_match<CA>,
    op_cache_delete<CA>,
  ],
  esm = [ "01_cache.js" ],
  options = {
    maybe_create_cache: Option<CreateCache<CA>>,
  },
  state = |state, options| {
    if let Some(create_cache) = options.maybe_create_cache {
      state.put(create_cache);
    }
  },
);

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
  pub response_status: u16,
  pub response_status_text: String,
  pub response_rid: Option<ResourceId>,
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

#[async_trait(?Send)]
pub trait Cache: Clone + 'static {
  type CacheMatchResourceType: Resource;

  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError>;
  async fn storage_has(&self, cache_name: String) -> Result<bool, AnyError>;
  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError>;

  /// Put a resource into the cache.
  async fn put(
    &self,
    request_response: CachePutRequest,
    resource: Option<Rc<dyn Resource>>,
  ) -> Result<(), AnyError>;

  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<Self::CacheMatchResourceType>)>,
    AnyError,
  >;
  async fn delete(&self, request: CacheDeleteRequest)
    -> Result<bool, AnyError>;
}

#[op2(async)]
#[number]
pub async fn op_cache_storage_open<CA>(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<i64, AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_open(cache_name).await
}

#[op2(async)]
pub async fn op_cache_storage_has<CA>(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_has(cache_name).await
}

#[op2(async)]
pub async fn op_cache_storage_delete<CA>(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  cache.storage_delete(cache_name).await
}

#[op2(async)]
pub async fn op_cache_put<CA>(
  state: Rc<RefCell<OpState>>,
  #[serde] request_response: CachePutRequest,
) -> Result<(), AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  let resource = match request_response.response_rid {
    Some(rid) => Some(state.borrow_mut().resource_table.take_any(rid)?),
    None => None,
  };
  cache.put(request_response, resource).await
}

#[op2(async)]
#[serde]
pub async fn op_cache_match<CA>(
  state: Rc<RefCell<OpState>>,
  #[serde] request: CacheMatchRequest,
) -> Result<Option<CacheMatchResponse>, AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  match cache.r#match(request).await? {
    Some((meta, None)) => Ok(Some(CacheMatchResponse(meta, None))),
    Some((meta, Some(resource))) => {
      let rid = state.borrow_mut().resource_table.add(resource);
      Ok(Some(CacheMatchResponse(meta, Some(rid))))
    }
    None => Ok(None),
  }
}

#[op2(async)]
pub async fn op_cache_delete<CA>(
  state: Rc<RefCell<OpState>>,
  #[serde] request: CacheDeleteRequest,
) -> Result<bool, AnyError>
where
  CA: Cache,
{
  let cache = get_cache::<CA>(&state)?;
  cache.delete(request).await
}

pub fn get_cache<CA>(state: &Rc<RefCell<OpState>>) -> Result<CA, AnyError>
where
  CA: Cache,
{
  let mut state = state.borrow_mut();
  if let Some(cache) = state.try_borrow::<CA>() {
    Ok(cache.clone())
  } else if let Some(create_cache) = state.try_borrow::<CreateCache<CA>>() {
    let cache = create_cache.0();
    state.put(cache);
    Ok(state.borrow::<CA>().clone())
  } else {
    Err(type_error("CacheStorage is not available in this context"))
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
