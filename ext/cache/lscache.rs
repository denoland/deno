// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;

use async_stream::try_stream;
use async_trait::async_trait;
use base64::Engine;
use bytes::Bytes;
use deno_core::unsync::spawn;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufMutView;
use deno_core::ByteString;
use deno_core::Resource;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use http::header::VARY;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use slab::Slab;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use crate::get_header;
use crate::get_headers_from_vary_header;
use crate::lsc_shard::CacheShard;
use crate::Cache;
use crate::CacheDeleteRequest;
use crate::CacheError;
use crate::CacheMatchRequest;
use crate::CacheMatchResponseMeta;
use crate::CachePutRequest;

const REQHDR_PREFIX: &str = "x-lsc-meta-reqhdr-";

#[derive(Clone, Default)]
pub struct LscBackend {
  shard: Rc<RefCell<Option<Rc<CacheShard>>>>,
  id2name: Rc<RefCell<Slab<String>>>,
}

impl LscBackend {
  pub fn set_shard(&self, shard: Rc<CacheShard>) {
    *self.shard.borrow_mut() = Some(shard);
  }
}

#[async_trait(?Send)]
impl Cache for LscBackend {
  type CacheMatchResourceType = CacheResponseResource;

  /// Open a cache storage. Internally, this allocates an id and maps it
  /// to the provided cache name.
  async fn storage_open(&self, cache_name: String) -> Result<i64, CacheError> {
    if cache_name.is_empty() {
      return Err(CacheError::EmptyName);
    }
    let id = self.id2name.borrow_mut().insert(cache_name);
    Ok(id as i64)
  }

  /// Check if a cache with the provided name exists. Always returns `true`.
  async fn storage_has(&self, _cache_name: String) -> Result<bool, CacheError> {
    Ok(true)
  }

  /// Delete a cache storage. Not yet implemented.
  async fn storage_delete(
    &self,
    _cache_name: String,
  ) -> Result<bool, CacheError> {
    Err(CacheError::DeletionNotSupported)
  }

  /// Writes an entry to the cache.
  async fn put(
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
    let body = reqwest::Body::wrap_stream(body_rx);
    shard.put_object(&object_key, headers, body).await?;
    Ok(())
  }

  /// Matches a request against the cache.
  async fn r#match(
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
    if let Some(vary_header) = res.headers().get(&VARY) {
      if !vary_header_matches(
        vary_header.as_bytes(),
        &request.request_headers,
        res.headers(),
      ) {
        return Ok(None);
      }
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
    {
      if let Ok(cached_at) = chrono::DateTime::parse_from_rfc3339(x) {
        let age = chrono::Utc::now()
          .signed_duration_since(cached_at)
          .num_seconds();
        if age >= 0 {
          response_headers.push(("age".into(), age.to_string().into()));
        }
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

    let body = CacheResponseResource::new(
      res
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
    );

    Ok(Some((meta, Some(body))))
  }

  async fn delete(
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
    shard
      .put_object(&object_key, headers, reqwest::Body::from(&[][..]))
      .await?;
    Ok(true)
  }
}
impl deno_core::Resource for LscBackend {
  fn name(&self) -> std::borrow::Cow<str> {
    "LscBackend".into()
  }
}

pub struct CacheResponseResource {
  body: AsyncRefCell<Pin<Box<dyn AsyncRead>>>,
}

impl CacheResponseResource {
  fn new(
    body: impl Stream<Item = Result<Bytes, std::io::Error>> + 'static,
  ) -> Self {
    Self {
      body: AsyncRefCell::new(Box::pin(StreamReader::new(body))),
    }
  }

  async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let resource = deno_core::RcRef::map(&self, |r| &r.body);
    let mut body = resource.borrow_mut().await;
    let nread = body.read(data).await?;
    Ok(nread)
  }
}

impl Resource for CacheResponseResource {
  deno_core::impl_readable_byob!();

  fn name(&self) -> Cow<str> {
    "CacheResponseResource".into()
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
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(cache_name),
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(request_url),
  )
}
