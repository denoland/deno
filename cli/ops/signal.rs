// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::*;

#[cfg(unix)]
use super::dispatch_json::Deserialize;
#[cfg(unix)]
use futures::future::{poll_fn, FutureExt};
#[cfg(unix)]
use serde_json;
#[cfg(unix)]
use std::task::Waker;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_signal_bind", s.stateful_json_op(op_signal_bind));
  i.register_op("op_signal_unbind", s.stateful_json_op(op_signal_unbind));
  i.register_op("op_signal_poll", s.stateful_json_op(op_signal_poll));
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
pub struct SignalStreamResource(pub Signal, pub Option<Waker>);

#[cfg(unix)]
#[derive(Deserialize)]
struct BindSignalArgs {
  signo: i32,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct SignalArgs {
  rid: i32,
}

#[cfg(unix)]
fn op_signal_bind(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: BindSignalArgs = serde_json::from_value(args)?;
  let mut state = state.borrow_mut();
  let rid = state.resource_table.add(
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
fn op_signal_poll(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let state_ = state.clone();

  let future = poll_fn(move |cx| {
    let mut state = state_.borrow_mut();
    if let Some(mut signal) =
      state.resource_table.get_mut::<SignalStreamResource>(rid)
    {
      signal.1 = Some(cx.waker().clone());
      return signal.0.poll_recv(cx);
    }
    std::task::Poll::Ready(None)
  })
  .then(|result| async move { Ok(json!({ "done": result.is_none() })) });

  Ok(JsonOp::AsyncUnref(future.boxed_local()))
}

#[cfg(unix)]
pub fn op_signal_unbind(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let mut state = state.borrow_mut();
  let resource = state.resource_table.get::<SignalStreamResource>(rid);
  if let Some(signal) = resource {
    if let Some(waker) = &signal.1 {
      // Wakes up the pending poll if exists.
      // This prevents the poll future from getting stuck forever.
      waker.clone().wake();
    }
  }
  state
    .resource_table
    .close(rid)
    .ok_or_else(OpError::bad_resource_id)?;
  Ok(JsonOp::Sync(json!({})))
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_poll(
  _state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  unimplemented!();
}
