// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::compressible::is_content_compressible;
use crate::extract_network_stream;
use crate::hyper_util_tokioio::TokioIo;
use crate::network_buffered_stream::NetworkStreamPrefixCheck;
use crate::request_body::HttpRequestBody;
use crate::request_properties::HttpConnectionProperties;
use crate::request_properties::HttpListenProperties;
use crate::request_properties::HttpPropertyExtractor;
use crate::response_body::Compression;
use crate::response_body::ResponseBytes;
use crate::response_body::ResponseBytesInner;
use crate::slab::slab_drop;
use crate::slab::slab_get;
use crate::slab::slab_init;
use crate::slab::slab_insert;
use crate::slab::RefCount;
use crate::slab::HttpRequestBodyAutocloser;
use crate::slab::SlabId;
use crate::websocket_upgrade::WebSocketUpgrade;
use crate::LocalExecutor;
use cache_control::CacheControl;
use deno_core::error::AnyError;
use deno_core::futures::future::poll_fn;
use deno_core::futures::TryFutureExt;
use deno_core::op;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::serde_v8::from_v8;
use deno_core::unsync::spawn;
use deno_core::unsync::JoinHandle;
use deno_core::v8;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_net::ops_tls::TlsStream;
use deno_net::raw::NetworkStream;
use deno_websocket::ws_create_server_stream;
use fly_accept_encoding::Encoding;
use http::header::ACCEPT_ENCODING;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_RANGE;
use http::header::CONTENT_TYPE;
use http::HeaderMap;
use hyper1::body::Incoming;
use hyper1::header::COOKIE;
use hyper1::http::HeaderName;
use hyper1::http::HeaderValue;
use hyper1::server::conn::http1;
use hyper1::server::conn::http2;
use hyper1::service::service_fn;
use hyper1::service::HttpService;
use hyper1::StatusCode;
use once_cell::sync::Lazy;
use pin_project::pin_project;
use pin_project::pinned_drop;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

type Request = hyper1::Request<Incoming>;
type Response = hyper1::Response<ResponseBytes>;

static USE_WRITEV: Lazy<bool> = Lazy::new(|| {
  let enable = std::env::var("DENO_USE_WRITEV").ok();

  if let Some(val) = enable {
    return !val.is_empty();
  }

  false
});

/// All HTTP/2 connections start with this byte string.
///
/// In HTTP/2, each endpoint is required to send a connection preface as a final confirmation
/// of the protocol in use and to establish the initial settings for the HTTP/2 connection. The
/// client and server each send a different connection preface.
///
/// The client connection preface starts with a sequence of 24 octets, which in hex notation is:
///
/// 0x505249202a20485454502f322e300d0a0d0a534d0d0a0d0a
///
/// That is, the connection preface starts with the string PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n). This sequence
/// MUST be followed by a SETTINGS frame (Section 6.5), which MAY be empty.
const HTTP2_PREFIX: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// ALPN negotiation for "h2"
const TLS_ALPN_HTTP_2: &[u8] = b"h2";

/// ALPN negotiation for "http/1.1"
const TLS_ALPN_HTTP_11: &[u8] = b"http/1.1";

/// Name a trait for streams we can serve HTTP over.
trait HttpServeStream:
  tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
{
}
impl<
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
  > HttpServeStream for S
{
}

#[op2(fast)]
#[smi]
pub fn op_http_upgrade_raw(
  state: &mut OpState,
  #[smi] slab_id: SlabId,
) -> Result<ResourceId, AnyError> {
  // Stage 1: extract the upgrade future
  let upgrade = slab_get(slab_id).upgrade()?;
  let (read, write) = tokio::io::duplex(1024);
  let (read_rx, write_tx) = tokio::io::split(read);
  let (mut write_rx, mut read_tx) = tokio::io::split(write);
  spawn(async move {
    let mut upgrade_stream = WebSocketUpgrade::<ResponseBytes>::default();

    // Stage 2: Extract the Upgraded connection
    let mut buf = [0; 1024];
    let upgraded = loop {
      let read = Pin::new(&mut write_rx).read(&mut buf).await?;
      match upgrade_stream.write(&buf[..read]) {
        Ok(None) => continue,
        Ok(Some((response, bytes))) => {
          let mut http = slab_get(slab_id);
          *http.response() = response;
          http.complete();
          let mut upgraded = TokioIo::new(upgrade.await?);
          upgraded.write_all(&bytes).await?;
          break upgraded;
        }
        Err(err) => return Err(err),
      }
    };

    // Stage 3: Pump the data
    let (mut upgraded_rx, mut upgraded_tx) = tokio::io::split(upgraded);

    spawn(async move {
      let mut buf = [0; 1024];
      loop {
        let read = upgraded_rx.read(&mut buf).await?;
        if read == 0 {
          break;
        }
        read_tx.write_all(&buf[..read]).await?;
      }
      Ok::<_, AnyError>(())
    });
    spawn(async move {
      let mut buf = [0; 1024];
      loop {
        let read = write_rx.read(&mut buf).await?;
        if read == 0 {
          break;
        }
        upgraded_tx.write_all(&buf[..read]).await?;
      }
      Ok::<_, AnyError>(())
    });

    Ok(())
  });

  Ok(
    state
      .resource_table
      .add(UpgradeStream::new(read_rx, write_tx)),
  )
}

