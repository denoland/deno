// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use async_compression::tokio::write::BrotliEncoder;
use async_compression::tokio::write::GzipEncoder;
use cache_control::CacheControl;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::channel::oneshot;
use deno_core::futures::future::pending;
use deno_core::futures::future::select;
use deno_core::futures::future::Either;
use deno_core::futures::future::Pending;
use deno_core::futures::future::RemoteHandle;
use deno_core::futures::future::Shared;
use deno_core::futures::never::Never;
use deno_core::futures::pin_mut;
use deno_core::futures::ready;
use deno_core::futures::stream::Peekable;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::futures::TryFutureExt;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use deno_websocket::ws_create_server_stream;
use flate2::write::GzEncoder;
use flate2::Compression;
use fly_accept_encoding::Encoding;
use hyper::body::Bytes;
use hyper::body::HttpBody;
use hyper::body::SizeHint;
use hyper::header::HeaderName;
use hyper::header::HeaderValue;
use hyper::server::conn::Http;
use hyper::service::Service;
use hyper::Body;
use hyper::HeaderMap;
use hyper::Request;
use hyper::Response;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::min;
use std::error::Error;
use std::future::Future;
use std::io;
use std::io::Write;
use std::mem::replace;
use std::mem::take;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_local;

use crate::reader_stream::ExternallyAbortableReaderStream;
use crate::reader_stream::ShutdownHandle;

pub mod compressible;
mod reader_stream;

pub fn init() -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec!["deno_web", "deno_net", "deno_fetch", "deno_websocket"])
    .js(include_js_files!(
      prefix "deno:ext/http",
      "01_http.js",
    ))
    .ops(vec![
      op_http_accept::decl(),
      op_http_write_headers::decl(),
      op_http_headers::decl(),
      op_http_write::decl(),
      op_http_write_resource::decl(),
      op_http_shutdown::decl(),
      op_http_websocket_accept_header::decl(),
      op_http_upgrade_websocket::decl(),
    ])
    .build()
}

pub enum HttpSocketAddr {
  IpSocket(std::net::SocketAddr),
  #[cfg(unix)]
  UnixSocket(tokio::net::unix::SocketAddr),
}

impl From<std::net::SocketAddr> for HttpSocketAddr {
  fn from(addr: std::net::SocketAddr) -> Self {
    Self::IpSocket(addr)
  }
}

#[cfg(unix)]
impl From<tokio::net::unix::SocketAddr> for HttpSocketAddr {
  fn from(addr: tokio::net::unix::SocketAddr) -> Self {
    Self::UnixSocket(addr)
  }
}

struct HttpConnResource {
  addr: HttpSocketAddr,
  scheme: &'static str,
  acceptors_tx: mpsc::UnboundedSender<HttpAcceptor>,
  closed_fut: Shared<RemoteHandle<Result<(), Arc<hyper::Error>>>>,
  cancel_handle: Rc<CancelHandle>, // Closes gracefully and cancels accept ops.
}

