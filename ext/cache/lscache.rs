// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use async_stream::try_stream;
use base64::Engine;
use bytes::Bytes;
use deno_core::BufMutView;
use deno_core::ByteString;
use deno_core::Resource;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::unsync::spawn;
use deno_error::JsErrorBox;
use futures::StreamExt;
use futures::TryStreamExt;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::VARY;
use http_body_util::BodyExt;
use http_body_util::Full;
use http_body_util::combinators::UnsyncBoxBody;
use slab::Slab;

use crate::CacheDeleteRequest;
use crate::CacheError;
use crate::CacheKey;
use crate::CacheKeysRequest;
use crate::CacheMatchRequest;
use crate::CacheMatchResponseMeta;
use crate::CachePutRequest;
use crate::CacheResponseResource;
use crate::cache_key_matches_request;
use crate::get_header;
use crate::get_headers_from_vary_header;
use crate::lsc_shard::CacheShard;

const REQHDR_PREFIX: &str = "x-lsc-meta-reqhdr-";

#[derive(Clone, Default)]
pub struct LscBackend {
  shard: Rc<RefCell<Option<Rc<CacheShard>>>>,
  id2name: Rc<RefCell<Slab<String>>>,
  keys: Rc<RefCell<HashMap<String, LscCacheKeys>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LscCacheKey {
  request_url: String,
  request_headers: Vec<(ByteString, ByteString)>,
  response_headers: Vec<(ByteString, ByteString)>,
}

#[derive(Deserialize, Serialize)]
struct PersistedLscCacheKey {
  request_url: String,
  request_headers: Vec<(Vec<u8>, Vec<u8>)>,
  response_headers: Vec<(Vec<u8>, Vec<u8>)>,
}

impl From<&LscCacheKey> for PersistedLscCacheKey {
  fn from(key: &LscCacheKey) -> Self {
    Self {
      request_url: key.request_url.clone(),
      request_headers: key
        .request_headers
        .iter()
        .map(|(name, value)| (name.to_vec(), value.to_vec()))
        .collect(),
      response_headers: key
        .response_headers
        .iter()
        .map(|(name, value)| (name.to_vec(), value.to_vec()))
        .collect(),
    }
  }
}

impl From<PersistedLscCacheKey> for LscCacheKey {
  fn from(key: PersistedLscCacheKey) -> Self {
    Self {
      request_url: key.request_url,
      request_headers: key
        .request_headers
        .into_iter()
        .map(|(name, value)| (ByteString::from(name), ByteString::from(value)))
        .collect(),
      response_headers: key
        .response_headers
        .into_iter()
        .map(|(name, value)| (ByteString::from(name), ByteString::from(value)))
        .collect(),
    }
  }
}

#[derive(Clone, Default)]
struct LscCacheKeys {
  order: Vec<String>,
  entries: HashMap<String, LscCacheKey>,
}

impl LscCacheKeys {
  fn insert(&mut self, key: LscCacheKey) {
    let request_url = key.request_url.clone();
    if !self.entries.contains_key(&request_url) {
      self.order.push(request_url.clone());
    }
    self.entries.insert(request_url, key);
  }

  fn remove(&mut self, request_url: &str) -> bool {
    if self.entries.remove(request_url).is_some() {
      self.order.retain(|url| url != request_url);
      true
    } else {
      false
    }
  }

  fn iter(&self) -> impl Iterator<Item = &LscCacheKey> {
    self.order.iter().filter_map(|url| self.entries.get(url))
  }

  fn from_keys(keys: Vec<LscCacheKey>) -> Self {
    let mut result = Self::default();
    for key in keys {
      result.insert(key);
    }
    result
  }

