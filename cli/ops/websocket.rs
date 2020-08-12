// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::ZeroCopyBuf;
use deno_core::{CoreIsolate, CoreIsolateState};
use futures::future::FutureExt;
use std::borrow::Cow;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::{connect_async, WebSocketStream};

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_ws_create", s.stateful_json_op2(op_ws_create));
  i.register_op("op_ws_close", s.stateful_json_op2(op_ws_close));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateArgs {
  url: String,
}

pub fn op_ws_create(
  isolate_state: & mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: CreateArgs = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let future = async move {
    let mut resource_table = resource_table.borrow_mut();
    let (stream, _) = connect_async(args.url).await.unwrap();
    let rid = resource_table.add("webSocketStream", Box::new(stream));
    Ok(json!(rid))
  };
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
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: CloseArgs = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let future = async move {
    let mut resource_table = resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<WebSocketStream<TcpStream>>(args.rid)
      .ok_or_else(OpError::bad_resource_id)?;
    stream
      .close(Some(CloseFrame {
        code: CloseCode::from(args.code.unwrap_or(1005)),
        reason: match args.reason {
          Some(reason) => Cow::from(reason),
          None => Default::default(),
        },
      })).await.unwrap();

    Ok(json!({}))
  };


  Ok(JsonOp::Async(future.boxed_local()))
}