impl HttpConnResource {
  fn new<S>(io: S, scheme: &'static str, addr: HttpSocketAddr) -> Self
  where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
  {
    let (acceptors_tx, acceptors_rx) = mpsc::unbounded::<HttpAcceptor>();
    let service = HttpService::new(acceptors_rx);

    let conn_fut = Http::new()
      .with_executor(LocalExecutor)
      .serve_connection(io, service)
      .with_upgrades();

    // When the cancel handle is used, the connection shuts down gracefully.
    // No new HTTP streams will be accepted, but existing streams will be able
    // to continue operating and eventually shut down cleanly.
    let cancel_handle = CancelHandle::new_rc();
    let shutdown_fut = never().or_cancel(&cancel_handle).fuse();

    // A local task that polls the hyper connection future to completion.
    let task_fut = async move {
      pin_mut!(shutdown_fut);
      pin_mut!(conn_fut);
      let result = match select(conn_fut, shutdown_fut).await {
        Either::Left((result, _)) => result,
        Either::Right((_, mut conn_fut)) => {
          conn_fut.as_mut().graceful_shutdown();
          conn_fut.await
        }
      };
      filter_enotconn(result).map_err(Arc::from)
    };
    let (task_fut, closed_fut) = task_fut.remote_handle();
    let closed_fut = closed_fut.shared();
    spawn_local(task_fut);

    Self {
      addr,
      scheme,
      acceptors_tx,
      closed_fut,
      cancel_handle,
    }
  }

  // Accepts a new incoming HTTP request.
  async fn accept(
    self: &Rc<Self>,
  ) -> Result<Option<(HttpStreamResource, String, String)>, AnyError> {
    let fut = async {
      let (request_tx, request_rx) = oneshot::channel();
      let (response_tx, response_rx) = oneshot::channel();

      let acceptor = HttpAcceptor::new(request_tx, response_rx);
      self.acceptors_tx.unbounded_send(acceptor).ok()?;

      let request = request_rx.await.ok()?;

      let accept_encoding = {
        let encodings = fly_accept_encoding::encodings_iter(request.headers())
          .filter(|r| {
            matches!(r, Ok((Some(Encoding::Brotli | Encoding::Gzip), _)))
          });

        fly_accept_encoding::preferred(encodings)
          .ok()
          .flatten()
          .unwrap_or(Encoding::Identity)
      };

      let method = request.method().to_string();
      let url = req_url(&request, self.scheme, &self.addr);
      let stream =
        HttpStreamResource::new(self, request, response_tx, accept_encoding);
      Some((stream, method, url))
    };

    async {
      match fut.await {
        Some(stream) => Ok(Some(stream)),
        // Return the connection error, if any.
        None => self.closed().map_ok(|_| None).await,
      }
    }
    .try_or_cancel(&self.cancel_handle)
    .await
  }

  /// A future that completes when this HTTP connection is closed or errors.
  async fn closed(&self) -> Result<(), AnyError> {
    self.closed_fut.clone().map_err(AnyError::from).await
  }
}

impl Resource for HttpConnResource {
  fn name(&self) -> Cow<str> {
    "httpConn".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

/// Creates a new HttpConn resource which uses `io` as its transport.
pub fn http_create_conn_resource<S, A>(
  state: &mut OpState,
  io: S,
  addr: A,
  scheme: &'static str,
) -> Result<ResourceId, AnyError>
where
  S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
  A: Into<HttpSocketAddr>,
{
  let conn = HttpConnResource::new(io, scheme, addr.into());
  let rid = state.resource_table.add(conn);
  Ok(rid)
}

/// An object that implements the `hyper::Service` trait, through which Hyper
/// delivers incoming HTTP requests.
struct HttpService {
  acceptors_rx: Peekable<mpsc::UnboundedReceiver<HttpAcceptor>>,
}

impl HttpService {
  fn new(acceptors_rx: mpsc::UnboundedReceiver<HttpAcceptor>) -> Self {
    let acceptors_rx = acceptors_rx.peekable();
    Self { acceptors_rx }
  }
}

impl Service<Request<Body>> for HttpService {
  type Response = Response<Body>;
  type Error = oneshot::Canceled;
  type Future = oneshot::Receiver<Response<Body>>;

  fn poll_ready(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), Self::Error>> {
    let acceptors_rx = Pin::new(&mut self.acceptors_rx);
    let result = ready!(acceptors_rx.poll_peek(cx))
      .map(|_| ())
      .ok_or(oneshot::Canceled);
    Poll::Ready(result)
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let acceptor = self.acceptors_rx.next().now_or_never().flatten().unwrap();
    acceptor.call(request)
  }
}

/// A pair of one-shot channels which first transfer a HTTP request from the
/// Hyper service to the HttpConn resource, and then take the Response back to
/// the service.
struct HttpAcceptor {
  request_tx: oneshot::Sender<Request<Body>>,
  response_rx: oneshot::Receiver<Response<Body>>,
}

impl HttpAcceptor {
  fn new(
    request_tx: oneshot::Sender<Request<Body>>,
    response_rx: oneshot::Receiver<Response<Body>>,
  ) -> Self {
    Self {
      request_tx,
      response_rx,
    }
  }

  fn call(self, request: Request<Body>) -> oneshot::Receiver<Response<Body>> {
    let Self {
      request_tx,
      response_rx,
    } = self;
    request_tx
      .send(request)
      .map(|_| response_rx)
      .unwrap_or_else(|_| oneshot::channel().1) // Make new canceled receiver.
  }
}

/// A resource representing a single HTTP request/response stream.
pub struct HttpStreamResource {
  conn: Rc<HttpConnResource>,
  pub rd: AsyncRefCell<HttpRequestReader>,
  wr: AsyncRefCell<HttpResponseWriter>,
  accept_encoding: Encoding,
  cancel_handle: CancelHandle,
  size: SizeHint,
}

impl HttpStreamResource {
  fn new(
    conn: &Rc<HttpConnResource>,
    request: Request<Body>,
    response_tx: oneshot::Sender<Response<Body>>,
    accept_encoding: Encoding,
  ) -> Self {
    let size = request.body().size_hint();
    Self {
      conn: conn.clone(),
      rd: HttpRequestReader::Headers(request).into(),
      wr: HttpResponseWriter::Headers(response_tx).into(),
      accept_encoding,
      size,
      cancel_handle: CancelHandle::new(),
    }
  }
}

impl Resource for HttpStreamResource {
  fn name(&self) -> Cow<str> {
    "httpStream".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;

      let body = loop {
        match &mut *rd {
          HttpRequestReader::Headers(_) => {}
          HttpRequestReader::Body(_, body) => break body,
          HttpRequestReader::Closed => return Ok(BufView::empty()),
        }
        match take(&mut *rd) {
          HttpRequestReader::Headers(request) => {
            let (parts, body) = request.into_parts();
            *rd = HttpRequestReader::Body(parts.headers, body.peekable());
          }
          _ => unreachable!(),
        };
      };

      let fut = async {
        let mut body = Pin::new(body);
        loop {
          match body.as_mut().peek_mut().await {
            Some(Ok(chunk)) if !chunk.is_empty() => {
              let len = min(limit, chunk.len());
              let buf = chunk.split_to(len);
              let view = BufView::from(buf);
              break Ok(view);
            }
            // This unwrap is safe because `peek_mut()` returned `Some`, and thus
            // currently has a peeked value that can be synchronously returned
            // from `next()`.
            //
            // The future returned from `next()` is always ready, so we can
            // safely call `await` on it without creating a race condition.
            Some(_) => match body.as_mut().next().await.unwrap() {
              Ok(chunk) => assert!(chunk.is_empty()),
              Err(err) => break Err(AnyError::from(err)),
            },
            None => break Ok(BufView::empty()),
          }
        }
      };

      let cancel_handle = RcRef::map(&self, |r| &r.cancel_handle);
      fut.try_or_cancel(cancel_handle).await
    })
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (self.size.lower(), self.size.upper())
  }
}

/// The read half of an HTTP stream.
pub enum HttpRequestReader {
  Headers(Request<Body>),
  Body(HeaderMap<HeaderValue>, Peekable<Body>),
  Closed,
}

impl Default for HttpRequestReader {
  fn default() -> Self {
    Self::Closed
  }
}

/// The write half of an HTTP stream.
enum HttpResponseWriter {
  Headers(oneshot::Sender<Response<Body>>),
  Body {
    writer: Pin<Box<dyn tokio::io::AsyncWrite>>,
    shutdown_handle: ShutdownHandle,
  },
  BodyUncompressed(BodyUncompressedSender),
  Closed,
}

impl Default for HttpResponseWriter {
  fn default() -> Self {
    Self::Closed
  }
}

struct BodyUncompressedSender(Option<hyper::body::Sender>);

impl BodyUncompressedSender {
  fn sender(&mut self) -> &mut hyper::body::Sender {
    // This is safe because we only ever take the sender out of the option
    // inside of the shutdown method.
    self.0.as_mut().unwrap()
  }

