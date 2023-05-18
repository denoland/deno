// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::stream::WebSocketStream;
use bytes::Bytes;
use deno_core::error::invalid_hostname;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::url;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use deno_net::raw::NetworkStream;
use deno_tls::create_client_config;
use deno_tls::RootCertStoreProvider;
use http::header::CONNECTION;
use http::header::UPGRADE;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::Request;
use http::Uri;
use hyper::Body;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio_rustls::rustls::RootCertStore;
use tokio_rustls::rustls::ServerName;
use tokio_rustls::TlsConnector;

use fastwebsockets::CloseCode;
use fastwebsockets::FragmentCollector;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::Role;
use fastwebsockets::WebSocket;

mod stream;

#[derive(Clone)]
pub struct WsRootStoreProvider(Option<Arc<dyn RootCertStoreProvider>>);

impl WsRootStoreProvider {
  pub fn get_or_try_init(&self) -> Result<Option<RootCertStore>, AnyError> {
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
  ) -> Result<(), AnyError>;
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
#[op]
pub fn op_ws_check_permission_and_cancel_handle<WP>(
  state: &mut OpState,
  api_name: String,
  url: String,
  cancel_handle: bool,
) -> Result<Option<ResourceId>, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  state
    .borrow_mut::<WP>()
    .check_net_url(&url::Url::parse(&url)?, &api_name)?;

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

async fn handshake<S: AsyncRead + AsyncWrite + Send + Unpin + 'static>(
  cancel_resource: Option<Rc<CancelHandle>>,
  request: Request<Body>,
  socket: S,
) -> Result<(WebSocket<WebSocketStream>, http::Response<Body>), AnyError> {
  let client =
    fastwebsockets::handshake::client(&LocalExecutor, request, socket);

  let (upgraded, response) = if let Some(cancel_resource) = cancel_resource {
    client.or_cancel(cancel_resource).await?
  } else {
    client.await
  }
  .map_err(|err| {
    DomExceptionNetworkError::new(&format!(
      "failed to connect to WebSocket: {err}"
    ))
  })?;

  let upgraded = upgraded.into_inner();
  let stream =
    WebSocketStream::new(stream::WsStreamKind::Upgraded(upgraded), None);
  let stream = WebSocket::after_handshake(stream, Role::Client);

  Ok((stream, response))
}