#[op2(async)]
#[smi]
pub async fn op_http_upgrade_websocket_next(
  state: Rc<RefCell<OpState>>,
  #[smi] slab_id: SlabId,
  #[serde] headers: Vec<(ByteString, ByteString)>,
) -> Result<ResourceId, AnyError> {
  let mut http = slab_get(slab_id);
  // Stage 1: set the response to 101 Switching Protocols and send it
  let upgrade = http.upgrade()?;

  let response = http.response();
  *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
  for (name, value) in headers {
    response.headers_mut().append(
      HeaderName::from_bytes(&name).unwrap(),
      HeaderValue::from_bytes(&value).unwrap(),
    );
  }
  http.complete();

  // Stage 2: wait for the request to finish upgrading
  let upgraded = upgrade.await?;

  // Stage 3: take the extracted raw network stream and upgrade it to a websocket, then return it
  let (stream, bytes) = extract_network_stream(upgraded);
  ws_create_server_stream(&mut state.borrow_mut(), stream, bytes)
}

#[op2(fast)]
pub fn op_http_set_promise_complete(#[smi] slab_id: SlabId, status: u16) {
  let mut http = slab_get(slab_id);
  // The Javascript code should never provide a status that is invalid here (see 23_response.js), so we
  // will quitely ignore invalid values.
  if let Ok(code) = StatusCode::from_u16(status) {
    *http.response().status_mut() = code;
  }
  http.complete();
}

