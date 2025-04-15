// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use deno_core::op2;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::ByteString;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_error::JsErrorBox;
use futures::Stream;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

mod lsc_shard;
mod lscache;
mod sqlite;

pub use lsc_shard::CacheShard;
pub use lscache::LscBackend;
pub use sqlite::SqliteBackedCache;
use tokio_util::io::StreamReader;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CacheError {
  #[class(type)]
  #[error("CacheStorage is not available in this context")]
  ContextUnsupported,
  #[class(type)]
  #[error("Cache name cannot be empty")]
  EmptyName,
  #[class(type)]
  #[error("Cache is not available")]
  NotAvailable,
  #[class(type)]
  #[error("Cache not found")]
  NotFound,
  #[class(type)]
  #[error("Cache deletion is not supported")]
  DeletionNotSupported,
  #[class(type)]
  #[error("Content-Encoding is not allowed in response headers")]
  ContentEncodingNotAllowed,
  #[class(generic)]
  #[error(transparent)]
  Sqlite(#[from] rusqlite::Error),
  #[class(generic)]
  #[error(transparent)]
  JoinError(#[from] tokio::task::JoinError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Other(JsErrorBox),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[class(type)]
  #[error(transparent)]
  InvalidHeaderName(#[from] hyper::header::InvalidHeaderName),
  #[class(type)]
  #[error(transparent)]
  InvalidHeaderValue(#[from] hyper::header::InvalidHeaderValue),
  #[class(type)]
  #[error(transparent)]
  Hyper(#[from] hyper::Error),
  #[class(generic)]
  #[error(transparent)]
  ClientError(#[from] hyper_util::client::legacy::Error),
  #[class(generic)]
  #[error("Failed to create cache storage directory {}", .dir.display())]
  CacheStorageDirectory {
    dir: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[class(generic)]
  #[error("cache {method} request failed: {status}")]
  RequestFailed {
    method: &'static str,
    status: hyper::StatusCode,
  },
}

#[derive(Clone)]
pub struct CreateCache(pub Arc<dyn Fn() -> Result<CacheImpl, CacheError>>);

deno_core::extension!(deno_cache,
  deps = [ deno_webidl, deno_web, deno_url, deno_fetch ],
  ops = [
    op_cache_storage_open,
    op_cache_storage_has,
    op_cache_storage_delete,
    op_cache_put,
    op_cache_match,
    op_cache_delete,
  ],
  esm = [ "01_cache.js" ],
  options = {
    maybe_create_cache: Option<CreateCache>,
  },
  state = |state, options| {
    if let Some(create_cache) = options.maybe_create_cache {
      state.put(create_cache);
    }
  },
);

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

  async fn storage_open(&self, cache_name: String) -> Result<i64, CacheError>;
  async fn storage_has(&self, cache_name: String) -> Result<bool, CacheError>;
  async fn storage_delete(
    &self,
    cache_name: String,
  ) -> Result<bool, CacheError>;

  /// Put a resource into the cache.
  async fn put(
    &self,
    request_response: CachePutRequest,
    resource: Option<Rc<dyn Resource>>,
  ) -> Result<(), CacheError>;

  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<Self::CacheMatchResourceType>)>,
    CacheError,
  >;
  async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, CacheError>;
}

#[derive(Clone)]
pub enum CacheImpl {
  Sqlite(SqliteBackedCache),
  Lsc(LscBackend),
}

#[async_trait(?Send)]
impl Cache for CacheImpl {
  type CacheMatchResourceType = CacheResponseResource;

  async fn storage_open(&self, cache_name: String) -> Result<i64, CacheError> {
    match self {
      Self::Sqlite(cache) => cache.storage_open(cache_name).await,
      Self::Lsc(cache) => cache.storage_open(cache_name).await,
    }
  }

  async fn storage_has(&self, cache_name: String) -> Result<bool, CacheError> {
    match self {
      Self::Sqlite(cache) => cache.storage_has(cache_name).await,
      Self::Lsc(cache) => cache.storage_has(cache_name).await,
    }
  }

  async fn storage_delete(
    &self,
    cache_name: String,
  ) -> Result<bool, CacheError> {
    match self {
      Self::Sqlite(cache) => cache.storage_delete(cache_name).await,
      Self::Lsc(cache) => cache.storage_delete(cache_name).await,
    }
  }

  async fn put(
    &self,
    request_response: CachePutRequest,
    resource: Option<Rc<dyn Resource>>,
  ) -> Result<(), CacheError> {
    match self {
      Self::Sqlite(cache) => cache.put(request_response, resource).await,
      Self::Lsc(cache) => cache.put(request_response, resource).await,
    }
  }

  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<Self::CacheMatchResourceType>)>,
    CacheError,
  > {
    match self {
      Self::Sqlite(cache) => cache.r#match(request).await,
      Self::Lsc(cache) => cache.r#match(request).await,
    }
  }

  async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, CacheError> {
    match self {
      Self::Sqlite(cache) => cache.delete(request).await,
      Self::Lsc(cache) => cache.delete(request).await,
    }
  }
}

pub enum CacheResponseResource {
  Sqlite(AsyncRefCell<tokio::fs::File>),
  Lsc(AsyncRefCell<Pin<Box<dyn AsyncRead>>>),
}

impl CacheResponseResource {
  fn sqlite(file: tokio::fs::File) -> Self {
    Self::Sqlite(AsyncRefCell::new(file))
  }

  fn lsc(
    body: impl Stream<Item = Result<Bytes, std::io::Error>> + 'static,
  ) -> Self {
    Self::Lsc(AsyncRefCell::new(Box::pin(StreamReader::new(body))))
  }

  async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let nread = match &*self {
      CacheResponseResource::Sqlite(_) => {
        let resource = deno_core::RcRef::map(&self, |r| match r {
          Self::Sqlite(r) => r,
          _ => unreachable!(),
        });
        let mut file = resource.borrow_mut().await;
        file.read(data).await?
      }
      CacheResponseResource::Lsc(_) => {
        let resource = deno_core::RcRef::map(&self, |r| match r {
          Self::Lsc(r) => r,
          _ => unreachable!(),
        });
        let mut file = resource.borrow_mut().await;
        file.read(data).await?
      }
    };

    Ok(nread)
  }
}

impl Resource for CacheResponseResource {
  deno_core::impl_readable_byob!();

  fn name(&self) -> Cow<str> {
    "CacheResponseResource".into()
  }
}

#[op2(async)]
#[number]
pub async fn op_cache_storage_open(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<i64, CacheError> {
  let cache = get_cache(&state)?;
  cache.storage_open(cache_name).await
}

#[op2(async)]
pub async fn op_cache_storage_has(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<bool, CacheError> {
  let cache = get_cache(&state)?;
  cache.storage_has(cache_name).await
}

#[op2(async)]
pub async fn op_cache_storage_delete(
  state: Rc<RefCell<OpState>>,
  #[string] cache_name: String,
) -> Result<bool, CacheError> {
  let cache = get_cache(&state)?;
  cache.storage_delete(cache_name).await
}

#[op2(async)]
pub async fn op_cache_put(
  state: Rc<RefCell<OpState>>,
  #[serde] request_response: CachePutRequest,
) -> Result<(), CacheError> {
  let cache = get_cache(&state)?;
  let resource = match request_response.response_rid {
    Some(rid) => Some(
      state
        .borrow_mut()
        .resource_table
        .take_any(rid)
        .map_err(CacheError::Resource)?,
    ),
    None => None,
  };
  cache.put(request_response, resource).await
}

#[op2(async)]
#[serde]
pub async fn op_cache_match(
  state: Rc<RefCell<OpState>>,
  #[serde] request: CacheMatchRequest,
) -> Result<Option<CacheMatchResponse>, CacheError> {
  let cache = get_cache(&state)?;
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
pub async fn op_cache_delete(
  state: Rc<RefCell<OpState>>,
  #[serde] request: CacheDeleteRequest,
) -> Result<bool, CacheError> {
  let cache = get_cache(&state)?;
  cache.delete(request).await
}

pub fn get_cache(
  state: &Rc<RefCell<OpState>>,
) -> Result<CacheImpl, CacheError> {
  let mut state = state.borrow_mut();
  if let Some(cache) = state.try_borrow::<CacheImpl>() {
    Ok(cache.clone())
  } else if let Some(create_cache) = state.try_borrow::<CreateCache>() {
    let cache = create_cache.0()?;
    state.put(cache);
    Ok(state.borrow::<CacheImpl>().clone())
  } else {
    Err(CacheError::ContextUnsupported)
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