  #[cfg(test)]
  fn to_vec(&self) -> Vec<LscCacheKey> {
    self.iter().cloned().collect()
  }
}

impl LscBackend {
  pub fn set_shard(&self, shard: Rc<CacheShard>) {
    *self.shard.borrow_mut() = Some(shard);
  }
}

#[allow(clippy::unused_async, reason = "trait requires async interface")]
impl LscBackend {
  /// Open a cache storage. Internally, this allocates an id and maps it
  /// to the provided cache name.
  pub async fn storage_open(
    &self,
    cache_name: String,
  ) -> Result<i64, CacheError> {
    if cache_name.is_empty() {
      return Err(CacheError::EmptyName);
    }
    let maybe_shard = self.shard.borrow().as_ref().cloned();
    let cache_keys = if let Some(shard) = maybe_shard {
      load_cache_keys(&shard, &cache_name).await?
    } else {
      LscCacheKeys::default()
    };
    let id = self.id2name.borrow_mut().insert(cache_name.clone());
    self.keys.borrow_mut().insert(cache_name, cache_keys);
    Ok(id as i64)
  }

  /// Check if a cache with the provided name exists. Always returns `true`.
  pub async fn storage_has(
    &self,
    _cache_name: String,
  ) -> Result<bool, CacheError> {
    Ok(true)
  }

  /// Delete a cache storage. Not yet implemented.
  pub async fn storage_delete(
    &self,
    _cache_name: String,
  ) -> Result<bool, CacheError> {
    Err(CacheError::DeletionNotSupported)
  }

  /// List all cache names currently known to this backend.
  pub async fn storage_keys(&self) -> Result<Vec<String>, CacheError> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for (_, name) in self.id2name.borrow().iter() {
      if seen.insert(name.clone()) {
        names.push(name.clone());
      }
    }
    Ok(names)
  }

  /// Writes an entry to the cache.
  pub async fn put(
    &self,
    request_response: CachePutRequest,
    resource: Option<Rc<dyn Resource>>,
  ) -> Result<(), CacheError> {
    let Some(shard) = self.shard.borrow().as_ref().cloned() else {
      return Err(CacheError::NotAvailable);
    };

    let Some(cache_name) = self
      .id2name
      .borrow_mut()
      .get(request_response.cache_id as usize)
      .cloned()
    else {
      return Err(CacheError::NotFound);
    };
    let object_key = build_cache_object_key(
      cache_name.as_bytes(),
      request_response.request_url.as_bytes(),
    );
    let mut headers = HeaderMap::new();
    for hdr in &request_response.request_headers {
      headers.insert(
        HeaderName::from_bytes(
          &[REQHDR_PREFIX.as_bytes(), &hdr.0[..]].concat(),
        )?,
        HeaderValue::from_bytes(&hdr.1[..])?,
      );
    }
    for hdr in &request_response.response_headers {
      if hdr.0.starts_with(b"x-lsc-meta-") {
        continue;
      }
      if hdr.0[..] == b"content-encoding"[..] {
        return Err(CacheError::ContentEncodingNotAllowed);
      }
      headers.insert(
        HeaderName::from_bytes(&hdr.0[..])?,
        HeaderValue::from_bytes(&hdr.1[..])?,
      );
    }

    headers.insert(
      HeaderName::from_bytes(b"x-lsc-meta-cached-at")?,
      HeaderValue::from_bytes(
        chrono::Utc::now()
          .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
          .as_bytes(),
      )?,
    );

    let body = try_stream! {
      if let Some(resource) = resource {
        loop {
          let (size, buf) = resource.clone().read_byob(BufMutView::new(64 * 1024)).await.map_err(CacheError::Other)?;
          if size == 0 {
            break;
          }
          yield Bytes::copy_from_slice(&buf[..size]);
        }
      }
    };
    let (body_tx, body_rx) = futures::channel::mpsc::channel(4);
    spawn(body.map(Ok::<Result<_, CacheError>, _>).forward(body_tx));
    let body = http_body_util::StreamBody::new(
      body_rx.into_stream().map_ok(http_body::Frame::data),
    );
    let body = UnsyncBoxBody::new(body);
    shard.put_object(&object_key, headers, body).await?;

    let key = LscCacheKey {
      request_url: request_response.request_url,
      request_headers: request_response.request_headers,
      response_headers: request_response.response_headers,
    };
    let cache_keys = {
      let keys = self.keys.borrow();
      let mut cache_keys = keys.get(&cache_name).cloned().unwrap_or_default();
      cache_keys.insert(key);
      cache_keys
    };
    persist_cache_keys(&shard, &cache_name, &cache_keys).await?;
    self.keys.borrow_mut().insert(cache_name, cache_keys);

    Ok(())
  }

