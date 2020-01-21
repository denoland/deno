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
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

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

struct GetMessageFuture {
  state: ThreadSafeState,
}

impl Future for GetMessageFuture {
  type Output = Option<Buf>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let mut channels = inner.state.worker_channels.lock().unwrap();
    let receiver = &mut channels.receiver;
    receiver.poll_next_unpin(cx)
  }
}

/// Get message from host as guest worker
fn op_worker_get_message(
  state: &ThreadSafeState,
  _args: Value,
  _data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let op = GetMessageFuture {
    state: state.clone(),
  };

  let op = async move {
    let maybe_buf = op.await;
    debug!("op_worker_get_message");
    Ok(json!({ "data": maybe_buf }))
  };

  Ok(JsonOp::Async(op.boxed()))
}

/// Post message to host as guest worker
fn op_worker_post_message(
  state: &ThreadSafeState,
  _args: Value,
  data: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();
  let mut channels = state.worker_channels.lock().unwrap();
  let sender = &mut channels.sender;
  futures::executor::block_on(sender.send(d))
    .map_err(|e| DenoError::new(ErrorKind::Other, e.to_string()))?;

  Ok(JsonOp::Sync(json!({})))
}