  fn shutdown(mut self) {
    // take the sender out of self so that when self is dropped at the end of
    // this block, it doesn't get aborted
    self.0.take();
  }
}

impl From<hyper::body::Sender> for BodyUncompressedSender {
  fn from(sender: hyper::body::Sender) -> Self {
    BodyUncompressedSender(Some(sender))
  }
}

impl Drop for BodyUncompressedSender {
  fn drop(&mut self) {
    if let Some(sender) = self.0.take() {
      sender.abort();
    }
  }
}

// We use a tuple instead of struct to avoid serialization overhead of the keys.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NextRequestResponse(
  // stream_rid:
  ResourceId,
  // method:
  // This is a String rather than a ByteString because reqwest will only return
  // the method as a str which is guaranteed to be ASCII-only.
  String,
  // url:
  String,
);

#[op]
async fn op_http_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<Option<NextRequestResponse>, AnyError> {
  let conn = state.borrow().resource_table.get::<HttpConnResource>(rid)?;

  match conn.accept().await {
    Ok(Some((stream, method, url))) => {
      let stream_rid =
        state.borrow_mut().resource_table.add_rc(Rc::new(stream));
      let r = NextRequestResponse(stream_rid, method, url);
      Ok(Some(r))
    }
    Ok(None) => Ok(None),
    Err(err) => Err(err),
  }
}