  /// Matches a request against the cache.
  pub async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<
    Option<(CacheMatchResponseMeta, Option<CacheResponseResource>)>,
    CacheError,
  > {
    let Some(shard) = self.shard.borrow().as_ref().cloned() else {
      return Err(CacheError::NotAvailable);
    };
    let Some(cache_name) = self
      .id2name
      .borrow()
      .get(request.cache_id as usize)
      .cloned()
    else {
      return Err(CacheError::NotFound);
    };
    let object_key = build_cache_object_key(
      cache_name.as_bytes(),
      request.request_url.as_bytes(),
    );
    let Some(res) = shard.get_object(&object_key).await? else {
      return Ok(None);
    };

    // Is this a tombstone?
    if res.headers().contains_key("x-lsc-meta-deleted-at") {
      return Ok(None);
    }

    // From https://w3c.github.io/ServiceWorker/#request-matches-cached-item-algorithm
    // If there's Vary header in the response, ensure all the
    // headers of the cached request match the query request.
    if let Some(vary_header) = res.headers().get(&VARY)
      && !vary_header_matches(
        vary_header.as_bytes(),
        &request.request_headers,
        res.headers(),
      )
    {
      return Ok(None);
    }

    let mut response_headers: Vec<(ByteString, ByteString)> = res
      .headers()
      .iter()
      .filter_map(|(k, v)| {
        if k.as_str().starts_with("x-lsc-meta-") || k.as_str() == "x-ryw" {
          None
        } else {
          Some((k.as_str().into(), v.as_bytes().into()))
        }
      })
      .collect();

    if let Some(x) = res
      .headers()
      .get("x-lsc-meta-cached-at")
      .and_then(|x| x.to_str().ok())
      && let Ok(cached_at) = chrono::DateTime::parse_from_rfc3339(x)
    {
      let age = chrono::Utc::now()
        .signed_duration_since(cached_at)
        .num_seconds();
      if age >= 0 {
        response_headers.push(("age".into(), age.to_string().into()));
      }
    }

    let meta = CacheMatchResponseMeta {
      response_status: res.status().as_u16(),
      response_status_text: res
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string(),
      request_headers: res
        .headers()
        .iter()
        .filter_map(|(k, v)| {
          let reqhdr_prefix = REQHDR_PREFIX.as_bytes();
          if k.as_str().as_bytes().starts_with(reqhdr_prefix) {
            Some((
              k.as_str().as_bytes()[REQHDR_PREFIX.len()..].into(),
              v.as_bytes().into(),
            ))
          } else {
            None
          }
        })
        .collect(),
      response_headers,
    };

    let body = http_body_util::BodyDataStream::new(res.into_body())
      .into_stream()
      .map_err(std::io::Error::other);
    let body = CacheResponseResource::lsc(body);

    Ok(Some((meta, Some(body))))
  }

