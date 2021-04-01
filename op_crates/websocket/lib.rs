// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::stream::SplitSink;
use deno_core::futures::stream::SplitStream;
use deno_core::futures::SinkExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;

use http::{Method, Request, Uri};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use tokio_tungstenite::stream::Stream as StreamSwitcher;
use tokio_tungstenite::tungstenite::Error as TungsteniteError;
use tokio_tungstenite::tungstenite::{
  handshake::client::Response, protocol::frame::coding::CloseCode,
  protocol::CloseFrame, Message,
};
use tokio_tungstenite::{client_async, WebSocketStream};
use webpki::DNSNameRef;

pub use tokio_tungstenite; // Re-export tokio_tungstenite

#[derive(Clone)]
pub struct WsCaData(pub Vec<u8>);
#[derive(Clone)]
pub struct WsUserAgent(pub String);

pub trait WebSocketPermissions {
  fn check_net_url(&self, _url: &url::Url) -> Result<(), AnyError>;
}

/// For use with `op_websocket_*` when the user does not want permissions.
pub struct NoWebSocketPermissions;

impl WebSocketPermissions for NoWebSocketPermissions {
  fn check_net_url(&self, _url: &url::Url) -> Result<(), AnyError> {
    Ok(())
  }
}

type MaybeTlsStream =
  StreamSwitcher<TcpStream, tokio_rustls::client::TlsStream<TcpStream>>;

type WsStream = WebSocketStream<MaybeTlsStream>;
struct WsStreamResource {
  tx: AsyncRefCell<SplitSink<WsStream, Message>>,
  rx: AsyncRefCell<SplitStream<WsStream>>,
  // When a `WsStreamResource` resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures are attached to this cancel handle.
  cancel: CancelHandle,
}

impl Resource for WsStreamResource {
  fn name(&self) -> Cow<str> {
    "webSocketStream".into()
  }
}

impl WsStreamResource {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckPermissionArgs {
  url: String,
}

// This op is needed because creating a WS instance in JavaScript is a sync
// operation and should throw error when permissions are not fulfilled,
// but actual op that connects WS is async.
pub fn op_ws_check_permission<WP>(
  state: &mut OpState,
  args: CheckPermissionArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  state
    .borrow::<WP>()
    .check_net_url(&url::Url::parse(&args.url)?)?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArgs {
  url: String,
  protocols: String,
}

pub async fn op_ws_create<WP>(
  state: Rc<RefCell<OpState>>,
  args: CreateArgs,
  _bufs: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError>
where
  WP: WebSocketPermissions + 'static,
{
  {
    let s = state.borrow();
    s.borrow::<WP>()
      .check_net_url(&url::Url::parse(&args.url)?)
      .expect(
        "Permission check should have been done in op_ws_check_permission",
      );
  }

  let ws_ca_data = state.borrow().try_borrow::<WsCaData>().cloned();
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
  let try_socket = TcpStream::connect(addr).await;
  let tcp_socket = match try_socket.map_err(TungsteniteError::Io) {
    Ok(socket) => socket,
    Err(_) => return Ok(json!({ "success": false })),
  };

  let socket: MaybeTlsStream = match uri.scheme_str() {
    Some("ws") => StreamSwitcher::Plain(tcp_socket),
    Some("wss") => {
      let mut config = ClientConfig::new();
      config
        .root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

      if let Some(ws_ca_data) = ws_ca_data {
        let reader = &mut BufReader::new(Cursor::new(ws_ca_data.0));
        config.root_store.add_pem_file(reader).unwrap();
      }

      let tls_connector = TlsConnector::from(Arc::new(config));
      let dnsname =
        DNSNameRef::try_from_ascii_str(&domain).expect("Invalid DNS lookup");
      let tls_socket = tls_connector.connect(dnsname, tcp_socket).await?;
      StreamSwitcher::Tls(tls_socket)
    }
    _ => unreachable!(),
  };

  let (stream, response): (WsStream, Response) =
    client_async(request, socket).await.map_err(|err| {
      type_error(format!(
        "failed to connect to WebSocket: {}",
        err.to_string()
      ))
    })?;

  let (ws_tx, ws_rx) = stream.split();
  let resource = WsStreamResource {
    rx: AsyncRefCell::new(ws_rx),
    tx: AsyncRefCell::new(ws_tx),
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
  Ok(json!({
    "success": true,
    "rid": rid,
    "protocol": protocol,
    "extensions": extensions
  }))
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
) -> Result<Value, AnyError> {
  let msg = match args.kind.as_str() {
    "text" => Message::Text(args.text.unwrap()),
    "binary" => Message::Binary(buf.ok_or(null_opbuf())?.to_vec()),
    "pong" => Message::Pong(vec![]),
    _ => unreachable!(),
  };
  let rid = args.rid;

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<WsStreamResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut tx = RcRef::map(&resource, |r| &r.tx).borrow_mut().await;
  tx.send(msg).await?;
  Ok(json!({}))
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
  _bufs: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
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
    .get::<WsStreamResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut tx = RcRef::map(&resource, |r| &r.tx).borrow_mut().await;
  tx.send(msg).await?;
  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextEventArgs {
  rid: ResourceId,
}

pub async fn op_ws_next_event(
  state: Rc<RefCell<OpState>>,
  args: NextEventArgs,
  _bufs: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<WsStreamResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let mut rx = RcRef::map(&resource, |r| &r.rx).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let val = rx.next().or_cancel(cancel).await?;
  let res = match val {
    Some(Ok(Message::Text(text))) => json!({
      "kind": "string",
      "data": text
    }),
    Some(Ok(Message::Binary(data))) => {
      // TODO(ry): don't use json to send binary data.
      json!({
        "kind": "binary",
        "data": data
      })
    }
    Some(Ok(Message::Close(Some(frame)))) => json!({
      "kind": "close",
      "data": {
        "code": u16::from(frame.code),
        "reason": frame.reason.as_ref()
      }
    }),
    Some(Ok(Message::Close(None))) => json!({
      "kind": "close",
      "data": {
        "code": 1005,
        "reason": ""
      }
    }),
    Some(Ok(Message::Ping(_))) => json!({ "kind": "ping" }),
    Some(Ok(Message::Pong(_))) => json!({ "kind": "pong" }),
    Some(Err(_)) => json!({ "kind": "error" }),
    None => {
      state.borrow_mut().resource_table.close(args.rid).unwrap();
      json!({ "kind": "closed" })
    }
  };
  Ok(res)
}

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  isolate
    .execute(
      "deno:op_crates/websocket/01_websocket.js",
      include_str!("01_websocket.js"),
    )
    .unwrap();
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_websocket.d.ts")
}
