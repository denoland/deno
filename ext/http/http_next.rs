// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::compressible::is_content_compressible;
use crate::extract_network_stream;
use crate::network_buffered_stream::NetworkStreamPrefixCheck;
use crate::request_body::HttpRequestBody;
use crate::request_properties::HttpConnectionProperties;
use crate::request_properties::HttpListenProperties;
use crate::request_properties::HttpPropertyExtractor;
use crate::response_body::Compression;
use crate::response_body::ResponseBytes;
use crate::response_body::ResponseBytesInner;
use crate::response_body::V8StreamHttpResponseBody;
use crate::slab::slab_drop;
use crate::slab::slab_get;
use crate::slab::slab_insert;
use crate::slab::SlabId;
use crate::websocket_upgrade::WebSocketUpgrade;
use crate::LocalExecutor;
use bytes::Bytes;
use cache_control::CacheControl;
use deno_core::error::AnyError;
use deno_core::futures::TryFutureExt;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::serde_v8::from_v8;
use deno_core::task::spawn;
use deno_core::task::JoinHandle;
use deno_core::v8;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
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

/// ALPN negotation for "h2"
const TLS_ALPN_HTTP_2: &[u8] = b"h2";

/// ALPN negotation for "http/1.1"
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

#[op]
pub fn op_http_upgrade_raw(
  state: &mut OpState,
  slab_id: SlabId,
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
          let mut upgraded = upgrade.await?;
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

#[op]
pub async fn op_http_upgrade_websocket_next(
  state: Rc<RefCell<OpState>>,
  slab_id: SlabId,
  headers: Vec<(ByteString, ByteString)>,
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

#[op(fast)]
pub fn op_http_set_promise_complete(slab_id: SlabId, status: u16) {
  let mut http = slab_get(slab_id);
  // The Javascript code will never provide a status that is invalid here (see 23_response.js)
  *http.response().status_mut() = StatusCode::from_u16(status).unwrap();
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

#[op]
pub fn op_http_get_request_header(
  slab_id: SlabId,
  name: String,
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
  state: &mut OpState,
  slab_id: SlabId,
) -> ResourceId {
  let mut http = slab_get(slab_id);
  let incoming = http.take_body();
  let body_resource = Rc::new(HttpRequestBody::new(incoming));
  state.resource_table.add_rc(body_resource)
}

#[op(fast)]
pub fn op_http_set_response_header(
  slab_id: SlabId,
  name: ByteString,
  value: ByteString,
) {
  let mut http = slab_get(slab_id);
  let resp_headers = http.response().headers_mut();
  // These are valid latin-1 strings
  let name = HeaderName::from_bytes(&name).unwrap();
  // SAFETY: These are valid latin-1 strings
  let value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
  resp_headers.append(name, value);
}

#[op(v8)]
fn op_http_set_response_headers(
  scope: &mut v8::HandleScope,
  slab_id: SlabId,
  headers: serde_v8::Value,
) {
  let mut http = slab_get(slab_id);
  // TODO(mmastrac): Invalid headers should be handled?
  let resp_headers = http.response().headers_mut();

  let arr = v8::Local::<v8::Array>::try_from(headers.v8_value).unwrap();

  let len = arr.length();
  let header_len = len * 2;
  resp_headers.reserve(header_len.try_into().unwrap());

  for i in 0..len {
    let item = arr.get_index(scope, i).unwrap();
    let pair = v8::Local::<v8::Array>::try_from(item).unwrap();
    let name = pair.get_index(scope, 0).unwrap();
    let value = pair.get_index(scope, 1).unwrap();

    let v8_name: ByteString = from_v8(scope, name).unwrap();
    let v8_value: ByteString = from_v8(scope, value).unwrap();
    let header_name = HeaderName::from_bytes(&v8_name).unwrap();
    // SAFETY: These are valid latin-1 strings
    let header_value =
      unsafe { HeaderValue::from_maybe_shared_unchecked(v8_value) };
    resp_headers.append(header_name, header_value);
  }
}

#[op]
pub fn op_http_set_response_trailers(
  slab_id: SlabId,
  trailers: Vec<(ByteString, ByteString)>,
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
  response_fn: impl FnOnce(Compression) -> ResponseBytesInner,
) {
  let mut http = slab_get(slab_id);
  let compression = is_request_compressible(&http.request_parts().headers);
  let response = http.response();
  let compression = modify_compressibility_from_response(
    compression,
    length,
    response.headers_mut(),
  );
  response.body_mut().initialize(response_fn(compression))
}

#[op(fast)]
pub fn op_http_set_response_body_resource(
  state: &mut OpState,
  slab_id: SlabId,
  stream_rid: ResourceId,
  auto_close: bool,
) -> Result<(), AnyError> {
  // If the stream is auto_close, we will hold the last ref to it until the response is complete.
  let resource = if auto_close {
    state.resource_table.take_any(stream_rid)?
  } else {
    state.resource_table.get_any(stream_rid)?
  };

  set_response(
    slab_id,
    resource.size_hint().1.map(|s| s as usize),
    move |compression| {
      ResponseBytesInner::from_resource(compression, resource, auto_close)
    },
  );

  Ok(())
}

#[op(fast)]
pub fn op_http_set_response_body_stream(
  state: &mut OpState,
  slab_id: SlabId,
) -> Result<ResourceId, AnyError> {
  // TODO(mmastrac): what should this channel size be?
  let (tx, rx) = tokio::sync::mpsc::channel(1);
  set_response(slab_id, None, |compression| {
    ResponseBytesInner::from_v8(compression, rx)
  });

  Ok(state.resource_table.add(V8StreamHttpResponseBody::new(tx)))
}

#[op(fast)]
pub fn op_http_set_response_body_text(slab_id: SlabId, text: String) {
  if !text.is_empty() {
    set_response(slab_id, Some(text.len()), |compression| {
      ResponseBytesInner::from_vec(compression, text.into_bytes())
    });
  }
}

#[op(fast)]
pub fn op_http_set_response_body_bytes(slab_id: SlabId, buffer: &[u8]) {
  if !buffer.is_empty() {
    set_response(slab_id, Some(buffer.len()), |compression| {
      ResponseBytesInner::from_slice(compression, buffer)
    });
  };
}

#[op]
pub async fn op_http_track(
  state: Rc<RefCell<OpState>>,
  slab_id: SlabId,
  server_rid: ResourceId,
) -> Result<(), AnyError> {
  let http = slab_get(slab_id);
  let handle = http.body_promise();

  let join_handle = state
    .borrow_mut()
    .resource_table
    .get::<HttpJoinHandle>(server_rid)?;

  match handle.or_cancel(join_handle.cancel_handle()).await {
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
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> SlabFuture<impl Future<Output = ()>> {
  let index = slab_insert(request, request_info);
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
) -> impl Future<Output = Result<(), AnyError>> + 'static {
  let conn = http1::Builder::new()
    .keep_alive(true)
    .writev(*USE_WRITEV)
    .serve_connection(io, svc);

  conn.with_upgrades().map_err(AnyError::from)
}

fn serve_http2_unconditional(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = ResponseBytes> + 'static,
) -> impl Future<Output = Result<(), AnyError>> + 'static {
  let conn = http2::Builder::new(LocalExecutor).serve_connection(io, svc);
  conn.map_err(AnyError::from)
}

async fn serve_http2_autodetect(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = ResponseBytes> + 'static,
) -> Result<(), AnyError> {
  let prefix = NetworkStreamPrefixCheck::new(io, HTTP2_PREFIX);
  let (matches, io) = prefix.match_prefix().await?;
  if matches {
    serve_http2_unconditional(io, svc).await
  } else {
    serve_http11_unconditional(io, svc).await
  }
}

fn serve_https(
  mut io: TlsStream,
  request_info: HttpConnectionProperties,
  cancel: Rc<CancelHandle>,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> JoinHandle<Result<(), AnyError>> {
  let svc = service_fn(move |req: Request| {
    new_slab_future(req, request_info.clone(), tx.clone())
  });
  spawn(
    async {
      io.handshake().await?;
      // If the client specifically negotiates a protocol, we will use it. If not, we'll auto-detect
      // based on the prefix bytes
      let handshake = io.get_ref().1.alpn_protocol();
      if handshake == Some(TLS_ALPN_HTTP_2) {
        serve_http2_unconditional(io, svc).await
      } else if handshake == Some(TLS_ALPN_HTTP_11) {
        serve_http11_unconditional(io, svc).await
      } else {
        serve_http2_autodetect(io, svc).await
      }
    }
    .try_or_cancel(cancel),
  )
}

fn serve_http(
  io: impl HttpServeStream,
  request_info: HttpConnectionProperties,
  cancel: Rc<CancelHandle>,
  tx: tokio::sync::mpsc::Sender<SlabId>,
) -> JoinHandle<Result<(), AnyError>> {
  let svc = service_fn(move |req: Request| {
    new_slab_future(req, request_info.clone(), tx.clone())
  });
  spawn(serve_http2_autodetect(io, svc).try_or_cancel(cancel))
}

fn serve_http_on<HTTP>(
  connection: HTTP::Connection,
  listen_properties: &HttpListenProperties,
  cancel: Rc<CancelHandle>,
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
      serve_http(conn, connection_properties, cancel, tx)
    }
    NetworkStream::Tls(conn) => {
      serve_https(conn, connection_properties, cancel, tx)
    }
    #[cfg(unix)]
    NetworkStream::Unix(conn) => {
      serve_http(conn, connection_properties, cancel, tx)
    }
  }
}

struct HttpJoinHandle(
  AsyncRefCell<Option<JoinHandle<Result<(), AnyError>>>>,
  // Cancel handle must live in a separate Rc to avoid keeping the outer join handle ref'd
  Rc<CancelHandle>,
  AsyncRefCell<tokio::sync::mpsc::Receiver<SlabId>>,
);

impl HttpJoinHandle {
  fn cancel_handle(self: &Rc<Self>) -> Rc<CancelHandle> {
    self.1.clone()
  }
}

impl Resource for HttpJoinHandle {
  fn name(&self) -> Cow<str> {
    "http".into()
  }

  fn close(self: Rc<Self>) {
    self.1.cancel()
  }
}

impl Drop for HttpJoinHandle {
  fn drop(&mut self) {
    // In some cases we may be dropped without closing, so let's cancel everything on the way out
    self.1.cancel();
  }
}

#[op(v8)]
pub fn op_http_serve<HTTP>(
  state: Rc<RefCell<OpState>>,
  listener_rid: ResourceId,
) -> Result<(ResourceId, &'static str, String), AnyError>
where
  HTTP: HttpPropertyExtractor,
{
  let listener =
    HTTP::get_listener_for_rid(&mut state.borrow_mut(), listener_rid)?;

  let listen_properties = HTTP::listen_properties_from_listener(&listener)?;

  let (tx, rx) = tokio::sync::mpsc::channel(10);
  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle(
    AsyncRefCell::new(None),
    CancelHandle::new_rc(),
    AsyncRefCell::new(rx),
  ));
  let cancel_clone = resource.cancel_handle();

  let listen_properties_clone: HttpListenProperties = listen_properties.clone();
  let handle = spawn(async move {
    loop {
      let conn = HTTP::accept_connection_from_listener(&listener)
        .try_or_cancel(cancel_clone.clone())
        .await?;
      serve_http_on::<HTTP>(
        conn,
        &listen_properties_clone,
        cancel_clone.clone(),
        tx.clone(),
      );
    }
    #[allow(unreachable_code)]
    Ok::<_, AnyError>(())
  });

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.0)
    .try_borrow_mut()
    .unwrap() = Some(handle);

  Ok((
    state.borrow_mut().resource_table.add_rc(resource),
    listen_properties.scheme,
    listen_properties.fallback_host,
  ))
}

#[op(v8)]
pub fn op_http_serve_on<HTTP>(
  state: Rc<RefCell<OpState>>,
  connection_rid: ResourceId,
) -> Result<(ResourceId, &'static str, String), AnyError>
where
  HTTP: HttpPropertyExtractor,
{
  let connection =
    HTTP::get_connection_for_rid(&mut state.borrow_mut(), connection_rid)?;

  let listen_properties = HTTP::listen_properties_from_connection(&connection)?;

  let (tx, rx) = tokio::sync::mpsc::channel(10);
  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle(
    AsyncRefCell::new(None),
    CancelHandle::new_rc(),
    AsyncRefCell::new(rx),
  ));

  let handle: JoinHandle<Result<(), deno_core::anyhow::Error>> =
    serve_http_on::<HTTP>(
      connection,
      &listen_properties,
      resource.cancel_handle(),
      tx,
    );

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.0)
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
#[op(fast)]
pub fn op_http_try_wait(state: &mut OpState, rid: ResourceId) -> SlabId {
  // The resource needs to exist.
  let Ok(join_handle) = state
    .resource_table
    .get::<HttpJoinHandle>(rid) else {
      return SlabId::MAX;
  };

  // If join handle is somehow locked, just abort.
  let Some(mut handle) = RcRef::map(&join_handle, |this| &this.2).try_borrow_mut() else {
    return SlabId::MAX;
  };

  // See if there are any requests waiting on this channel. If not, return.
  let Ok(id) = handle.try_recv() else {
    return SlabId::MAX;
  };

  id
}

#[op]
pub async fn op_http_wait(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<SlabId, AnyError> {
  // We will get the join handle initially, as we might be consuming requests still
  let join_handle = state
    .borrow_mut()
    .resource_table
    .get::<HttpJoinHandle>(rid)?;

  let cancel = join_handle.cancel_handle();
  let next = async {
    let mut recv = RcRef::map(&join_handle, |this| &this.2).borrow_mut().await;
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
  let res = RcRef::map(join_handle, |this| &this.0)
    .borrow_mut()
    .await
    .take()
    .unwrap()
    .await?;

  // Drop the cancel and join handles
  state
    .borrow_mut()
    .resource_table
    .take::<HttpJoinHandle>(rid)?;

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