fn req_url(
  req: &hyper::Request<hyper::Body>,
  scheme: &'static str,
  addr: &HttpSocketAddr,
) -> String {
  let host: Cow<str> = match addr {
    HttpSocketAddr::IpSocket(addr) => {
      if let Some(auth) = req.uri().authority() {
        match addr.port() {
          443 if scheme == "https" => Cow::Borrowed(auth.host()),
          80 if scheme == "http" => Cow::Borrowed(auth.host()),
          _ => Cow::Borrowed(auth.as_str()), // Includes port number.
        }
      } else if let Some(host) = req.uri().host() {
        Cow::Borrowed(host)
      } else if let Some(host) = req.headers().get("HOST") {
        match host.to_str() {
          Ok(host) => Cow::Borrowed(host),
          Err(_) => Cow::Owned(
            host
              .as_bytes()
              .iter()
              .cloned()
              .map(char::from)
              .collect::<String>(),
          ),
        }
      } else {
        Cow::Owned(addr.to_string())
      }
    }
    // There is no standard way for unix domain socket URLs
    // nginx and nodejs request use http://unix:[socket_path]:/ but it is not a valid URL
    // httpie uses http+unix://[percent_encoding_of_path]/ which we follow
    #[cfg(unix)]
    HttpSocketAddr::UnixSocket(addr) => Cow::Owned(
      percent_encoding::percent_encode(
        addr
          .as_pathname()
          .and_then(|x| x.to_str())
          .unwrap_or_default()
          .as_bytes(),
        percent_encoding::NON_ALPHANUMERIC,
      )
      .to_string(),
    ),
  };
  let path = req.uri().path_and_query().map_or("/", |p| p.as_str());
  [scheme, "://", &host, path].concat()
}

fn req_headers(
  header_map: &HeaderMap<HeaderValue>,
) -> Vec<(ByteString, ByteString)> {
  // We treat cookies specially, because we don't want them to get them
  // mangled by the `Headers` object in JS. What we do is take all cookie
  // headers and concat them into a single cookie header, separated by
  // semicolons.
  let cookie_sep = "; ".as_bytes();
  let mut cookies = vec![];

  let mut headers = Vec::with_capacity(header_map.len());
  for (name, value) in header_map.iter() {
    if name == hyper::header::COOKIE {
      cookies.push(value.as_bytes());
    } else {
      let name: &[u8] = name.as_ref();
      let value = value.as_bytes();
      headers.push((name.into(), value.into()));
    }
  }

  if !cookies.is_empty() {
    headers.push(("cookie".into(), cookies.join(cookie_sep).into()));
  }

  headers
}