  pub async fn keys(
    &self,
    request: CacheKeysRequest,
  ) -> Result<Vec<CacheKey>, CacheError> {
    let Some(_shard) = self.shard.borrow().as_ref().cloned() else {
      return Err(CacheError::NotAvailable);
    };

    let Some(cache_name) = self
      .id2name
      .borrow()
      .get(request.cache_id as usize)
      .cloned()
    else {
      return Err(CacheError::NotFound);
    };

    let keys = self.keys.borrow();
    let Some(cache_keys) = keys.get(&cache_name) else {
      return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for key in cache_keys.iter() {
      if cache_key_matches_request(
        request.request_url.as_deref(),
        &request.request_headers,
        &key.request_url,
        &key.request_headers,
        &key.response_headers,
        &request.options,
      ) {
        result.push(CacheKey {
          request_url: key.request_url.clone(),
          request_headers: key.request_headers.clone(),
        });
      }
    }

    Ok(result)
  }

  pub async fn delete(
    &self,
    request: CacheDeleteRequest,
  ) -> Result<bool, CacheError> {
    let Some(shard) = self.shard.borrow().as_ref().cloned() else {
      return Err(CacheError::NotAvailable);
    };

    let Some(cache_name) = self
      .id2name
      .borrow_mut()
      .get(request.cache_id as usize)
      .cloned()
    else {
      return Err(CacheError::NotFound);
    };
    let request_url = request.request_url.clone();
    let object_key = build_cache_object_key(
      cache_name.as_bytes(),
      request.request_url.as_bytes(),
    );
    let mut headers = HeaderMap::new();
    headers.insert(
      HeaderName::from_bytes(b"expires")?,
      HeaderValue::from_bytes(b"Thu, 01 Jan 1970 00:00:00 GMT")?,
    );
    headers.insert(
      HeaderName::from_bytes(b"x-lsc-meta-deleted-at")?,
      HeaderValue::from_bytes(
        chrono::Utc::now()
          .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
          .as_bytes(),
      )?,
    );
    shard.put_object_empty(&object_key, headers).await?;
    let cache_keys = {
      let keys = self.keys.borrow();
      if let Some(cache_keys) = keys.get(&cache_name).cloned() {
        let mut cache_keys = cache_keys;
        if cache_keys.remove(&request_url) {
          Some(cache_keys)
        } else {
          None
        }
      } else {
        None
      }
    };
    if let Some(cache_keys) = cache_keys {
      persist_cache_keys(&shard, &cache_name, &cache_keys).await?;
      self.keys.borrow_mut().insert(cache_name, cache_keys);
    }
    Ok(true)
  }
}
impl deno_core::Resource for LscBackend {
  fn name(&self) -> std::borrow::Cow<'_, str> {
    "LscBackend".into()
  }
}

fn vary_header_matches(
  vary_header: &[u8],
  query_request_headers: &[(ByteString, ByteString)],
  cached_headers: &HeaderMap,
) -> bool {
  let vary_header = match std::str::from_utf8(vary_header) {
    Ok(vary_header) => vary_header,
    Err(_) => return false,
  };
  let headers = get_headers_from_vary_header(vary_header);
  for header in headers {
    // Ignoring `accept-encoding` is safe because we refuse to cache responses
    // with `content-encoding`
    if header == "accept-encoding" {
      continue;
    }
    let lookup_key = format!("{}{}", REQHDR_PREFIX, header);
    let query_header = get_header(&header, query_request_headers);
    let cached_header = cached_headers.get(&lookup_key);
    if query_header.as_ref().map(|x| &x[..])
      != cached_header.as_ref().map(|x| x.as_bytes())
    {
      return false;
    }
  }
  true
}

fn build_cache_object_key(cache_name: &[u8], request_url: &[u8]) -> String {
  format!(
    "v1/{}/{}",
    encode_key_part(cache_name),
    encode_key_part(request_url),
  )
}

fn build_cache_keys_object_key(cache_name: &[u8]) -> String {
  format!("v1/{}/metadata/keys", encode_key_part(cache_name))
}

fn encode_key_part(value: &[u8]) -> String {
  base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
}

async fn load_cache_keys(
  shard: &CacheShard,
  cache_name: &str,
) -> Result<LscCacheKeys, CacheError> {
  let object_key = build_cache_keys_object_key(cache_name.as_bytes());
  let Some(res) = shard.get_object(&object_key).await? else {
    return Ok(LscCacheKeys::default());
  };
  if res.headers().contains_key("x-lsc-meta-deleted-at") {
    return Ok(LscCacheKeys::default());
  }
  let body = res.into_body().collect().await?.to_bytes();
  let keys: Vec<PersistedLscCacheKey> =
    serde_json::from_slice(&body).map_err(json_error)?;
  Ok(LscCacheKeys::from_keys(
    keys.into_iter().map(Into::into).collect(),
  ))
}

