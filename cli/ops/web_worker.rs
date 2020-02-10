// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::State;
use crate::worker::WorkerEvent;
use deno_core::*;
use futures;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use std;
use std::convert::From;

pub fn web_worker_op<D>(
  sender: mpsc::Sender<WorkerEvent>,
  dispatcher: D,
) -> impl Fn(Value, Option<ZeroCopyBuf>) -> Result<JsonOp, ErrBox>
where
  D: Fn(
    &mpsc::Sender<WorkerEvent>,
    Value,
    Option<ZeroCopyBuf>,
  ) -> Result<JsonOp, ErrBox>,
{
  move |args: Value, zero_copy: Option<ZeroCopyBuf>| -> Result<JsonOp, ErrBox> {
    dispatcher(&sender, args, zero_copy)
  }
}

pub fn init(i: &mut Isolate, s: &State, sender: &mpsc::Sender<WorkerEvent>) {
  i.register_op(
    "worker_post_message",
    s.core_op(json_op(web_worker_op(
      sender.clone(),
      op_worker_post_message,
    ))),
  );
  i.register_op(
    "worker_close",
    s.core_op(json_op(web_worker_op(sender.clone(), op_worker_close))),
  );
}

/// Post message to host as guest worker
fn op_worker_post_message(
  sender: &mpsc::Sender<WorkerEvent>,
  _args: Value,
  data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();
  let mut sender = sender.clone();
  let fut = sender.send(WorkerEvent::Message(d));
  futures::executor::block_on(fut).expect("Failed to post message to host");
  Ok(JsonOp::Sync(json!({})))
}

/// Notify host that guest worker closes
fn op_worker_close(
  sender: &mpsc::Sender<WorkerEvent>,
  _args: Value,
  _data: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let mut sender = sender.clone();
  sender.close_channel();
  Ok(JsonOp::Sync(json!({})))
}
