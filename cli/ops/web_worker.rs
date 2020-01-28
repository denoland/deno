// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std;
use std::convert::From;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "worker_post_message",
    s.core_op(json_op(s.stateful_op(op_worker_post_message))),
  );
  i.register_op(
    "worker_get_message",
    s.core_op(json_op(s.stateful_op(op_worker_get_message))),
  );
}

/// Get message from host as guest worker
fn op_worker_get_message(
  state: &ThreadSafeState,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let state_ = state.clone();
  let op = async move {
    let mut receiver = state_.worker_channels.receiver.lock().await;
    let maybe_buf = receiver.next().await;
    debug!("op_worker_get_message");
    Ok(json!({ "data": maybe_buf }))
  };

  Ok(JsonOp::Async(op.boxed()))
}

/// Post message to host as guest worker
fn op_worker_post_message(
  state: &ThreadSafeState,
  _args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();
  let mut sender = state.worker_channels.sender.clone();
  futures::executor::block_on(sender.send(d))
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

  Ok(JsonOp::Sync(json!({})))
}