async fn persist_cache_keys(
  shard: &CacheShard,
  cache_name: &str,
  cache_keys: &LscCacheKeys,
) -> Result<(), CacheError> {
  let keys = cache_keys
    .iter()
    .map(PersistedLscCacheKey::from)
    .collect::<Vec<_>>();
  let body = serde_json::to_vec(&keys).map_err(json_error)?;
  let body = Full::new(Bytes::from(body)).map_err(|err| match err {});
  let body = UnsyncBoxBody::new(body);
  let mut headers = HeaderMap::new();
  headers.insert(
    HeaderName::from_bytes(b"content-type")?,
    HeaderValue::from_bytes(b"application/json")?,
  );
  shard
    .put_object(
      &build_cache_keys_object_key(cache_name.as_bytes()),
      headers,
      body,
    )
    .await
}

fn json_error(err: serde_json::Error) -> CacheError {
  CacheError::Other(JsErrorBox::generic(err.to_string()))
}

#[cfg(test)]
mod tests {
  use std::convert::Infallible;
  use std::sync::Arc;
  use std::sync::Mutex;

  use hyper::Method;
  use hyper::Request;
  use hyper::Response;
  use hyper::StatusCode;
  use hyper::body::Incoming;
  use hyper::service::service_fn;
  use hyper_util::rt::TokioIo;

  use super::*;

  #[derive(Clone)]
  struct StoredObject {
    headers: HeaderMap,
    body: Bytes,
  }

  type StoredObjects = Arc<Mutex<HashMap<String, StoredObject>>>;

  fn lsc_cache_key(request_url: &str) -> LscCacheKey {
    LscCacheKey {
      request_url: request_url.to_string(),
      request_headers: vec![(
        ByteString::from("accept"),
        ByteString::from("*/*"),
      )],
      response_headers: vec![(
        ByteString::from("content-type"),
        ByteString::from("text/plain"),
      )],
    }
  }

  fn cache_keys_request(cache_id: i64) -> CacheKeysRequest {
    CacheKeysRequest {
      cache_id,
      request_url: None,
      request_headers: Vec::new(),
      options: Default::default(),
    }
  }

  fn cache_put_request(cache_id: i64, request_url: &str) -> CachePutRequest {
    CachePutRequest {
      cache_id,
      request_url: request_url.to_string(),
      request_headers: vec![(
        ByteString::from("accept"),
        ByteString::from("*/*"),
      )],
      response_headers: vec![(
        ByteString::from("content-type"),
        ByteString::from("text/plain"),
      )],
      response_status: 200,
      response_status_text: "OK".to_string(),
      response_rid: None,
    }
  }

  async fn start_lsc_object_server() -> String {
    let objects = Arc::new(Mutex::new(HashMap::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
      while let Ok((stream, _)) = listener.accept().await {
        let objects = objects.clone();
        tokio::spawn(async move {
          let service = service_fn(move |req| {
            handle_lsc_object_request(req, objects.clone())
          });
          let io = TokioIo::new(stream);
          let _ = hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await;
        });
      }
    });
    endpoint
  }

  async fn handle_lsc_object_request(
    req: Request<Incoming>,
    objects: StoredObjects,
  ) -> Result<Response<Full<Bytes>>, Infallible> {
    let Some(object_key) = req.uri().path().strip_prefix("/objects/") else {
      return Ok(response(StatusCode::NOT_FOUND, HeaderMap::new(), None));
    };
    let object_key = object_key.to_string();

    match *req.method() {
      Method::GET => {
        let Some(object) = objects.lock().unwrap().get(&object_key).cloned()
        else {
          return Ok(response(StatusCode::NOT_FOUND, HeaderMap::new(), None));
        };
        Ok(response(StatusCode::OK, object.headers, Some(object.body)))
      }
      Method::PUT => {
        let headers = req
          .headers()
          .iter()
          .filter(|(name, _)| {
            !matches!(
              name.as_str(),
              "authorization" | "host" | "content-length"
            )
          })
          .map(|(name, value)| (name.clone(), value.clone()))
          .collect();
        let body = req.into_body().collect().await.unwrap().to_bytes();
        objects
          .lock()
          .unwrap()
          .insert(object_key, StoredObject { headers, body });
        Ok(response(StatusCode::OK, HeaderMap::new(), None))
      }
      _ => Ok(response(
        StatusCode::METHOD_NOT_ALLOWED,
        HeaderMap::new(),
        None,
      )),
    }
  }

