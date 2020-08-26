// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![allow(warnings)]

use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use core::future::Future;
use core::task::Context;
use core::task::Poll;
use deno_core::ZeroCopyBuf;
use deno_core::{CoreIsolate, CoreIsolateState};
use futures::executor::block_on;
use futures::future::{poll_fn, FutureExt};
use futures::ready;
use futures::Sink;
use futures::{SinkExt, StreamExt};
use http::{Method, Request, Uri};
use std::borrow::Cow;
use std::pin::Pin;
use std::rc::Rc;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{client_async_tls, MaybeTlsStream, WebSocketStream};

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  i.register_op("op_ws_create", s.stateful_json_op2(op_ws_create));
  i.register_op("op_ws_send", s.stateful_json_op2(op_ws_send));
  i.register_op("op_ws_close", s.stateful_json_op2(op_ws_close));
  i.register_op("op_ws_next_event", s.stateful_json_op2(op_ws_next_event));
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateArgs {
  url: String,
  protocols: String,
}

pub fn op_ws_create(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: CreateArgs = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let future = async move {
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
      _ => 0,
    });
    let addr = format!("{}:{}", domain, port);
    let try_socket = TcpStream::connect(addr).await;
    let socket = match try_socket.map_err(Error::Io) {
      Ok(socket) => socket,
      Err(_) => return Ok(json!({"success": false})),
    };

    let (stream, response) = client_async_tls(request, socket).await.unwrap();

    let rid = {
      let mut resource_table = resource_table.borrow_mut();
      resource_table.add("webSocketStream", Box::new(stream))
    };

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
  };
  Ok(JsonOp::Async(future.boxed_local()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendArgs {
  rid: u32,
  text: Option<String>,
}

pub fn op_ws_send(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: SendArgs = serde_json::from_value(args)?;

  let mut maybe_msg = Some(match args.text {
    Some(text) => Message::Text(text),
    None => Message::Binary(zero_copy[0].to_owned().to_vec()),
  });
  let resource_table = isolate_state.resource_table.clone();
  let rid = args.rid;

  let future = poll_fn(move |cx| {
    let mut resource_table = resource_table.borrow_mut();
    let mut stream = resource_table
      .get_mut::<WsStream>(rid)
      .ok_or_else(OpError::bad_resource_id)?;

    // TODO(ry) Handle errors below instead of unwrap.
    // Need to map tungstenite::error::Error to OpError.

    let stream_pin = Pin::new(&mut stream);
    ready!(stream_pin.poll_ready(cx)).unwrap();

    if let Some(msg) = maybe_msg.take() {
      let stream_pin = Pin::new(&mut stream);
      stream_pin.start_send(msg).unwrap();
    }

    let stream_pin = Pin::new(&mut stream);
    ready!(stream_pin.poll_flush(cx)).unwrap();

    Poll::Ready(Ok(json!({})))
  });
  Ok(JsonOp::Async(future.boxed_local()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseArgs {
  rid: u32,
  code: Option<u16>,
  reason: Option<String>,
}

pub fn op_ws_close(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: CloseArgs = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let rid = args.rid;
  let mut maybe_msg = Some(Message::Close(args.code.map(|c| CloseFrame {
    code: CloseCode::from(c),
    reason: match args.reason {
      Some(reason) => Cow::from(reason),
      None => Default::default(),
    },
  })));

  let future = poll_fn(move |cx| {
    let mut resource_table = resource_table.borrow_mut();
    let mut stream = resource_table
      .get_mut::<WsStream>(rid)
      .ok_or_else(OpError::bad_resource_id)?;

    // TODO(ry) Handle errors below instead of unwrap.
    // Need to map tungstenite::error::Error to OpError.

    let stream_pin = Pin::new(&mut stream);
    ready!(stream_pin.poll_ready(cx)).unwrap();

    if let Some(msg) = maybe_msg.take() {
      let stream_pin = Pin::new(&mut stream);
      stream_pin.start_send(msg).unwrap();
    }

    let stream_pin = Pin::new(&mut stream);
    ready!(stream_pin.poll_flush(cx)).unwrap();

    let stream_pin = Pin::new(&mut stream);
    ready!(stream_pin.poll_close(cx)).unwrap();

    Poll::Ready(Ok(json!({})))
  });

  Ok(JsonOp::Async(future.boxed_local()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextEventArgs {
  rid: u32,
}

pub fn op_ws_next_event(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: NextEventArgs = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let future = poll_fn(move |cx| {
    let mut resource_table = resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<WsStream>(args.rid)
      .ok_or_else(OpError::bad_resource_id)?;

    /*
    match stream.rx.try_recv() {
      Ok(_) => todo!(),
      Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
      Err(tokio::sync::oneshot::error::TryRecvError::Closed) => todo!(),
    }

    if Poll::Pending != stream.rx.poll_unpin(cx) {
      todo!()
    }
    */

    stream.poll_next_unpin(cx).map(|val| {
      match val {
        Some(val) => {
          match val {
            Ok(message) => {
              match message {
                Message::Text(text) => Ok(json!({
                  "type": "string",
                  "data": text
                })),
                Message::Binary(data) => {
                  Ok(json!({ //TODO: don't use json to send binary data
                    "type": "binary",
                    "data": data
                  }))
                }
                Message::Close(frame) => {
                  if let Some(frame) = frame {
                    let code: u16 = frame.code.into();
                    Ok(json!({
                      "type": "close",
                      "code": code,
                      "reason": frame.reason.as_ref()
                    }))
                  } else {
                    Ok(json!({ "type": "close" }))
                  }
                }
                Message::Ping(_) => Ok(json!({"type": "ping"})),
                Message::Pong(_) => Ok(json!({"type": "pong"})),
              }
            }
            Err(_) => Ok(json!({
              "type": "error",
            })),
          }
        }
        None => {
          resource_table
            .close(args.rid)
            .ok_or_else(OpError::bad_resource_id)?;
          Ok(json!({
            "type": "closed",
          }))
        }
      }
    })
  });

  Ok(JsonOp::Async(future.boxed_local()))
}