#[op]
async fn op_http_write_headers(
  state: Rc<RefCell<OpState>>,
  rid: u32,
  status: u16,
  headers: Vec<(ByteString, ByteString)>,
  data: Option<StringOrBuffer>,
) -> Result<(), AnyError> {
  let stream = state
    .borrow_mut()
    .resource_table
    .get::<HttpStreamResource>(rid)?;

  // Track supported encoding
  let encoding = stream.accept_encoding;

  let mut builder = Response::builder();
  // SAFETY: can not fail, since a fresh Builder is non-errored
  let hmap = unsafe { builder.headers_mut().unwrap_unchecked() };

  // Add headers
  hmap.reserve(headers.len() + 2);
  for (k, v) in headers.into_iter() {
    let v: Vec<u8> = v.into();
    hmap.append(
      HeaderName::try_from(k.as_slice())?,
      HeaderValue::try_from(v)?,
    );
  }
  ensure_vary_accept_encoding(hmap);

  let accepts_compression =
    matches!(encoding, Encoding::Brotli | Encoding::Gzip);
  let compressing = accepts_compression
    && (matches!(data, Some(ref data) if data.len() > 20) || data.is_none())
    && should_compress(hmap);

  if compressing {
    weaken_etag(hmap);
    // Drop 'content-length' header. Hyper will update it using compressed body.
    hmap.remove(hyper::header::CONTENT_LENGTH);
    // Content-Encoding header
    hmap.insert(
      hyper::header::CONTENT_ENCODING,
      HeaderValue::from_static(match encoding {
        Encoding::Brotli => "br",
        Encoding::Gzip => "gzip",
        _ => unreachable!(), // Forbidden by accepts_compression
      }),
    );
  }

  let (new_wr, body) = http_response(data, compressing, encoding)?;
  let body = builder.status(status).body(body)?;

  let mut old_wr = RcRef::map(&stream, |r| &r.wr).borrow_mut().await;
  let response_tx = match replace(&mut *old_wr, new_wr) {
    HttpResponseWriter::Headers(response_tx) => response_tx,
    _ => return Err(http_error("response headers already sent")),
  };

  match response_tx.send(body) {
    Ok(_) => Ok(()),
    Err(_) => {
      stream.conn.closed().await?;
      Err(http_error("connection closed while sending response"))
    }
  }
}

#[op]
fn op_http_headers(
  state: &mut OpState,
  rid: u32,
) -> Result<Vec<(ByteString, ByteString)>, AnyError> {
  let stream = state.resource_table.get::<HttpStreamResource>(rid)?;
  let rd = RcRef::map(&stream, |r| &r.rd)
    .try_borrow()
    .ok_or_else(|| http_error("already in use"))?;
  match &*rd {
    HttpRequestReader::Headers(request) => Ok(req_headers(request.headers())),
    HttpRequestReader::Body(headers, _) => Ok(req_headers(headers)),
    _ => unreachable!(),
  }
}

fn http_response(
  data: Option<StringOrBuffer>,
  compressing: bool,
  encoding: Encoding,
) -> Result<(HttpResponseWriter, hyper::Body), AnyError> {
  match data {
    Some(data) if compressing => match encoding {
      Encoding::Brotli => {
        // quality level 6 is based on google's nginx default value for
        // on-the-fly compression
        // https://github.com/google/ngx_brotli#brotli_comp_level
        // lgwin 22 is equivalent to brotli window size of (2**22)-16 bytes
        // (~4MB)
        let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 6, 22);
        writer.write_all(&data)?;
        Ok((HttpResponseWriter::Closed, writer.into_inner().into()))
      }
      Encoding::Gzip => {
        // Gzip, after level 1, doesn't produce significant size difference.
        // Probably the reason why nginx's default gzip compression level is
        // 1.
        // https://nginx.org/en/docs/http/ngx_http_gzip_module.html#gzip_comp_level
        let mut writer = GzEncoder::new(Vec::new(), Compression::new(1));
        writer.write_all(&data)?;
        Ok((HttpResponseWriter::Closed, writer.finish()?.into()))
      }
      _ => unreachable!(), // forbidden by accepts_compression
    },
    Some(data) => {
      // If a buffer was passed, but isn't compressible, we use it to
      // construct a response body.
      Ok((HttpResponseWriter::Closed, Bytes::from(data).into()))
    }
    None if compressing => {
      // Create a one way pipe that implements tokio's async io traits. To do
      // this we create a [tokio::io::DuplexStream], but then throw away one
      // of the directions to create a one way pipe.
      let (a, b) = tokio::io::duplex(64 * 1024);
      let (reader, _) = tokio::io::split(a);
      let (_, writer) = tokio::io::split(b);
      let writer: Pin<Box<dyn tokio::io::AsyncWrite>> = match encoding {
        Encoding::Brotli => Box::pin(BrotliEncoder::new(writer)),
        Encoding::Gzip => Box::pin(GzipEncoder::new(writer)),
        _ => unreachable!(), // forbidden by accepts_compression
      };
      let (stream, shutdown_handle) =
        ExternallyAbortableReaderStream::new(reader);
      Ok((
        HttpResponseWriter::Body {
          writer,
          shutdown_handle,
        },
        Body::wrap_stream(stream),
      ))
    }
    None => {
      let (body_tx, body_rx) = Body::channel();
      Ok((
        HttpResponseWriter::BodyUncompressed(body_tx.into()),
        body_rx,
      ))
    }
  }
}

