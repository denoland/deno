// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;

use bytes::Bytes;
use deno_core::futures::TryFutureExt;
use deno_core::op2;
use deno_core::unsync::spawn;
use deno_core::url;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_error::JsErrorBox;
use deno_net::raw::NetworkStream;
use deno_permissions::PermissionCheckError;
use deno_tls::create_client_config;
use deno_tls::rustls::ClientConfig;
use deno_tls::rustls::ClientConnection;
use deno_tls::RootCertStoreProvider;
use deno_tls::SocketUse;
use deno_tls::TlsKeys;
use fastwebsockets::CloseCode;
use fastwebsockets::FragmentCollectorRead;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::Role;
use fastwebsockets::WebSocket;
use fastwebsockets::WebSocketWrite;
use http::header::CONNECTION;
use http::header::UPGRADE;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::Request;
use http::StatusCode;
use http::Uri;
use once_cell::sync::Lazy;
use rustls_tokio_stream::rustls::pki_types::ServerName;
use rustls_tokio_stream::rustls::RootCertStore;
use rustls_tokio_stream::TlsStream;
use serde::Serialize;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::net::TcpStream;

use crate::stream::WebSocketStream;

mod stream;

static USE_WRITEV: Lazy<bool> = Lazy::new(|| {
  let enable = std::env::var("DENO_USE_WRITEV").ok();

  if let Some(val) = enable {
    return !val.is_empty();
  }

  false
});

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WebsocketError {
  #[class(inherit)]
  #[error(transparent)]
  Url(url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(generic)]
  #[error(transparent)]
  Uri(#[from] http::uri::InvalidUri),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[class(type)]
  #[error(transparent)]
  WebSocket(#[from] fastwebsockets::WebSocketError),
  #[class("DOMExceptionNetworkError")]
  #[error("failed to connect to WebSocket: {0}")]
  ConnectionFailed(#[from] HandshakeError),
  #[class(inherit)]
  #[error(transparent)]
  Canceled(#[from] deno_core::Canceled),
}

#[derive(Clone)]
pub struct WsRootStoreProvider(Option<Arc<dyn RootCertStoreProvider>>);

impl WsRootStoreProvider {
  pub fn get_or_try_init(&self) -> Result<Option<RootCertStore>, JsErrorBox> {
    Ok(match &self.0 {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

#[derive(Clone)]
pub struct WsUserAgent(pub String);

pub trait WebSocketPermissions {
  fn check_net_url(
    &mut self,
    _url: &url::Url,
    _api_name: &str,
  ) -> Result<(), PermissionCheckError>;
}

impl WebSocketPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &url::Url,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_net_url(self, url, api_name)
  }
}

/// `UnsafelyIgnoreCertificateErrors` is a wrapper struct so it can be placed inside `GothamState`;
/// using type alias for a `Option<Vec<String>>` could work, but there's a high chance
/// that there might be another type alias pointing to a `Option<Vec<String>>`, which
/// would override previously used alias.
pub struct UnsafelyIgnoreCertificateErrors(Option<Vec<String>>);

pub struct WsCancelResource(Rc<CancelHandle>);

impl Resource for WsCancelResource {
  fn name(&self) -> Cow<str> {
    "webSocketCancel".into()
  }

  fn close(self: Rc<Self>) {
    self.0.cancel()
  }
}

// This op is needed because creating a WS instance in JavaScript is a sync
// operation and should throw error when permissions are not fulfilled,
// but actual op that connects WS is async.
#[op2(stack_trace)]
#[smi]
pub fn op_ws_check_permission_and_cancel_handle<WP>(
  state: &mut OpState,
  #[string] api_name: String,
  #[string] url: String,
  cancel_handle: bool,
) -> Result<Option<ResourceId>, WebsocketError>
where
  WP: WebSocketPermissions + 'static,
{
  state.borrow_mut::<WP>().check_net_url(
    &url::Url::parse(&url).map_err(WebsocketError::Url)?,
    &api_name,
  )?;

  if cancel_handle {
    let rid = state
      .resource_table
      .add(WsCancelResource(CancelHandle::new_rc()));
    Ok(Some(rid))
  } else {
    Ok(None)
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResponse {
  rid: ResourceId,
  protocol: String,
  extensions: String,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HandshakeError {
  #[class(type)]
  #[error("Missing path in url")]
  MissingPath,
  #[class(generic)]
  #[error("Invalid status code {0}")]
  InvalidStatusCode(StatusCode),
  #[class(generic)]
  #[error(transparent)]
  Http(#[from] http::Error),
  #[class(type)]
  #[error(transparent)]
  WebSocket(#[from] fastwebsockets::WebSocketError),
  #[class(generic)]
  #[error("Didn't receive h2 alpn, aborting connection")]
  NoH2Alpn,
  #[class(generic)]
  #[error(transparent)]
  Rustls(#[from] deno_tls::rustls::Error),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error(transparent)]
  H2(#[from] h2::Error),
  #[class(type)]
  #[error("Invalid hostname: '{0}'")]
  InvalidHostname(String),
  #[class(inherit)]
  #[error(transparent)]
  RootStoreError(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Tls(deno_tls::TlsError),
  #[class(type)]
  #[error(transparent)]
  HeaderName(#[from] http::header::InvalidHeaderName),
  #[class(type)]
  #[error(transparent)]
  HeaderValue(#[from] http::header::InvalidHeaderValue),
}

async fn handshake_websocket(
  state: &Rc<RefCell<OpState>>,
  uri: &Uri,
  protocols: &str,
  headers: Option<Vec<(ByteString, ByteString)>>,
) -> Result<(WebSocket<WebSocketStream>, http::HeaderMap), HandshakeError> {
  let mut request = Request::builder().method(Method::GET).uri(
    uri
      .path_and_query()
      .ok_or(HandshakeError::MissingPath)?
      .as_str(),
  );

  let authority = uri.authority().unwrap().as_str();
  let host = authority
    .find('@')
    .map(|idx| authority.split_at(idx + 1).1)
    .unwrap_or_else(|| authority);
  request = request
    .header("Host", host)
    .header(UPGRADE, "websocket")
    .header(CONNECTION, "Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    );

  let user_agent = state.borrow().borrow::<WsUserAgent>().0.clone();
  request =
    populate_common_request_headers(request, &user_agent, protocols, &headers)?;

  let request = request
    .body(http_body_util::Empty::new())
    .map_err(HandshakeError::Http)?;
  let domain = &uri.host().unwrap().to_string();
  let port = &uri.port_u16().unwrap_or(match uri.scheme_str() {
    Some("wss") => 443,
    Some("ws") => 80,
    _ => unreachable!(),
  });
  let addr = format!("{domain}:{port}");

  let res = match uri.scheme_str() {
    Some("ws") => handshake_http1_ws(request, &addr).await?,
    Some("wss") => {
      match handshake_http1_wss(state, request, domain, &addr).await {
        Ok(res) => res,
        Err(_) => {
          handshake_http2_wss(
            state,
            uri,
            authority,
            &user_agent,
            protocols,
            domain,
            &headers,
            &addr,
          )
          .await?
        }
      }
    }
    _ => unreachable!(),
  };
  Ok(res)
}

async fn handshake_http1_ws(
  request: Request<http_body_util::Empty<Bytes>>,
  addr: &String,
) -> Result<(WebSocket<WebSocketStream>, http::HeaderMap), HandshakeError> {
  let tcp_socket = TcpStream::connect(addr).await?;
  handshake_connection(request, tcp_socket).await
}

async fn handshake_http1_wss(
  state: &Rc<RefCell<OpState>>,
  request: Request<http_body_util::Empty<Bytes>>,
  domain: &str,
  addr: &str,
) -> Result<(WebSocket<WebSocketStream>, http::HeaderMap), HandshakeError> {
  let tcp_socket = TcpStream::connect(addr).await?;
  let tls_config = create_ws_client_config(state, SocketUse::Http1Only)?;
  let dnsname = ServerName::try_from(domain.to_string())
    .map_err(|_| HandshakeError::InvalidHostname(domain.to_string()))?;
  let mut tls_connector = TlsStream::new_client_side(
    tcp_socket,
    ClientConnection::new(tls_config.into(), dnsname)?,
    NonZeroUsize::new(65536),
  );
  // If we can bail on an http/1.1 ALPN mismatch here, we can avoid doing extra work
  tls_connector.handshake().await?;
  handshake_connection(request, tls_connector).await
}

#[allow(clippy::too_many_arguments)]
async fn handshake_http2_wss(
  state: &Rc<RefCell<OpState>>,
  uri: &Uri,
  authority: &str,
  user_agent: &str,
  protocols: &str,
  domain: &str,
  headers: &Option<Vec<(ByteString, ByteString)>>,
  addr: &str,
) -> Result<(WebSocket<WebSocketStream>, http::HeaderMap), HandshakeError> {
  let tcp_socket = TcpStream::connect(addr).await?;
  let tls_config = create_ws_client_config(state, SocketUse::Http2Only)?;
  let dnsname = ServerName::try_from(domain.to_string())
    .map_err(|_| HandshakeError::InvalidHostname(domain.to_string()))?;
  // We need to better expose the underlying errors here
  let mut tls_connector = TlsStream::new_client_side(
    tcp_socket,
    ClientConnection::new(tls_config.into(), dnsname)?,
    None,
  );
  let handshake = tls_connector.handshake().await?;
  if handshake.alpn.is_none() {
    return Err(HandshakeError::NoH2Alpn);
  }
  let h2 = h2::client::Builder::new();
  let (mut send, conn) = h2.handshake::<_, Bytes>(tls_connector).await?;
  spawn(conn);
  let mut request = Request::builder();
  request = request.method(Method::CONNECT);
  let uri = Uri::builder()
    .authority(authority)
    .path_and_query(uri.path_and_query().unwrap().as_str())
    .scheme("https")
    .build()?;
  request = request.uri(uri);
  request =
    populate_common_request_headers(request, user_agent, protocols, headers)?;
  request = request.extension(h2::ext::Protocol::from("websocket"));
  let (resp, send) = send.send_request(request.body(())?, false)?;
  let resp = resp.await?;
  if resp.status() != StatusCode::OK {
    return Err(HandshakeError::InvalidStatusCode(resp.status()));
  }
  let (http::response::Parts { headers, .. }, recv) = resp.into_parts();
  let mut stream = WebSocket::after_handshake(
    WebSocketStream::new(stream::WsStreamKind::H2(send, recv), None),
    Role::Client,
  );
  // We currently don't support vectored writes in the H2 streams
  stream.set_writev(false);
  // TODO(mmastrac): we should be able to use a zero masking key over HTTPS
  // stream.set_auto_apply_mask(false);
  Ok((stream, headers))
}

async fn handshake_connection<
  S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
>(
  request: Request<http_body_util::Empty<Bytes>>,
  socket: S,
) -> Result<(WebSocket<WebSocketStream>, http::HeaderMap), HandshakeError> {
  let (upgraded, response) =
    fastwebsockets::handshake::client(&LocalExecutor, request, socket).await?;

  let upgraded = upgraded.into_inner();
  let stream =
    WebSocketStream::new(stream::WsStreamKind::Upgraded(upgraded), None);
  let stream = WebSocket::after_handshake(stream, Role::Client);

  Ok((stream, response.into_parts().0.headers))
}

pub fn create_ws_client_config(
  state: &Rc<RefCell<OpState>>,
  socket_use: SocketUse,
) -> Result<ClientConfig, HandshakeError> {
  let unsafely_ignore_certificate_errors: Option<Vec<String>> = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());
  let root_cert_store = state
    .borrow()
    .borrow::<WsRootStoreProvider>()
    .get_or_try_init()
    .map_err(HandshakeError::RootStoreError)?;

  create_client_config(
    root_cert_store,
    vec![],
    unsafely_ignore_certificate_errors,
    TlsKeys::Null,
    socket_use,
  )
  .map_err(HandshakeError::Tls)
}

/// Headers common to both http/1.1 and h2 requests.
fn populate_common_request_headers(
  mut request: http::request::Builder,
  user_agent: &str,
  protocols: &str,
  headers: &Option<Vec<(ByteString, ByteString)>>,
) -> Result<http::request::Builder, HandshakeError> {
  request = request
    .header("User-Agent", user_agent)
    .header("Sec-WebSocket-Version", "13");

  if !protocols.is_empty() {
    request = request.header("Sec-WebSocket-Protocol", protocols);
  }

  if let Some(headers) = headers {
    for (key, value) in headers {
      let name = HeaderName::from_bytes(key)?;
      let v = HeaderValue::from_bytes(value)?;

      let is_disallowed_header = matches!(
        name,
        http::header::HOST
          | http::header::SEC_WEBSOCKET_ACCEPT
          | http::header::SEC_WEBSOCKET_EXTENSIONS
          | http::header::SEC_WEBSOCKET_KEY
          | http::header::SEC_WEBSOCKET_PROTOCOL
          | http::header::SEC_WEBSOCKET_VERSION
          | http::header::UPGRADE
          | http::header::CONNECTION
      );
      if !is_disallowed_header {
        request = request.header(name, v);
      }
    }
  }
  Ok(request)
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_ws_create<WP>(
  state: Rc<RefCell<OpState>>,
  #[string] api_name: String,
  #[string] url: String,
  #[string] protocols: String,
  #[smi] cancel_handle: Option<ResourceId>,
  #[serde] headers: Option<Vec<(ByteString, ByteString)>>,
) -> Result<CreateResponse, WebsocketError>
where
  WP: WebSocketPermissions + 'static,
{
  {
    let mut s = state.borrow_mut();
    s.borrow_mut::<WP>()
      .check_net_url(
        &url::Url::parse(&url).map_err(WebsocketError::Url)?,
        &api_name,
      )
      .expect(
        "Permission check should have been done in op_ws_check_permission",
      );
  }

  let cancel_resource = if let Some(cancel_rid) = cancel_handle {
    let r = state
      .borrow_mut()
      .resource_table
      .get::<WsCancelResource>(cancel_rid)?;
    Some(r.0.clone())
  } else {
    None
  };

  let uri: Uri = url.parse()?;

  let handshake = handshake_websocket(&state, &uri, &protocols, headers)
    .map_err(WebsocketError::ConnectionFailed);
  let (stream, response) = match cancel_resource {
    Some(rc) => handshake.try_or_cancel(rc).await?,
    None => handshake.await?,
  };

  if let Some(cancel_rid) = cancel_handle {
    if let Ok(res) = state.borrow_mut().resource_table.take_any(cancel_rid) {
      res.close();
    }
  }

  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(ServerWebSocket::new(stream));

  let protocol = match response.get("Sec-WebSocket-Protocol") {
    Some(header) => header.to_str().unwrap(),
    None => "",
  };
  let extensions = response
    .get_all("Sec-WebSocket-Extensions")
    .iter()
    .map(|header| header.to_str().unwrap())
    .collect::<String>();
  Ok(CreateResponse {
    rid,
    protocol: protocol.to_string(),
    extensions,
  })
}

#[repr(u16)]
pub enum MessageKind {
  Text = 0,
  Binary = 1,
  Pong = 2,
  Error = 3,
  ClosedDefault = 1005,
}

/// To avoid locks, we keep as much as we can inside of [`Cell`]s.
pub struct ServerWebSocket {
  buffered: Cell<usize>,
  error: Cell<Option<String>>,
  errored: Cell<bool>,
  closed: Cell<bool>,
  buffer: Cell<Option<Vec<u8>>>,
  string: Cell<Option<String>>,
  ws_read: AsyncRefCell<FragmentCollectorRead<ReadHalf<WebSocketStream>>>,
  ws_write: AsyncRefCell<WebSocketWrite<WriteHalf<WebSocketStream>>>,
}

impl ServerWebSocket {
  fn new(ws: WebSocket<WebSocketStream>) -> Self {
    let (ws_read, ws_write) = ws.split(tokio::io::split);
    Self {
      buffered: Cell::new(0),
      error: Cell::new(None),
      errored: Cell::new(false),
      closed: Cell::new(false),
      buffer: Cell::new(None),
      string: Cell::new(None),
      ws_read: AsyncRefCell::new(FragmentCollectorRead::new(ws_read)),
      ws_write: AsyncRefCell::new(ws_write),
    }
  }

  fn set_error(&self, error: Option<String>) {
    if let Some(error) = error {
      self.error.set(Some(error));
      self.errored.set(true);
    } else {
      self.error.set(None);
      self.errored.set(false);
    }
  }

  /// Reserve a lock, but don't wait on it. This gets us our place in line.
  fn reserve_lock(
    self: &Rc<Self>,
  ) -> AsyncMutFuture<WebSocketWrite<WriteHalf<WebSocketStream>>> {
    RcRef::map(self, |r| &r.ws_write).borrow_mut()
  }

  #[inline]
  async fn write_frame(
    self: &Rc<Self>,
    lock: AsyncMutFuture<WebSocketWrite<WriteHalf<WebSocketStream>>>,
    frame: Frame<'_>,
  ) -> Result<(), WebsocketError> {
    let mut ws = lock.await;
    if ws.is_closed() {
      return Ok(());
    }
    ws.write_frame(frame).await?;
    Ok(())
  }
}

impl Resource for ServerWebSocket {
  fn name(&self) -> Cow<str> {
    "serverWebSocket".into()
  }
}

pub fn ws_create_server_stream(
  state: &mut OpState,
  transport: NetworkStream,
  read_buf: Bytes,
) -> ResourceId {
  let mut ws = WebSocket::after_handshake(
    WebSocketStream::new(
      stream::WsStreamKind::Network(transport),
      Some(read_buf),
    ),
    Role::Server,
  );
  ws.set_writev(*USE_WRITEV);
  ws.set_auto_close(true);
  ws.set_auto_pong(true);

  state.resource_table.add(ServerWebSocket::new(ws))
}

fn send_binary(state: &mut OpState, rid: ResourceId, data: &[u8]) {
  let resource = state.resource_table.get::<ServerWebSocket>(rid).unwrap();
  let data = data.to_vec();
  let len = data.len();
  resource.buffered.set(resource.buffered.get() + len);
  let lock = resource.reserve_lock();
  deno_core::unsync::spawn(async move {
    if let Err(err) = resource
      .write_frame(lock, Frame::new(true, OpCode::Binary, None, data.into()))
      .await
    {
      resource.set_error(Some(err.to_string()));
    } else {
      resource.buffered.set(resource.buffered.get() - len);
    }
  });
}

#[op2]
pub fn op_ws_send_binary(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[anybuffer] data: &[u8],
) {
  send_binary(state, rid, data)
}

#[op2(fast)]
pub fn op_ws_send_binary_ab(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[arraybuffer] data: &[u8],
) {
  send_binary(state, rid, data)
}

#[op2(fast)]
pub fn op_ws_send_text(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] data: String,
) {
  let resource = state.resource_table.get::<ServerWebSocket>(rid).unwrap();
  let len = data.len();
  resource.buffered.set(resource.buffered.get() + len);
  let lock = resource.reserve_lock();
  deno_core::unsync::spawn(async move {
    if let Err(err) = resource
      .write_frame(
        lock,
        Frame::new(true, OpCode::Text, None, data.into_bytes().into()),
      )
      .await
    {
      resource.set_error(Some(err.to_string()));
    } else {
      resource.buffered.set(resource.buffered.get() - len);
    }
  });
}

/// Async version of send. Does not update buffered amount as we rely on the socket itself for backpressure.
#[op2(async)]
pub async fn op_ws_send_binary_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] data: JsBuffer,
) -> Result<(), WebsocketError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let data = data.to_vec();
  let lock = resource.reserve_lock();
  resource
    .write_frame(lock, Frame::new(true, OpCode::Binary, None, data.into()))
    .await
}

/// Async version of send. Does not update buffered amount as we rely on the socket itself for backpressure.
#[op2(async)]
pub async fn op_ws_send_text_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] data: String,
) -> Result<(), WebsocketError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let lock = resource.reserve_lock();
  resource
    .write_frame(
      lock,
      Frame::new(true, OpCode::Text, None, data.into_bytes().into()),
    )
    .await
}

const EMPTY_PAYLOAD: &[u8] = &[];

#[op2(fast)]
#[smi]
pub fn op_ws_get_buffered_amount(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> u32 {
  state
    .resource_table
    .get::<ServerWebSocket>(rid)
    .unwrap()
    .buffered
    .get() as u32
}

#[op2(async)]
pub async fn op_ws_send_ping(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), WebsocketError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let lock = resource.reserve_lock();
  resource
    .write_frame(
      lock,
      Frame::new(true, OpCode::Ping, None, EMPTY_PAYLOAD.into()),
    )
    .await
}

#[op2(async(lazy))]
pub async fn op_ws_close(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[smi] code: Option<u16>,
  #[string] reason: Option<String>,
) -> Result<(), WebsocketError> {
  let Ok(resource) = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)
  else {
    return Ok(());
  };

  const EMPTY_PAYLOAD: &[u8] = &[];

  let frame = reason
    .map(|reason| Frame::close(code.unwrap_or(1005), reason.as_bytes()))
    .unwrap_or_else(|| match code {
      Some(code) => Frame::close(code, EMPTY_PAYLOAD),
      _ => Frame::close_raw(EMPTY_PAYLOAD.into()),
    });

  resource.closed.set(true);
  let lock = resource.reserve_lock();
  resource.write_frame(lock, frame).await
}

#[op2]
#[serde]
pub fn op_ws_get_buffer(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Option<ToJsBuffer> {
  let Ok(resource) = state.resource_table.get::<ServerWebSocket>(rid) else {
    return None;
  };
  resource.buffer.take().map(ToJsBuffer::from)
}

#[op2]
#[string]
pub fn op_ws_get_buffer_as_string(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Option<String> {
  let Ok(resource) = state.resource_table.get::<ServerWebSocket>(rid) else {
    return None;
  };
  resource.string.take()
}

#[op2]
#[string]
pub fn op_ws_get_error(state: &mut OpState, #[smi] rid: ResourceId) -> String {
  let Ok(resource) = state.resource_table.get::<ServerWebSocket>(rid) else {
    return "Bad resource".into();
  };
  resource.errored.set(false);
  resource.error.take().unwrap_or_default()
}

#[op2(async)]
pub async fn op_ws_next_event(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> u16 {
  let Ok(resource) = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)
  else {
    // op_ws_get_error will correctly handle a bad resource
    return MessageKind::Error as u16;
  };

  // If there's a pending error, this always returns error
  if resource.errored.get() {
    return MessageKind::Error as u16;
  }

  let mut ws = RcRef::map(&resource, |r| &r.ws_read).borrow_mut().await;
  let writer = RcRef::map(&resource, |r| &r.ws_write);
  let mut sender = move |frame| {
    let writer = writer.clone();
    async move { writer.borrow_mut().await.write_frame(frame).await }
  };
  loop {
    let res = ws.read_frame(&mut sender).await;
    let val = match res {
      Ok(val) => val,
      Err(err) => {
        // No message was received, socket closed while we waited.
        // Report closed status to JavaScript.
        if resource.closed.get() {
          return MessageKind::ClosedDefault as u16;
        }

        resource.set_error(Some(err.to_string()));
        return MessageKind::Error as u16;
      }
    };

    break match val.opcode {
      OpCode::Text => match String::from_utf8(val.payload.to_vec()) {
        Ok(s) => {
          resource.string.set(Some(s));
          MessageKind::Text as u16
        }
        Err(_) => {
          resource.set_error(Some("Invalid string data".into()));
          MessageKind::Error as u16
        }
      },
      OpCode::Binary => {
        resource.buffer.set(Some(val.payload.to_vec()));
        MessageKind::Binary as u16
      }
      OpCode::Close => {
        // Close reason is returned through error
        if val.payload.len() < 2 {
          resource.set_error(None);
          MessageKind::ClosedDefault as u16
        } else {
          let close_code = CloseCode::from(u16::from_be_bytes([
            val.payload[0],
            val.payload[1],
          ]));
          let reason = String::from_utf8(val.payload[2..].to_vec()).ok();
          resource.set_error(reason);
          close_code.into()
        }
      }
      OpCode::Pong => MessageKind::Pong as u16,
      OpCode::Continuation | OpCode::Ping => {
        continue;
      }
    };
  }
}

deno_core::extension!(deno_websocket,
  deps = [ deno_url, deno_webidl ],
  parameters = [P: WebSocketPermissions],
  ops = [
    op_ws_check_permission_and_cancel_handle<P>,
    op_ws_create<P>,
    op_ws_close,
    op_ws_next_event,
    op_ws_get_buffer,
    op_ws_get_buffer_as_string,
    op_ws_get_error,
    op_ws_send_binary,
    op_ws_send_binary_ab,
    op_ws_send_text,
    op_ws_send_binary_async,
    op_ws_send_text_async,
    op_ws_send_ping,
    op_ws_get_buffered_amount,
  ],
  esm = [ "01_websocket.js", "02_websocketstream.js" ],
  options = {
    user_agent: String,
    root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
    unsafely_ignore_certificate_errors: Option<Vec<String>>
  },
  state = |state, options| {
    state.put::<WsUserAgent>(WsUserAgent(options.user_agent));
    state.put(UnsafelyIgnoreCertificateErrors(
      options.unsafely_ignore_certificate_errors,
    ));
    state.put::<WsRootStoreProvider>(WsRootStoreProvider(options.root_cert_store_provider));
  },
);

// Needed so hyper can use non Send futures
#[derive(Clone)]
struct LocalExecutor;

impl<Fut> hyper::rt::Executor<Fut> for LocalExecutor
where
  Fut: Future + 'static,
  Fut::Output: 'static,
{
  fn execute(&self, fut: Fut) {
    deno_core::unsync::spawn(fut);
  }
}