#[op]
pub async fn op_ws_create<WP>(
  state: Rc<RefCell<OpState>>,
  api_name: String,
  url: String,
  protocols: String,
  cancel_handle: Option<ResourceId>,
  headers: Option<Vec<(ByteString, ByteString)>>,
) -> Result<CreateResponse, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  {
    let mut s = state.borrow_mut();
    s.borrow_mut::<WP>()
      .check_net_url(&url::Url::parse(&url)?, &api_name)
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

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .try_borrow::<UnsafelyIgnoreCertificateErrors>()
    .and_then(|it| it.0.clone());
  let root_cert_store = state
    .borrow()
    .borrow::<WsRootStoreProvider>()
    .get_or_try_init()?;
  let user_agent = state.borrow().borrow::<WsUserAgent>().0.clone();
  let uri: Uri = url.parse()?;
  let mut request = Request::builder().method(Method::GET).uri(
    uri
      .path_and_query()
      .ok_or(type_error("Missing path in url".to_string()))?
      .as_str(),
  );

  let authority = uri.authority().unwrap().as_str();
  let host = authority
    .find('@')
    .map(|idx| authority.split_at(idx + 1).1)
    .unwrap_or_else(|| authority);
  request = request
    .header("User-Agent", user_agent)
    .header("Host", host)
    .header(UPGRADE, "websocket")
    .header(CONNECTION, "Upgrade")
    .header(
      "Sec-WebSocket-Key",
      fastwebsockets::handshake::generate_key(),
    )
    .header("Sec-WebSocket-Version", "13");

  if !protocols.is_empty() {
    request = request.header("Sec-WebSocket-Protocol", protocols);
  }

  if let Some(headers) = headers {
    for (key, value) in headers {
      let name = HeaderName::from_bytes(&key)
        .map_err(|err| type_error(err.to_string()))?;
      let v = HeaderValue::from_bytes(&value)
        .map_err(|err| type_error(err.to_string()))?;

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

  let request = request.body(Body::empty())?;
  let domain = &uri.host().unwrap().to_string();
  let port = &uri.port_u16().unwrap_or(match uri.scheme_str() {
    Some("wss") => 443,
    Some("ws") => 80,
    _ => unreachable!(),
  });
  let addr = format!("{domain}:{port}");
  let tcp_socket = TcpStream::connect(addr).await?;

  let (stream, response) = match uri.scheme_str() {
    Some("ws") => handshake(cancel_resource, request, tcp_socket).await?,
    Some("wss") => {
      let tls_config = create_client_config(
        root_cert_store,
        vec![],
        unsafely_ignore_certificate_errors,
        None,
      )?;
      let tls_connector = TlsConnector::from(Arc::new(tls_config));
      let dnsname = ServerName::try_from(domain.as_str())
        .map_err(|_| invalid_hostname(domain))?;
      let tls_socket = tls_connector.connect(dnsname, tcp_socket).await?;
      handshake(cancel_resource, request, tls_socket).await?
    }
    _ => unreachable!(),
  };

  if let Some(cancel_rid) = cancel_handle {
    state.borrow_mut().resource_table.close(cancel_rid).ok();
  }

  let resource = ServerWebSocket {
    ws: AsyncRefCell::new(FragmentCollector::new(stream)),
    closed: Rc::new(Cell::new(false)),
  };
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(resource);

  let protocol = match response.headers().get("Sec-WebSocket-Protocol") {
    Some(header) => header.to_str().unwrap(),
    None => "",
  };
  let extensions = response
    .headers()
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
  Closed = 4,
}

pub struct ServerWebSocket {
  ws: AsyncRefCell<FragmentCollector<WebSocketStream>>,
  closed: Rc<Cell<bool>>,
}

impl ServerWebSocket {
  #[inline]
  pub async fn write_frame(
    self: Rc<Self>,
    frame: Frame,
  ) -> Result<(), AnyError> {
    // SAFETY: fastwebsockets only needs a mutable reference to the WebSocket
    // to populate the write buffer. We encounter an await point when writing
    // to the socket after the frame has already been written to the buffer.
    let ws = unsafe { &mut *self.ws.as_ptr() };
    ws.write_frame(frame)
      .await
      .map_err(|err| type_error(err.to_string()))?;
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
) -> Result<ResourceId, AnyError> {
  let mut ws = WebSocket::after_handshake(
    WebSocketStream::new(
      stream::WsStreamKind::Network(transport),
      Some(read_buf),
    ),
    Role::Server,
  );
  ws.set_writev(true);
  ws.set_auto_close(true);
  ws.set_auto_pong(true);

  let ws_resource = ServerWebSocket {
    ws: AsyncRefCell::new(FragmentCollector::new(ws)),
    closed: Rc::new(Cell::new(false)),
  };

  let rid = state.resource_table.add(ws_resource);
  Ok(rid)
}

#[op]
pub async fn op_ws_send_binary(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  resource
    .write_frame(Frame::new(true, OpCode::Binary, None, data.to_vec()))
    .await
}

#[op]
pub async fn op_ws_send_text(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: String,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  resource
    .write_frame(Frame::new(true, OpCode::Text, None, data.into_bytes()))
    .await
}

#[op]
pub async fn op_ws_send_pong(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  resource.write_frame(Frame::pong(vec![])).await
}

#[op]
pub async fn op_ws_send_ping(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  resource
    .write_frame(Frame::new(true, OpCode::Ping, None, vec![]))
    .await
}

#[op(deferred)]
pub async fn op_ws_close(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  code: Option<u16>,
  reason: Option<String>,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let frame = reason
    .map(|reason| Frame::close(code.unwrap_or(1005), reason.as_bytes()))
    .unwrap_or_else(|| Frame::close_raw(vec![]));

  let cell = Rc::clone(&resource.closed);
  cell.set(true);
  resource.write_frame(frame).await?;
  Ok(())
}

#[op(fast)]
pub async fn op_ws_next_event(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(u16, StringOrBuffer), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;

  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;
  loop {
    let val = match ws.read_frame().await {
      Ok(val) => val,
      Err(err) => {
        // No message was received, socket closed while we waited.
        // Try close the stream, ignoring any errors, and report closed status to JavaScript.
        if resource.closed.get() {
          let _ = state.borrow_mut().resource_table.close(rid);
          return Ok((
            MessageKind::Closed as u16,
            StringOrBuffer::Buffer(vec![].into()),
          ));
        }

        return Ok((
          MessageKind::Error as u16,
          StringOrBuffer::String(err.to_string()),
        ));
      }
    };

    break Ok(match val.opcode {
      OpCode::Text => (
        MessageKind::Text as u16,
        StringOrBuffer::String(String::from_utf8(val.payload).unwrap()),
      ),
      OpCode::Binary => (
        MessageKind::Binary as u16,
        StringOrBuffer::Buffer(val.payload.into()),
      ),
      OpCode::Close => {
        if val.payload.len() < 2 {
          return Ok((1005, StringOrBuffer::String("".to_string())));
        }

        let close_code =
          CloseCode::from(u16::from_be_bytes([val.payload[0], val.payload[1]]));
        let reason = String::from_utf8(val.payload[2..].to_vec()).unwrap();
        (close_code.into(), StringOrBuffer::String(reason))
      }
      OpCode::Pong => (
        MessageKind::Pong as u16,
        StringOrBuffer::Buffer(vec![].into()),
      ),
      OpCode::Continuation | OpCode::Ping => {
        continue;
      }
    });
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
    op_ws_send_binary,
    op_ws_send_text,
    op_ws_send_ping,
    op_ws_send_pong,
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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_websocket.d.ts")
}

#[derive(Debug)]
pub struct DomExceptionNetworkError {
  pub msg: String,
}

impl DomExceptionNetworkError {
  pub fn new(msg: &str) -> Self {
    DomExceptionNetworkError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionNetworkError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionNetworkError {}

pub fn get_network_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionNetworkError>()
    .map(|_| "DOMExceptionNetworkError")
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
    deno_core::task::spawn(fut);
  }
}