#[op(v8)]
pub fn op_http_get_request_method_and_url<'scope, HTTP>(
  scope: &mut v8::HandleScope<'scope>,
  slab_id: SlabId,
) -> serde_v8::Value<'scope>
where
  HTTP: HttpPropertyExtractor,
{
  let http = slab_get(slab_id);
  let request_info = http.request_info();
  let request_parts = http.request_parts();
  let request_properties = HTTP::request_properties(
    request_info,
    &request_parts.uri,
    &request_parts.headers,
  );

  let method: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    request_parts.method.as_str().as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();

  let authority: v8::Local<v8::Value> = match request_properties.authority {
    Some(authority) => v8::String::new_from_utf8(
      scope,
      authority.as_ref(),
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into(),
    None => v8::undefined(scope).into(),
  };

  // Only extract the path part - we handle authority elsewhere
  let path = match &request_parts.uri.path_and_query() {
    Some(path_and_query) => path_and_query.to_string(),
    None => "".to_owned(),
  };

  let path: v8::Local<v8::Value> =
    v8::String::new_from_utf8(scope, path.as_ref(), v8::NewStringType::Normal)
      .unwrap()
      .into();

  let peer_address: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    request_info.peer_address.as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();

  let port: v8::Local<v8::Value> = match request_info.peer_port {
    Some(port) => v8::Integer::new(scope, port.into()).into(),
    None => v8::undefined(scope).into(),
  };

  let vec = [method, authority, path, peer_address, port];
  let array = v8::Array::new_with_elements(scope, vec.as_slice());
  let array_value: v8::Local<v8::Value> = array.into();

  array_value.into()
}

#[op2]
#[serde]
pub fn op_http_get_request_header(
  #[smi] slab_id: SlabId,
  #[string] name: String,
) -> Option<ByteString> {
  let http = slab_get(slab_id);
  let value = http.request_parts().headers.get(name);
  value.map(|value| value.as_bytes().into())
}

#[op(v8)]
pub fn op_http_get_request_headers<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  slab_id: SlabId,
) -> serde_v8::Value<'scope> {
  let http = slab_get(slab_id);
  let headers = &http.request_parts().headers;
  // Two slots for each header key/value pair
  let mut vec: SmallVec<[v8::Local<v8::Value>; 32]> =
    SmallVec::with_capacity(headers.len() * 2);

  let mut cookies: Option<Vec<&[u8]>> = None;
  for (name, value) in headers {
    if name == COOKIE {
      if let Some(ref mut cookies) = cookies {
        cookies.push(value.as_bytes());
      } else {
        cookies = Some(vec![value.as_bytes()]);
      }
    } else {
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          name.as_ref(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          value.as_bytes(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
    }
  }

  // We treat cookies specially, because we don't want them to get them
  // mangled by the `Headers` object in JS. What we do is take all cookie
  // headers and concat them into a single cookie header, separated by
  // semicolons.
  // TODO(mmastrac): This should probably happen on the JS side on-demand
  if let Some(cookies) = cookies {
    let cookie_sep = "; ".as_bytes();

    vec.push(
      v8::String::new_external_onebyte_static(scope, COOKIE.as_ref())
        .unwrap()
        .into(),
    );
    vec.push(
      v8::String::new_from_one_byte(
        scope,
        cookies.join(cookie_sep).as_ref(),
        v8::NewStringType::Normal,
      )
      .unwrap()
      .into(),
    );
  }

  let array = v8::Array::new_with_elements(scope, vec.as_slice());
  let array_value: v8::Local<v8::Value> = array.into();

  array_value.into()
}

#[op(fast)]
pub fn op_http_read_request_body(
  state: Rc<RefCell<OpState>>,
  slab_id: SlabId,
) -> ResourceId {
  let mut http = slab_get(slab_id);
  let rid = if let Some(incoming) = http.take_body() {
    let body_resource = Rc::new(HttpRequestBody::new(incoming));
    state.borrow_mut().resource_table.add_rc(body_resource)
  } else {
    // This should not be possible, but rather than panicking we'll return an invalid
    // resource value to JavaScript.
    ResourceId::MAX
  };
  http.put_resource(HttpRequestBodyAutocloser::new(rid, state.clone()));
  rid
}

#[op2(fast)]
pub fn op_http_set_response_header(
  #[smi] slab_id: SlabId,
  #[string(onebyte)] name: Cow<[u8]>,
  #[string(onebyte)] value: Cow<[u8]>,
) {
  let mut http = slab_get(slab_id);
  let resp_headers = http.response().headers_mut();
  // These are valid latin-1 strings
  let name = HeaderName::from_bytes(&name).unwrap();
  let value = match value {
    Cow::Borrowed(bytes) => HeaderValue::from_bytes(bytes).unwrap(),
    // SAFETY: These are valid latin-1 strings
    Cow::Owned(bytes_vec) => unsafe {
      HeaderValue::from_maybe_shared_unchecked(bytes::Bytes::from(bytes_vec))
    },
  };
  resp_headers.append(name, value);
}

#[op2]
pub fn op_http_set_response_headers(
  scope: &mut v8::HandleScope,
  #[smi] slab_id: SlabId,
  headers: v8::Local<v8::Array>,
) {
  let mut http = slab_get(slab_id);
  // TODO(mmastrac): Invalid headers should be handled?
  let resp_headers = http.response().headers_mut();

  let len = headers.length();
  let header_len = len * 2;
  resp_headers.reserve(header_len.try_into().unwrap());

  for i in 0..len {
    let item = headers.get_index(scope, i).unwrap();
    let pair = v8::Local::<v8::Array>::try_from(item).unwrap();
    let name = pair.get_index(scope, 0).unwrap();
    let value = pair.get_index(scope, 1).unwrap();

    let v8_name: ByteString = from_v8(scope, name).unwrap();
    let v8_value: ByteString = from_v8(scope, value).unwrap();
    let header_name = HeaderName::from_bytes(&v8_name).unwrap();
    let header_value =
      // SAFETY: These are valid latin-1 strings
      unsafe { HeaderValue::from_maybe_shared_unchecked(v8_value) };
    resp_headers.append(header_name, header_value);
  }
}

#[op2]
pub fn op_http_set_response_trailers(
  #[smi] slab_id: SlabId,
  #[serde] trailers: Vec<(ByteString, ByteString)>,
) {
  let mut http = slab_get(slab_id);
  let mut trailer_map: HeaderMap = HeaderMap::with_capacity(trailers.len());
  for (name, value) in trailers {
    // These are valid latin-1 strings
    let name = HeaderName::from_bytes(&name).unwrap();
    // SAFETY: These are valid latin-1 strings
    let value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
    trailer_map.append(name, value);
  }
  *http.trailers().borrow_mut() = Some(trailer_map);
}

fn is_request_compressible(headers: &HeaderMap) -> Compression {
  let Some(accept_encoding) = headers.get(ACCEPT_ENCODING) else {
    return Compression::None;
  };

  match accept_encoding.to_str().unwrap() {
    // Firefox and Chrome send this -- no need to parse
    "gzip, deflate, br" => return Compression::Brotli,
    "gzip" => return Compression::GZip,
    "br" => return Compression::Brotli,
    _ => (),
  }

  // Fall back to the expensive parser
  let accepted = fly_accept_encoding::encodings_iter(headers).filter(|r| {
    matches!(
      r,
      Ok((
        Some(Encoding::Identity | Encoding::Gzip | Encoding::Brotli),
        _
      ))
    )
  });
  match fly_accept_encoding::preferred(accepted) {
    Ok(Some(fly_accept_encoding::Encoding::Gzip)) => Compression::GZip,
    Ok(Some(fly_accept_encoding::Encoding::Brotli)) => Compression::Brotli,
    _ => Compression::None,
  }
}

fn is_response_compressible(headers: &HeaderMap) -> bool {
  if let Some(content_type) = headers.get(CONTENT_TYPE) {
    if !is_content_compressible(content_type) {
      return false;
    }
  } else {
    return false;
  }
  if headers.contains_key(CONTENT_ENCODING) {
    return false;
  }
  if headers.contains_key(CONTENT_RANGE) {
    return false;
  }
  if let Some(cache_control) = headers.get(CACHE_CONTROL) {
    if let Ok(s) = std::str::from_utf8(cache_control.as_bytes()) {
      if let Some(cache_control) = CacheControl::from_value(s) {
        if cache_control.no_transform {
          return false;
        }
      }
    }
  }
  true
}

fn modify_compressibility_from_response(
  compression: Compression,
  length: Option<usize>,
  headers: &mut HeaderMap,
) -> Compression {
  ensure_vary_accept_encoding(headers);
  if let Some(length) = length {
    // By the time we add compression headers and Accept-Encoding, it probably doesn't make sense
    // to compress stuff that's smaller than this.
    if length < 64 {
      return Compression::None;
    }
  }
  if compression == Compression::None {
    return Compression::None;
  }
  if !is_response_compressible(headers) {
    return Compression::None;
  }
  let encoding = match compression {
    Compression::Brotli => "br",
    Compression::GZip => "gzip",
    _ => unreachable!(),
  };
  weaken_etag(headers);
  headers.remove(CONTENT_LENGTH);
  headers.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
  compression
}

/// If the user provided a ETag header for uncompressed data, we need to ensure it is a
/// weak Etag header ("W/").
fn weaken_etag(hmap: &mut HeaderMap) {
  if let Some(etag) = hmap.get_mut(hyper::header::ETAG) {
    if !etag.as_bytes().starts_with(b"W/") {
      let mut v = Vec::with_capacity(etag.as_bytes().len() + 2);
      v.extend(b"W/");
      v.extend(etag.as_bytes());
      *etag = v.try_into().unwrap();
    }
  }
}

// Set Vary: Accept-Encoding header for direct body response.
// Note: we set the header irrespective of whether or not we compress the data
// to make sure cache services do not serve uncompressed data to clients that
// support compression.
fn ensure_vary_accept_encoding(hmap: &mut HeaderMap) {
  if let Some(v) = hmap.get_mut(hyper::header::VARY) {
    if let Ok(s) = v.to_str() {
      if !s.to_lowercase().contains("accept-encoding") {
        *v = format!("Accept-Encoding, {s}").try_into().unwrap()
      }
      return;
    }
  }
  hmap.insert(
    hyper::header::VARY,
    HeaderValue::from_static("Accept-Encoding"),
  );
}

fn set_response(
  slab_id: SlabId,
  length: Option<usize>,
  status: u16,
  response_fn: impl FnOnce(Compression) -> ResponseBytesInner,
) {
  let mut http = slab_get(slab_id);
  // The request may have been cancelled by this point and if so, there's no need for us to
  // do all of this work to send the response.
  if !http.cancelled() {
    let resource = http.take_resource();
    let compression = is_request_compressible(&http.request_parts().headers);
    let response = http.response();
    let compression = modify_compressibility_from_response(
      compression,
      length,
      response.headers_mut(),
    );
    response
      .body_mut()
      .initialize(response_fn(compression), resource);

    // The Javascript code should never provide a status that is invalid here (see 23_response.js), so we
    // will quitely ignore invalid values.
    if let Ok(code) = StatusCode::from_u16(status) {
      *response.status_mut() = code;
    }
  }
  http.complete();
}

#[op2(fast)]
pub fn op_http_set_response_body_resource(
  state: Rc<RefCell<OpState>>,
  #[smi] slab_id: SlabId,
  #[smi] stream_rid: ResourceId,
  auto_close: bool,
  status: u16,
) -> Result<(), AnyError> {
  // IMPORTANT: We might end up requiring the OpState lock in set_response if we need to drop the request
  // body resource so we _cannot_ hold the OpState lock longer than necessary.

  // If the stream is auto_close, we will hold the last ref to it until the response is complete.
  // TODO(mmastrac): We should be using the same auto-close functionality rather than removing autoclose resources.
  // It's possible things could fail elsewhere if code expects the rid to continue existing after the response has been
  // returned.
  let resource = {
    let mut state = state.borrow_mut();
    if auto_close {
      state.resource_table.take_any(stream_rid)?
    } else {
      state.resource_table.get_any(stream_rid)?
    }
  };

  set_response(
    slab_id,
    resource.size_hint().1.map(|s| s as usize),
    status,
    move |compression| {
      ResponseBytesInner::from_resource(compression, resource, auto_close)
    },
  );

  Ok(())
}

#[op2(fast)]
pub fn op_http_set_response_body_text(
  #[smi] slab_id: SlabId,
  #[string] text: String,
  status: u16,
) {
  if !text.is_empty() {
    set_response(slab_id, Some(text.len()), status, |compression| {
      ResponseBytesInner::from_vec(compression, text.into_bytes())
    });
  } else {
    op_http_set_promise_complete::call(slab_id, status);
  }
}

// Skipping `fast` because we prefer an owned buffer here.
#[op2]
pub fn op_http_set_response_body_bytes(
  #[smi] slab_id: SlabId,
  #[buffer] buffer: JsBuffer,
  status: u16,
) {
  if !buffer.is_empty() {
    set_response(slab_id, Some(buffer.len()), status, |compression| {
      ResponseBytesInner::from_bufview(compression, BufView::from(buffer))
    });
  } else {
    op_http_set_promise_complete::call(slab_id, status);
  }
}

#[op2(async)]
pub async fn op_http_track(
  state: Rc<RefCell<OpState>>,
  #[smi] slab_id: SlabId,
  #[smi] server_rid: ResourceId,
) -> Result<(), AnyError> {
  let http = slab_get(slab_id);
  let handle = http.body_promise();

  let join_handle = state
    .borrow_mut()
    .resource_table
    .get::<HttpJoinHandle>(server_rid)?;

  match handle
    .or_cancel(join_handle.connection_cancel_handle())
    .await
  {
    Ok(true) => Ok(()),
    Ok(false) => {
      Err(AnyError::msg("connection closed before message completed"))
    }
    Err(_e) => Ok(()),
  }
}

#[pin_project(PinnedDrop)]
pub struct SlabFuture<F: Future<Output = ()>>(SlabId, #[pin] F);

pub fn new_slab_future(
  request: Request,
  request_info: HttpConnectionProperties,
  refcount: RefCount,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> SlabFuture<impl Future<Output = ()>> {
  let index = slab_insert(request, request_info, refcount);
  let rx = slab_get(index).promise();
  SlabFuture(index, async move {
    if tx.send(index).await.is_ok() {
      // We only need to wait for completion if we aren't closed
      rx.await;
    }
  })
}

impl<F: Future<Output = ()>> SlabFuture<F> {}

#[pinned_drop]
impl<F: Future<Output = ()>> PinnedDrop for SlabFuture<F> {
  fn drop(self: Pin<&mut Self>) {
    slab_drop(self.0);
  }
}

impl<F: Future<Output = ()>> Future for SlabFuture<F> {
  type Output = Result<Response, hyper::Error>;

  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let index = self.0;
    self
      .project()
      .1
      .poll(cx)
      .map(|_| Ok(slab_get(index).take_response()))
  }
}

fn serve_http11_unconditional(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = ResponseBytes> + 'static,
  cancel: Rc<CancelHandle>,
) -> impl Future<Output = Result<(), AnyError>> + 'static {
  let mut conn = http1::Builder::new()
    .keep_alive(true)
    .writev(*USE_WRITEV)
    .serve_connection(TokioIo::new(io), svc)
    .with_upgrades();

  poll_fn(move |cx| {
    if cancel.is_canceled() {
      println!("cancel!");
      // Should be safe to call repeatedly
      Pin::new(&mut conn).graceful_shutdown();
    }
    conn.try_poll_unpin(cx)
  })
  .map_err(AnyError::from)
}

fn serve_http2_unconditional(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = ResponseBytes> + 'static,
  cancel: Rc<CancelHandle>,
) -> impl Future<Output = Result<(), AnyError>> + 'static {
  let mut conn =
    http2::Builder::new(LocalExecutor).serve_connection(TokioIo::new(io), svc);
  poll_fn(move |cx| {
    if cancel.is_canceled() {
      // Should be safe to call repeatedly
      Pin::new(&mut conn).graceful_shutdown();
    }
    conn.try_poll_unpin(cx)
  })
  .map_err(AnyError::from)
}