// If user provided a ETag header for uncompressed data, we need to
// ensure it is a Weak Etag header ("W/").
fn weaken_etag(hmap: &mut hyper::HeaderMap) {
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
fn ensure_vary_accept_encoding(hmap: &mut hyper::HeaderMap) {
  if let Some(v) = hmap.get_mut(hyper::header::VARY) {
    if let Ok(s) = v.to_str() {
      if !s.to_lowercase().contains("accept-encoding") {
        *v = format!("Accept-Encoding, {}", s).try_into().unwrap()
      }
      return;
    }
  }
  hmap.insert(
    hyper::header::VARY,
    HeaderValue::from_static("Accept-Encoding"),
  );
}

fn should_compress(headers: &hyper::HeaderMap) -> bool {
  // skip compression if the cache-control header value is set to "no-transform" or not utf8
  fn cache_control_no_transform(headers: &hyper::HeaderMap) -> Option<bool> {
    let v = headers.get(hyper::header::CACHE_CONTROL)?;
    let s = match std::str::from_utf8(v.as_bytes()) {
      Ok(s) => s,
      Err(_) => return Some(true),
    };
    let c = CacheControl::from_value(s)?;
    Some(c.no_transform)
  }
  // we skip compression if the `content-range` header value is set, as it
  // indicates the contents of the body were negotiated based directly
  // with the user code and we can't compress the response
  let content_range = headers.contains_key(hyper::header::CONTENT_RANGE);
  // assume body is already compressed if Content-Encoding header present, thus avoid recompressing
  let is_precompressed = headers.contains_key(hyper::header::CONTENT_ENCODING);

  !content_range
    && !is_precompressed
    && !cache_control_no_transform(headers).unwrap_or_default()
    && headers
      .get(hyper::header::CONTENT_TYPE)
      .map(compressible::is_content_compressible)
      .unwrap_or_default()
}

#[op]
async fn op_http_write_resource(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  stream: ResourceId,
) -> Result<(), AnyError> {
  let http_stream = state
    .borrow()
    .resource_table
    .get::<HttpStreamResource>(rid)?;
  let mut wr = RcRef::map(&http_stream, |r| &r.wr).borrow_mut().await;
  let resource = state.borrow().resource_table.get_any(stream)?;
  loop {
    match *wr {
      HttpResponseWriter::Headers(_) => {
        return Err(http_error("no response headers"))
      }
      HttpResponseWriter::Closed => {
        return Err(http_error("response already completed"))
      }
      _ => {}
    };

    let view = resource.clone().read(64 * 1024).await?; // 64KB
    if view.is_empty() {
      break;
    }

    match &mut *wr {
      HttpResponseWriter::Body { writer, .. } => {
        let mut result = writer.write_all(&view).await;
        if result.is_ok() {
          result = writer.flush().await;
        }
        if let Err(err) = result {
          assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
          // Don't return "broken pipe", that's an implementation detail.
          // Pull up the failure associated with the transport connection instead.
          http_stream.conn.closed().await?;
          // If there was no connection error, drop body_tx.
          *wr = HttpResponseWriter::Closed;
        }
      }
      HttpResponseWriter::BodyUncompressed(body) => {
        let bytes = Bytes::from(view);
        if let Err(err) = body.sender().send_data(bytes).await {
          assert!(err.is_closed());
          // Pull up the failure associated with the transport connection instead.
          http_stream.conn.closed().await?;
          // If there was no connection error, drop body_tx.
          *wr = HttpResponseWriter::Closed;
        }
      }
      _ => unreachable!(),
    };
  }
  Ok(())
}

#[op]
async fn op_http_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let stream = state
    .borrow()
    .resource_table
    .get::<HttpStreamResource>(rid)?;
  let mut wr = RcRef::map(&stream, |r| &r.wr).borrow_mut().await;

  match &mut *wr {
    HttpResponseWriter::Headers(_) => Err(http_error("no response headers")),
    HttpResponseWriter::Closed => Err(http_error("response already completed")),
    HttpResponseWriter::Body { writer, .. } => {
      let mut result = writer.write_all(&buf).await;
      if result.is_ok() {
        result = writer.flush().await;
      }
      match result {
        Ok(_) => Ok(()),
        Err(err) => {
          assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
          // Don't return "broken pipe", that's an implementation detail.
          // Pull up the failure associated with the transport connection instead.
          stream.conn.closed().await?;
          // If there was no connection error, drop body_tx.
          *wr = HttpResponseWriter::Closed;
          Err(http_error("response already completed"))
        }
      }
    }
    HttpResponseWriter::BodyUncompressed(body) => {
      let bytes = Bytes::from(buf);
      match body.sender().send_data(bytes).await {
        Ok(_) => Ok(()),
        Err(err) => {
          assert!(err.is_closed());
          // Pull up the failure associated with the transport connection instead.
          stream.conn.closed().await?;
          // If there was no connection error, drop body_tx.
          *wr = HttpResponseWriter::Closed;
          Err(http_error("response already completed"))
        }
      }
    }
  }
}

