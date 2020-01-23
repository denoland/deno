// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno_core::*;

#[cfg(unix)]
use super::dispatch_json::Deserialize;
#[cfg(unix)]
use crate::deno_error::bad_resource;
#[cfg(unix)]
use std::task::Waker;
#[cfg(unix)]
use deno_core::Resource;
#[cfg(unix)]
use futures::future::{poll_fn, FutureExt};
#[cfg(unix)]
use serde_json;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "bind_signal",
    s.core_op(json_op(s.stateful_op(op_bind_signal))),
  );
  i.register_op(
    "unbind_signal",
    s.core_op(json_op(s.stateful_op(op_unbind_signal))),
  );
  i.register_op(
    "poll_signal",
    s.core_op(json_op(s.stateful_op(op_poll_signal))),
  );
}

#[cfg(unix)]
pub struct SignalStreamResource(pub Signal, pub Option<Waker>);

#[cfg(unix)]
impl Resource for SignalStreamResource {}

#[cfg(unix)]
#[derive(Deserialize)]
struct BindSignalArgs {
  signo: i32,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct UnbindSignalArgs {
  rid: i32,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct PollSignalArgs {
  rid: i32,
}

#[cfg(unix)]
fn op_bind_signal(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: BindSignalArgs = serde_json::from_value(args)?;
  let mut table = state.lock_resource_table();
  let rid = table.add(
    "signal",
    Box::new(SignalStreamResource(
      signal(SignalKind::from_raw(args.signo)).expect(""),
      None,
    )),
  );
  Ok(JsonOp::Sync(json!({
    "rid": rid,
  })))
}

#[cfg(unix)]
fn op_poll_signal(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PollSignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let state_ = state.clone();

  let future = poll_fn(move |cx| {
    let mut table = state_.lock_resource_table();
    if let Some(mut signal) = table.get_mut::<SignalStreamResource>(rid) {
      signal.1 = Some(cx.waker().clone());
      return signal.0.poll_recv(cx);
    }
    std::task::Poll::Ready(None)
  })
  .then(|result| async move { Ok(json!({ "done": result.is_none() })) });

  Ok(JsonOp::AsyncUnref(future.boxed()))
}

#[cfg(unix)]
pub fn op_unbind_signal(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: UnbindSignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let mut table = state.lock_resource_table();
  let resource = table.get::<SignalStreamResource>(rid);
  if let Some(signal) = resource {
    if let Some(waker) = &signal.1 {
      waker.clone().wake();
    }
  }
  table.close(rid).ok_or_else(bad_resource)?;
  Ok(JsonOp::Sync(json!({})))
}

#[cfg(not(unix))]
pub fn op_bind_signal(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_unbind_signal(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_poll_signal(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}