async fn serve_http2_autodetect(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = ResponseBytes> + 'static,
  cancel: Rc<CancelHandle>,
) -> Result<(), AnyError> {
  let prefix = NetworkStreamPrefixCheck::new(io, HTTP2_PREFIX);
  let (matches, io) = prefix.match_prefix().await?;
  if matches {
    serve_http2_unconditional(io, svc, cancel).await
  } else {
    serve_http11_unconditional(io, svc, cancel).await
  }
}

fn serve_https(
  mut io: TlsStream,
  request_info: HttpConnectionProperties,
  lifetime: HttpLifetime,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> JoinHandle<Result<(), AnyError>> {
  let HttpLifetime {
    refcount,
    connection_cancel_handle,
    listen_cancel_handle,
  } = lifetime;

  let svc = service_fn(move |req: Request| {
    new_slab_future(req, request_info.clone(), refcount.clone(), tx.clone())
  });
  spawn(
    async {
      io.handshake().await?;
      // If the client specifically negotiates a protocol, we will use it. If not, we'll auto-detect
      // based on the prefix bytes
      let handshake = io.get_ref().1.alpn_protocol();
      if handshake == Some(TLS_ALPN_HTTP_2) {
        serve_http2_unconditional(io, svc, listen_cancel_handle).await
      } else if handshake == Some(TLS_ALPN_HTTP_11) {
        serve_http11_unconditional(io, svc, listen_cancel_handle).await
      } else {
        serve_http2_autodetect(io, svc, listen_cancel_handle).await
      }
    }
    .try_or_cancel(connection_cancel_handle),
  )
}

fn serve_http(
  io: impl HttpServeStream,
  request_info: HttpConnectionProperties,
  lifetime: HttpLifetime,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> JoinHandle<Result<(), AnyError>> {
  let HttpLifetime {
    refcount,
    connection_cancel_handle,
    listen_cancel_handle,
  } = lifetime;

  let svc = service_fn(move |req: Request| {
    new_slab_future(req, request_info.clone(), refcount.clone(), tx.clone())
  });
  spawn(
    serve_http2_autodetect(io, svc, listen_cancel_handle)
      .try_or_cancel(connection_cancel_handle),
  )
}

fn serve_http_on<HTTP>(
  connection: HTTP::Connection,
  listen_properties: &HttpListenProperties,
  lifetime: HttpLifetime,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> JoinHandle<Result<(), AnyError>>
where
  HTTP: HttpPropertyExtractor,
{
  let connection_properties: HttpConnectionProperties =
    HTTP::connection_properties(listen_properties, &connection);

  let network_stream = HTTP::to_network_stream_from_connection(connection);

  match network_stream {
    NetworkStream::Tcp(conn) => {
      serve_http(conn, connection_properties, lifetime, tx)
    }
    NetworkStream::Tls(conn) => {
      serve_https(conn, connection_properties, lifetime, tx)
    }
    #[cfg(unix)]
    NetworkStream::Unix(conn) => {
      serve_http(conn, connection_properties, lifetime, tx)
    }
  }
}

#[derive(Clone)]
struct HttpLifetime {
  connection_cancel_handle: Rc<CancelHandle>,
  listen_cancel_handle: Rc<CancelHandle>,
  refcount: RefCount,
}

struct HttpJoinHandle {
  join_handle: AsyncRefCell<Option<JoinHandle<Result<(), AnyError>>>>,
  connection_cancel_handle: Rc<CancelHandle>,
  listen_cancel_handle: Rc<CancelHandle>,
  rx: AsyncRefCell<tokio::sync::mpsc::Receiver<SlabId>>,
  refcount: RefCount,
}

impl HttpJoinHandle {
  fn new(rx: tokio::sync::mpsc::Receiver<SlabId>) -> Self {
    Self {
      join_handle: AsyncRefCell::new(None),
      connection_cancel_handle: CancelHandle::new_rc(),
      listen_cancel_handle: CancelHandle::new_rc(),
      rx: AsyncRefCell::new(rx),
      refcount: RefCount::default(),
    }
  }

  fn lifetime(self: &Rc<Self>) -> HttpLifetime {
    HttpLifetime {
      connection_cancel_handle: self.connection_cancel_handle.clone(),
      listen_cancel_handle: self.listen_cancel_handle.clone(),
      refcount: self.refcount.clone(),
    }
  }

  fn connection_cancel_handle(self: &Rc<Self>) -> Rc<CancelHandle> {
    self.connection_cancel_handle.clone()
  }

  fn listen_cancel_handle(self: &Rc<Self>) -> Rc<CancelHandle> {
    self.listen_cancel_handle.clone()
  }
}

impl Resource for HttpJoinHandle {
  fn name(&self) -> Cow<str> {
    "http".into()
  }

  fn close(self: Rc<Self>) {
    // During a close operation, we cancel everything
    self.connection_cancel_handle.cancel();
    self.listen_cancel_handle.cancel();
  }
}

impl Drop for HttpJoinHandle {
  fn drop(&mut self) {
    // In some cases we may be dropped without closing, so let's cancel everything on the way out
    self.connection_cancel_handle.cancel();
    self.listen_cancel_handle.cancel();
  }
}

#[op2]
#[serde]
pub fn op_http_serve<HTTP>(
  state: Rc<RefCell<OpState>>,
  #[smi] listener_rid: ResourceId,
) -> Result<(ResourceId, &'static str, String), AnyError>
where
  HTTP: HttpPropertyExtractor,
{
  slab_init();

  let listener =
    HTTP::get_listener_for_rid(&mut state.borrow_mut(), listener_rid)?;

  let listen_properties = HTTP::listen_properties_from_listener(&listener)?;

  let (tx, rx) = tokio::sync::mpsc::channel(10);
  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle::new(rx));
  let listen_cancel_clone = resource.listen_cancel_handle();

  let lifetime = resource.lifetime();

  let listen_properties_clone: HttpListenProperties = listen_properties.clone();
  let handle = spawn(async move {
    loop {
      let conn = HTTP::accept_connection_from_listener(&listener)
        .try_or_cancel(listen_cancel_clone.clone())
        .await?;
      serve_http_on::<HTTP>(
        conn,
        &listen_properties_clone,
        lifetime.clone(),
        tx.clone(),
      );
    }
    #[allow(unreachable_code)]
    Ok::<_, AnyError>(())
  });

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.join_handle)
    .try_borrow_mut()
    .unwrap() = Some(handle);

  Ok((
    state.borrow_mut().resource_table.add_rc(resource),
    listen_properties.scheme,
    listen_properties.fallback_host,
  ))
}

#[op2]
#[serde]
pub fn op_http_serve_on<HTTP>(
  state: Rc<RefCell<OpState>>,
  #[smi] connection_rid: ResourceId,
) -> Result<(ResourceId, &'static str, String), AnyError>
where
  HTTP: HttpPropertyExtractor,
{
  slab_init();

  let connection =
    HTTP::get_connection_for_rid(&mut state.borrow_mut(), connection_rid)?;

  let listen_properties = HTTP::listen_properties_from_connection(&connection)?;

  let (tx, rx) = tokio::sync::mpsc::channel(10);
  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle::new(rx));

  let handle: JoinHandle<Result<(), deno_core::anyhow::Error>> =
    serve_http_on::<HTTP>(
      connection,
      &listen_properties,
      resource.lifetime(),
      tx,
    );

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.join_handle)
    .try_borrow_mut()
    .unwrap() = Some(handle);

  Ok((
    state.borrow_mut().resource_table.add_rc(resource),
    listen_properties.scheme,
    listen_properties.fallback_host,
  ))
}

/// Synchronous, non-blocking call to see if there are any further HTTP requests. If anything
/// goes wrong in this method we return [`SlabId::MAX`] and let the async handler pick up the real error.
#[op2(fast)]
#[smi]
pub fn op_http_try_wait(state: &mut OpState, #[smi] rid: ResourceId) -> SlabId {
  // The resource needs to exist.
  let Ok(join_handle) = state.resource_table.get::<HttpJoinHandle>(rid) else {
    return SlabId::MAX;
  };

  // If join handle is somehow locked, just abort.
  let Some(mut handle) =
    RcRef::map(&join_handle, |this| &this.rx).try_borrow_mut()
  else {
    return SlabId::MAX;
  };

  // See if there are any requests waiting on this channel. If not, return.
  let Ok(id) = handle.try_recv() else {
    return SlabId::MAX;
  };

  id
}

#[op2(async)]
#[smi]
pub async fn op_http_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<SlabId, AnyError> {
  // We will get the join handle initially, as we might be consuming requests still
  let join_handle = state
    .borrow_mut()
    .resource_table
    .get::<HttpJoinHandle>(rid)?;

  let cancel = join_handle.listen_cancel_handle();
  let next = async {
    let mut recv = RcRef::map(&join_handle, |this| &this.rx).borrow_mut().await;
    recv.recv().await
  }
  .or_cancel(cancel)
  .unwrap_or_else(|_| None)
  .await;

  // Do we have a request?
  if let Some(req) = next {
    return Ok(req);
  }

  // No - we're shutting down
  let res = RcRef::map(join_handle, |this| &this.join_handle)
    .borrow_mut()
    .await
    .take()
    .unwrap()
    .await?;

  // Filter out shutdown (ENOTCONN) errors
  if let Err(err) = res {
    if let Some(err) = err.source() {
      if let Some(err) = err.downcast_ref::<io::Error>() {
        if err.kind() == io::ErrorKind::NotConnected {
          return Ok(SlabId::MAX);
        }
      }
    }
    return Err(err);
  }

  Ok(SlabId::MAX)
}

/// Cancels the HTTP handle.
#[op2(fast)]
pub fn op_http_cancel(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  graceful: bool,
) -> Result<(), AnyError> {
  let join_handle = state.resource_table.get::<HttpJoinHandle>(rid)?;

  if graceful {
    // In a graceful shutdown, we close the listener and allow all the remaining connections to drain
    join_handle.listen_cancel_handle().cancel();
  } else {
    // In a forceful shutdown, we close everything
    join_handle.listen_cancel_handle().cancel();
    join_handle.connection_cancel_handle().cancel();
  }

  Ok(())
}

#[op2(async)]
pub async fn op_http_close(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  graceful: bool,
) -> Result<(), AnyError> {
  let join_handle = state
    .borrow_mut()
    .resource_table
    .take::<HttpJoinHandle>(rid)?;

  if graceful {
    // In a graceful shutdown, we close the listener and allow all the remaining connections to drain
    join_handle.listen_cancel_handle().cancel();
  } else {
    // In a forceful shutdown, we close everything
    join_handle.listen_cancel_handle().cancel();
    join_handle.connection_cancel_handle().cancel();
  }

  // Async spin on the refcount while we wait for everything to drain
  while Rc::strong_count(&join_handle.refcount.0) > 1 {
    println!("{}", Rc::strong_count(&join_handle.refcount.0));
    tokio::time::sleep(Duration::from_millis(10)).await;
  }

  let mut join_handle = RcRef::map(&join_handle, |this| &this.join_handle)
    .borrow_mut()
    .await;
  println!("wait");
  if let Some(join_handle) = join_handle.take() {
    join_handle.await??;
  }

  Ok(())
}

struct UpgradeStream {
  read: AsyncRefCell<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
  write: AsyncRefCell<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
  cancel_handle: CancelHandle,
}

impl UpgradeStream {
  pub fn new(
    read: tokio::io::ReadHalf<tokio::io::DuplexStream>,
    write: tokio::io::WriteHalf<tokio::io::DuplexStream>,
  ) -> Self {
    Self {
      read: AsyncRefCell::new(read),
      write: AsyncRefCell::new(write),
      cancel_handle: CancelHandle::new(),
    }
  }

  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    async {
      let read = RcRef::map(self, |this| &this.read);
      let mut read = read.borrow_mut().await;
      Ok(Pin::new(&mut *read).read(buf).await?)
    }
    .try_or_cancel(cancel_handle)
    .await
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    async {
      let write = RcRef::map(self, |this| &this.write);
      let mut write = write.borrow_mut().await;
      Ok(Pin::new(&mut *write).write(buf).await?)
    }
    .try_or_cancel(cancel_handle)
    .await
  }

  async fn write_vectored(
    self: Rc<Self>,
    buf1: &[u8],
    buf2: &[u8],
  ) -> Result<usize, AnyError> {
    let mut wr = RcRef::map(self, |r| &r.write).borrow_mut().await;

    let total = buf1.len() + buf2.len();
    let mut bufs = [std::io::IoSlice::new(buf1), std::io::IoSlice::new(buf2)];
    let mut nwritten = wr.write_vectored(&bufs).await?;
    if nwritten == total {
      return Ok(nwritten);
    }

    // Slightly more optimized than (unstable) write_all_vectored for 2 iovecs.
    while nwritten <= buf1.len() {
      bufs[0] = std::io::IoSlice::new(&buf1[nwritten..]);
      nwritten += wr.write_vectored(&bufs).await?;
    }

    // First buffer out of the way.
    if nwritten < total && nwritten > buf1.len() {
      wr.write_all(&buf2[nwritten - buf1.len()..]).await?;
    }

    Ok(total)
  }
}

impl Resource for UpgradeStream {
  fn name(&self) -> Cow<str> {
    "httpRawUpgradeStream".into()
  }

  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[op2(fast)]
pub fn op_can_write_vectored(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> bool {
  state.resource_table.get::<UpgradeStream>(rid).is_ok()
}

// TODO(bartlomieju): op2 doesn't want to handle `usize` in the return type
#[op]
pub async fn op_raw_write_vectored(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf1: JsBuffer,
  buf2: JsBuffer,
) -> Result<usize, AnyError> {
  let resource: Rc<UpgradeStream> =
    state.borrow().resource_table.get::<UpgradeStream>(rid)?;
  let nwritten = resource.write_vectored(&buf1, &buf2).await?;
  Ok(nwritten)
}