/// Gracefully closes the write half of the HTTP stream. Note that this does not
/// remove the HTTP stream resource from the resource table; it still has to be
/// closed with `Deno.core.close()`.
#[op]
async fn op_http_shutdown(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let stream = state
    .borrow()
    .resource_table
    .get::<HttpStreamResource>(rid)?;
  let mut wr = RcRef::map(&stream, |r| &r.wr).borrow_mut().await;
  let wr = take(&mut *wr);
  match wr {
    HttpResponseWriter::Body {
      mut writer,
      shutdown_handle,
    } => {
      shutdown_handle.shutdown();
      match writer.shutdown().await {
        Ok(_) => {}
        Err(err) => {
          assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
          // Don't return "broken pipe", that's an implementation detail.
          // Pull up the failure associated with the transport connection instead.
          stream.conn.closed().await?;
        }
      }
    }
    HttpResponseWriter::BodyUncompressed(body) => {
      body.shutdown();
    }
    _ => {}
  };
  Ok(())
}

#[op]
fn op_http_websocket_accept_header(key: String) -> Result<String, AnyError> {
  let digest = ring::digest::digest(
    &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
    format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key).as_bytes(),
  );
  Ok(base64::encode(digest))
}

struct UpgradedStream(hyper::upgrade::Upgraded);
impl tokio::io::AsyncRead for UpgradedStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut tokio::io::ReadBuf,
  ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
  }
}

impl tokio::io::AsyncWrite for UpgradedStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
  }
  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_flush(cx)
  }
  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
  }
}

impl deno_websocket::Upgraded for UpgradedStream {}

#[op]
async fn op_http_upgrade_websocket(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ResourceId, AnyError> {
  let stream = state
    .borrow_mut()
    .resource_table
    .get::<HttpStreamResource>(rid)?;
  let mut rd = RcRef::map(&stream, |r| &r.rd).borrow_mut().await;

  let request = match &mut *rd {
    HttpRequestReader::Headers(request) => request,
    _ => {
      return Err(http_error("cannot upgrade because request body was used"))
    }
  };

  let transport = hyper::upgrade::on(request).await?;
  let ws_rid =
    ws_create_server_stream(&state, Box::pin(UpgradedStream(transport)))
      .await?;
  Ok(ws_rid)
}

// Needed so hyper can use non Send futures
#[derive(Clone)]
struct LocalExecutor;

impl<Fut> hyper::rt::Executor<Fut> for LocalExecutor
where
  Fut: Future + 'static,
  Fut::Output: 'static,
{
  fn execute(&self, fut: Fut) {
    spawn_local(fut);
  }
}

fn http_error(message: &'static str) -> AnyError {
  custom_error("Http", message)
}

/// Filters out the ever-surprising 'shutdown ENOTCONN' errors.
fn filter_enotconn(
  result: Result<(), hyper::Error>,
) -> Result<(), hyper::Error> {
  if result
    .as_ref()
    .err()
    .and_then(|err| err.source())
    .and_then(|err| err.downcast_ref::<io::Error>())
    .filter(|err| err.kind() == io::ErrorKind::NotConnected)
    .is_some()
  {
    Ok(())
  } else {
    result
  }
}

/// Create a future that is forever pending.
fn never() -> Pending<Never> {
  pending()
}
