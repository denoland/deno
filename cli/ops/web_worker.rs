// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::State;
use crate::worker::WorkerEvent;
use deno_core::*;
use futures;
use std;
use std::convert::From;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "worker_post_message",
    s.core_op(json_op(s.stateful_op(op_worker_post_message))),
  );
  i.register_op(
    "worker_close",
    s.core_op(json_op(s.stateful_op(op_worker_close))),
  );
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
    .post_event(WorkerEvent::Message(d));
  futures::executor::block_on(fut).expect("Failed to post message to host");

  Ok(JsonOp::Sync(json!({})))
}

/// Notify host that guest worker closes
fn op_worker_close(
  state: &State,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let state = state.borrow();
  let channels = state.worker_channels_internal.as_ref().unwrap().clone();
  futures::executor::block_on(channels.post_event(WorkerEvent::Close))
    .expect("Failed to post message to host");
  Ok(JsonOp::Sync(json!({})))
}