  fn response(
    status: StatusCode,
    headers: HeaderMap,
    body: Option<Bytes>,
  ) -> Response<Full<Bytes>> {
    let mut response = Response::builder().status(status);
    for (name, value) in headers {
      if let Some(name) = name {
        response = response.header(name, value);
      }
    }
    response.body(Full::new(body.unwrap_or_default())).unwrap()
  }

  #[test]
  fn lsc_cache_keys_replace_without_reordering() {
    let mut keys = LscCacheKeys::default();
    keys.insert(lsc_cache_key("https://example.com/a"));
    keys.insert(lsc_cache_key("https://example.com/b"));
    let mut replacement = lsc_cache_key("https://example.com/a");
    replacement.request_headers = vec![(
      ByteString::from("accept"),
      ByteString::from("application/json"),
    )];
    keys.insert(replacement);

    assert_eq!(
      keys
        .iter()
        .map(|key| key.request_url.as_str())
        .collect::<Vec<_>>(),
      vec!["https://example.com/a", "https://example.com/b"]
    );
    assert_eq!(keys.entries.len(), 2);
    assert_eq!(
      keys.entries["https://example.com/a"].request_headers,
      vec![(
        ByteString::from("accept"),
        ByteString::from("application/json")
      )]
    );
  }

  #[test]
  fn lsc_cache_keys_serde_roundtrip() {
    let mut keys = LscCacheKeys::default();
    keys.insert(lsc_cache_key("https://example.com/a"));
    keys.insert(lsc_cache_key("https://example.com/b"));

    let serialized = serde_json::to_vec(
      &keys
        .iter()
        .map(PersistedLscCacheKey::from)
        .collect::<Vec<_>>(),
    )
    .unwrap();
    let deserialized: Vec<PersistedLscCacheKey> =
      serde_json::from_slice(&serialized).unwrap();
    let keys = LscCacheKeys::from_keys(
      deserialized.into_iter().map(Into::into).collect(),
    );

    assert_eq!(
      keys.to_vec(),
      vec![
        lsc_cache_key("https://example.com/a"),
        lsc_cache_key("https://example.com/b")
      ]
    );
  }

  #[tokio::test(flavor = "current_thread")]
  async fn lsc_cache_keys_survive_backend_reopen() {
    let shard = Rc::new(CacheShard::new(
      start_lsc_object_server().await,
      "test-token".to_string(),
    ));
    let backend = LscBackend::default();
    backend.set_shard(shard.clone());
    let cache_id = backend.storage_open("cache-v1".to_string()).await.unwrap();
    backend
      .put(cache_put_request(cache_id, "https://example.com/a"), None)
      .await
      .unwrap();
    backend
      .put(cache_put_request(cache_id, "https://example.com/b"), None)
      .await
      .unwrap();

    let restarted_backend = LscBackend::default();
    restarted_backend.set_shard(shard.clone());
    let restarted_cache_id = restarted_backend
      .storage_open("cache-v1".to_string())
      .await
      .unwrap();
    let keys = restarted_backend
      .keys(cache_keys_request(restarted_cache_id))
      .await
      .unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].request_url, "https://example.com/a");
    assert_eq!(keys[1].request_url, "https://example.com/b");

    restarted_backend
      .delete(CacheDeleteRequest {
        cache_id: restarted_cache_id,
        request_url: "https://example.com/a".to_string(),
      })
      .await
      .unwrap();

    let reopened_after_delete = LscBackend::default();
    reopened_after_delete.set_shard(shard);
    let reopened_cache_id = reopened_after_delete
      .storage_open("cache-v1".to_string())
      .await
      .unwrap();
    let keys = reopened_after_delete
      .keys(cache_keys_request(reopened_cache_id))
      .await
      .unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].request_url, "https://example.com/b");
  }
}
