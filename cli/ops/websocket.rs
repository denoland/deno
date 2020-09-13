// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use core::task::Poll;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpState;
use futures::future::poll_fn;
use futures::StreamExt;
use futures::{ready, SinkExt};
use http::{Method, Request, Uri};
use serde_derive::Deserialize;
use serde_json::Value;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{rustls::ClientConfig, TlsConnector};
use tokio_tungstenite::stream::Stream as StreamSwitcher;
use tokio_tungstenite::tungstenite::{
  handshake::client::Response, protocol::frame::coding::CloseCode,
  protocol::CloseFrame, Error, Message,
};
use tokio_tungstenite::{client_async, WebSocketStream};
use webpki::DNSNameRef;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_ws_create", op_ws_create);
  super::reg_json_async(rt, "op_ws_send", op_ws_send);
  super::reg_json_async(rt, "op_ws_close", op_ws_close);
  super::reg_json_async(rt, "op_ws_next_event", op_ws_next_event);
}

type MaybeTlsStream =
  StreamSwitcher<TcpStream, tokio_rustls::client::TlsStream<TcpStream>>;

type WsStream = WebSocketStream<MaybeTlsStream>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateArgs {
  url: String,
  protocols: String,
}

pub async fn op_ws_create(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, ErrBox> {
  let args: CreateArgs = serde_json::from_value(args)?;
  let ca_file = {
    let cli_state = super::cli_state2(&state);
    cli_state.check_net_url(&url::Url::parse(&args.url)?)?;
    cli_state.global_state.flags.ca_file.clone()
  };
  let uri: Uri = args.url.parse().unwrap();
  let request = Request::builder()
    .method(Method::GET)
    .uri(&uri)
    .header("Sec-WebSocket-Protocol", args.protocols)
    .body(())
    .unwrap();
  let domain = &uri.host().unwrap().to_string();
  let port = &uri.port_u16().unwrap_or(match uri.scheme_str() {
    Some("wss") => 443,
    Some("ws") => 80,
    _ => unreachable!(),
  });
  let addr = format!("{}:{}", domain, port);
  let try_socket = TcpStream::connect(addr).await;
  let tcp_socket = match try_socket.map_err(Error::Io) {
    Ok(socket) => socket,
    Err(_) => return Ok(json!({"success": false})),
  };

  let socket: MaybeTlsStream = match uri.scheme_str() {
    Some("ws") => StreamSwitcher::Plain(tcp_socket),
    Some("wss") => {
      let mut config = ClientConfig::new();
      config
        .root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

      if let Some(path) = ca_file {
        let key_file = File::open(path)?;
        let reader = &mut BufReader::new(key_file);
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
    client_async(request, socket).await.unwrap();

  let mut state = state.borrow_mut();
  let rid = state
    .resource_table
    .add("webSocketStream", Box::new(stream));

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
struct SendArgs {
  rid: u32,
  text: Option<String>,
}

pub async fn op_ws_send(
  state: Rc<RefCell<OpState>>,
  args: Value,
  bufs: BufVec,
) -> Result<Value, ErrBox> {
  let args: SendArgs = serde_json::from_value(args)?;

  let mut maybe_msg = Some(match args.text {
    Some(text) => Message::Text(text),
    None => Message::Binary(bufs[0].to_vec()),
  });
  let rid = args.rid;

  poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let stream = state
      .resource_table
      .get_mut::<WsStream>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;

    // TODO(ry) Handle errors below instead of unwrap.
    // Need to map tungstenite::error::Error to ErrBox.

    ready!(stream.poll_ready_unpin(cx)).unwrap();
    if let Some(msg) = maybe_msg.take() {
      stream.start_send_unpin(msg).unwrap();
    }
    ready!(stream.poll_flush_unpin(cx)).unwrap();

    Poll::Ready(Ok(json!({})))
  })
  .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseArgs {
  rid: u32,
  code: Option<u16>,
  reason: Option<String>,
}

pub async fn op_ws_close(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, ErrBox> {
  let args: CloseArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let mut maybe_msg = Some(Message::Close(args.code.map(|c| CloseFrame {
    code: CloseCode::from(c),
    reason: match args.reason {
      Some(reason) => Cow::from(reason),
      None => Default::default(),
    },
  })));

  poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let stream = state
      .resource_table
      .get_mut::<WsStream>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;

    // TODO(ry) Handle errors below instead of unwrap.
    // Need to map tungstenite::error::Error to ErrBox.

    ready!(stream.poll_ready_unpin(cx)).unwrap();
    if let Some(msg) = maybe_msg.take() {
      stream.start_send_unpin(msg).unwrap();
    }
    ready!(stream.poll_flush_unpin(cx)).unwrap();
    ready!(stream.poll_close_unpin(cx)).unwrap();

    Poll::Ready(Ok(json!({})))
  })
  .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextEventArgs {
  rid: u32,
}

pub async fn op_ws_next_event(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, ErrBox> {
  let args: NextEventArgs = serde_json::from_value(args)?;
  poll_fn(move |cx| {
    let mut state = state.borrow_mut();
    let stream = state
      .resource_table
      .get_mut::<WsStream>(args.rid)
      .ok_or_else(ErrBox::bad_resource_id)?;
    stream
      .poll_next_unpin(cx)
      .map(|val| {
        match val {
          Some(Ok(Message::Text(text))) => json!({
            "type": "string",
            "data": text
          }),
          Some(Ok(Message::Binary(data))) => {
            // TODO(ry): don't use json to send binary data.
            json!({
              "type": "binary",
              "data": data
            })
          }
          Some(Ok(Message::Close(Some(frame)))) => json!({
            "type": "close",
            "code": u16::from(frame.code),
            "reason": frame.reason.as_ref()
          }),
          Some(Ok(Message::Close(None))) => json!({ "type": "close" }),
          Some(Ok(Message::Ping(_))) => json!({"type": "ping"}),
          Some(Ok(Message::Pong(_))) => json!({"type": "pong"}),
          Some(Err(_)) => json!({"type": "error"}),
          None => {
            state.resource_table.close(args.rid).unwrap();
            json!({"type": "closed"})
          }
        }
      })
      .map(Ok)
  })
  .await
}
