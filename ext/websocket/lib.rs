// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::invalid_hostname;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::futures::stream::SplitSink;
use deno_core::futures::stream::SplitStream;
use deno_core::futures::SinkExt;
use deno_core::futures::StreamExt;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::url;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_tls::create_client_config;
use deno_tls::webpki::DNSNameRef;

use http::{Method, Request, Uri};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::RootCertStore;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::{
  handshake::client::Response, protocol::frame::coding::CloseCode,
  protocol::CloseFrame, Message,
};
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::{client_async, WebSocketStream};

pub use tokio_tungstenite; // Re-export tokio_tungstenite

#[derive(Clone)]
pub struct WsRootStore(pub Option<RootCertStore>);
#[derive(Clone)]
pub struct WsUserAgent(pub String);

pub trait WebSocketPermissions {
  fn check_net_url(&mut self, _url: &url::Url) -> Result<(), AnyError>;
}

/// `UnsafelyIgnoreCertificateErrors` is a wrapper struct so it can be placed inside `GothamState`;
/// using type alias for a `Option<Vec<String>>` could work, but there's a high chance
/// that there might be another type alias pointing to a `Option<Vec<String>>`, which
/// would override previously used alias.
pub struct UnsafelyIgnoreCertificateErrors(Option<Vec<String>>);

/// For use with `op_websocket_*` when the user does not want permissions.
pub struct NoWebSocketPermissions;

impl WebSocketPermissions for NoWebSocketPermissions {
  fn check_net_url(&mut self, _url: &url::Url) -> Result<(), AnyError> {
    Ok(())
  }
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub enum WebSocketStreamType {
  Client {
    tx: AsyncRefCell<SplitSink<WsStream, Message>>,
    rx: AsyncRefCell<SplitStream<WsStream>>,
  },
  Server {
    tx: AsyncRefCell<
      SplitSink<WebSocketStream<hyper::upgrade::Upgraded>, Message>,
    >,
    rx: AsyncRefCell<SplitStream<WebSocketStream<hyper::upgrade::Upgraded>>>,
  },
}

pub struct WsStreamResource {
  pub stream: WebSocketStreamType,
  // When a `WsStreamResource` resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures are attached to this cancel handle.
  pub cancel: CancelHandle,
}

impl WsStreamResource {
  async fn send(self: &Rc<Self>, message: Message) -> Result<(), AnyError> {
    match self.stream {
      WebSocketStreamType::Client { .. } => {
        let mut tx = RcRef::map(self, |r| match &r.stream {
          WebSocketStreamType::Client { tx, .. } => tx,
          WebSocketStreamType::Server { .. } => unreachable!(),
        })
        .borrow_mut()
        .await;
        tx.send(message).await?;
      }
      WebSocketStreamType::Server { .. } => {
        let mut tx = RcRef::map(self, |r| match &r.stream {
          WebSocketStreamType::Client { .. } => unreachable!(),
          WebSocketStreamType::Server { tx, .. } => tx,
        })
        .borrow_mut()
        .await;
        tx.send(message).await?;
      }
    }

    Ok(())
  }

  async fn next_message(
    self: &Rc<Self>,
    cancel: RcRef<CancelHandle>,
  ) -> Result<
    Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
    AnyError,
  > {
    match &self.stream {
      WebSocketStreamType::Client { .. } => {
        let mut rx = RcRef::map(self, |r| match &r.stream {
          WebSocketStreamType::Client { rx, .. } => rx,
          WebSocketStreamType::Server { .. } => unreachable!(),
        })
        .borrow_mut()
        .await;
        rx.next().or_cancel(cancel).await.map_err(AnyError::from)
      }
      WebSocketStreamType::Server { .. } => {
        let mut rx = RcRef::map(self, |r| match &r.stream {
          WebSocketStreamType::Client { .. } => unreachable!(),
          WebSocketStreamType::Server { rx, .. } => rx,
        })
        .borrow_mut()
        .await;
        rx.next().or_cancel(cancel).await.map_err(AnyError::from)
      }
    }
  }
}

impl Resource for WsStreamResource {
  fn name(&self) -> Cow<str> {
    "webSocketStream".into()
  }
}

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
pub fn op_ws_check_permission_and_cancel_handle<WP>(
  state: &mut OpState,
  url: String,
  cancel_handle: bool,
) -> Result<Option<ResourceId>, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  state
    .borrow_mut::<WP>()
    .check_net_url(&url::Url::parse(&url)?)?;

