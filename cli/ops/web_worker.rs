// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::ops::json_op;
use crate::state::State;
use deno_core::*;
use futures;
use futures::future::FutureExt;
use std;
use std::convert::From;

pub fn init(i: &mut Isolate, s: &State) {
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
  state: &State,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let state_ = state.clone();
  let op = async move {
    let fut = {
      let state = state_.borrow();
      state
        .worker_channels_internal
        .as_ref()
        .unwrap()
        .get_message()
    };
    let maybe_buf = fut.await;
    debug!("op_worker_get_message");
    Ok(json!({ "data": maybe_buf }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

/// Post message to host as guest worker
fn op_worker_post_message(
  state: &State,
  _args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();
  let state = state.borrow();
  let fut = state
    .worker_channels_internal
    .as_ref()
    .unwrap()
    .post_message(d);
  futures::executor::block_on(fut)
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

  Ok(JsonOp::Sync(json!({})))
}