  if cancel_handle {
    let rid = state
      .resource_table
      .add(WsCancelResource(CancelHandle::new_rc()));
    Ok(Some(rid))
  } else {
    Ok(None)
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArgs {
  url: String,
  protocols: String,
  cancel_handle: Option<ResourceId>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResponse {
  rid: ResourceId,
  protocol: String,
  extensions: String,
}

pub async fn op_ws_create<WP>(
  state: Rc<RefCell<OpState>>,
  args: CreateArgs,
  _: (),
) -> Result<CreateResponse, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  {
    let mut s = state.borrow_mut();
    s.borrow_mut::<WP>()
      .check_net_url(&url::Url::parse(&args.url)?)
      .expect(
        "Permission check should have been done in op_ws_check_permission",
      );
  }

  let unsafely_ignore_certificate_errors = state
    .borrow()
    .borrow::<UnsafelyIgnoreCertificateErrors>()
    .0
    .clone();
  let root_cert_store = state.borrow().borrow::<WsRootStore>().0.clone();
  let user_agent = state.borrow().borrow::<WsUserAgent>().0.clone();
  let uri: Uri = args.url.parse()?;
  let mut request = Request::builder().method(Method::GET).uri(&uri);

  request = request.header("User-Agent", user_agent);

  if !args.protocols.is_empty() {
    request = request.header("Sec-WebSocket-Protocol", args.protocols);
  }

  let request = request.body(())?;
  let domain = &uri.host().unwrap().to_string();
  let port = &uri.port_u16().unwrap_or(match uri.scheme_str() {
    Some("wss") => 443,
    Some("ws") => 80,
    _ => unreachable!(),
  });
  let addr = format!("{}:{}", domain, port);
  let tcp_socket = TcpStream::connect(addr).await?;

  let socket: MaybeTlsStream<TcpStream> = match uri.scheme_str() {
    Some("ws") => MaybeTlsStream::Plain(tcp_socket),
    Some("wss") => {
      let tls_config = create_client_config(
        root_cert_store,
        None,
        unsafely_ignore_certificate_errors,
      )?;
      let tls_connector = TlsConnector::from(Arc::new(tls_config));
      let dnsname = DNSNameRef::try_from_ascii_str(domain)
        .map_err(|_| invalid_hostname(domain))?;
      let tls_socket = tls_connector.connect(dnsname, tcp_socket).await?;
      MaybeTlsStream::Rustls(tls_socket)
    }
    _ => unreachable!(),
  };

  let client = client_async(request, socket);
  let (stream, response): (WsStream, Response) =
    if let Some(cancel_rid) = args.cancel_handle {
      let r = state
        .borrow_mut()
        .resource_table
        .get::<WsCancelResource>(cancel_rid)?;
      client
        .or_cancel(r.0.to_owned())
        .await
        .map_err(|_| DomExceptionAbortError::new("connection was aborted"))?
    } else {
      client.await
    }
    .map_err(|err| {
      DomExceptionNetworkError::new(&format!(
        "failed to connect to WebSocket: {}",
        err.to_string()
      ))
    })?;

  if let Some(cancel_rid) = args.cancel_handle {
    state.borrow_mut().resource_table.close(cancel_rid).ok();
  }

  let (ws_tx, ws_rx) = stream.split();
  let resource = WsStreamResource {
    stream: WebSocketStreamType::Client {
      rx: AsyncRefCell::new(ws_rx),
      tx: AsyncRefCell::new(ws_tx),
    },
    cancel: Default::default(),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendArgs {
  rid: ResourceId,
  kind: String,
  text: Option<String>,
}

pub async fn op_ws_send(
  state: Rc<RefCell<OpState>>,
  args: SendArgs,
  buf: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let msg = match args.kind.as_str() {
    "text" => Message::Text(args.text.unwrap()),
    "binary" => Message::Binary(buf.ok_or_else(null_opbuf)?.to_vec()),
    "pong" => Message::Pong(vec![]),
    _ => unreachable!(),
  };

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<WsStreamResource>(args.rid)?;
  resource.send(msg).await?;
  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseArgs {
  rid: ResourceId,
  code: Option<u16>,
  reason: Option<String>,
}

pub async fn op_ws_close(
  state: Rc<RefCell<OpState>>,
  args: CloseArgs,
  _: (),
) -> Result<(), AnyError> {
  let rid = args.rid;
  let msg = Message::Close(args.code.map(|c| CloseFrame {
    code: CloseCode::from(c),
    reason: match args.reason {
      Some(reason) => Cow::from(reason),
      None => Default::default(),
    },
  }));

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<WsStreamResource>(rid)?;
  resource.send(msg).await?;
  Ok(())
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum NextEventResponse {
  String(String),
  Binary(ZeroCopyBuf),
  Close { code: u16, reason: String },
  Ping,
  Pong,
  Error(String),
  Closed,
}

pub async fn op_ws_next_event(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<NextEventResponse, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<WsStreamResource>(rid)?;

  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let val = resource.next_message(cancel).await?;
  let res = match val {
    Some(Ok(Message::Text(text))) => NextEventResponse::String(text),
    Some(Ok(Message::Binary(data))) => NextEventResponse::Binary(data.into()),
    Some(Ok(Message::Close(Some(frame)))) => NextEventResponse::Close {
      code: frame.code.into(),
      reason: frame.reason.to_string(),
    },
    Some(Ok(Message::Close(None))) => NextEventResponse::Close {
      code: 1005,
      reason: String::new(),
    },
    Some(Ok(Message::Ping(_))) => NextEventResponse::Ping,
    Some(Ok(Message::Pong(_))) => NextEventResponse::Pong,
    Some(Err(e)) => NextEventResponse::Error(e.to_string()),
    None => {
      state.borrow_mut().resource_table.close(rid).unwrap();
      NextEventResponse::Closed
    }
  };
  Ok(res)
}

pub fn init<P: WebSocketPermissions + 'static>(
  user_agent: String,
  root_cert_store: Option<RootCertStore>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/websocket",
      "01_websocket.js",
      "02_websocketstream.js",
    ))
    .ops(vec![
      (
        "op_ws_check_permission_and_cancel_handle",
        op_sync(op_ws_check_permission_and_cancel_handle::<P>),
      ),
      ("op_ws_create", op_async(op_ws_create::<P>)),
      ("op_ws_send", op_async(op_ws_send)),
      ("op_ws_close", op_async(op_ws_close)),
      ("op_ws_next_event", op_async(op_ws_next_event)),
    ])
    .state(move |state| {
      state.put::<WsUserAgent>(WsUserAgent(user_agent.clone()));
      state.put(UnsafelyIgnoreCertificateErrors(
        unsafely_ignore_certificate_errors.clone(),
      ));
      state.put::<WsRootStore>(WsRootStore(root_cert_store.clone()));
      Ok(())
    })
    .build()
}

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

#[derive(Debug)]
pub struct DomExceptionAbortError {
  pub msg: String,
}

impl DomExceptionAbortError {
  pub fn new(msg: &str) -> Self {
    DomExceptionAbortError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionAbortError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionAbortError {}

pub fn get_abort_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionAbortError>()
    .map(|_| "DOMExceptionAbortError")
}
