// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;

#[cfg(unix)]
use super::dispatch_json::Deserialize;
#[cfg(unix)]
use futures::future::{poll_fn, FutureExt};
#[cfg(unix)]
use std::task::Waker;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_signal_bind", s.stateful_json_op2(op_signal_bind));
  i.register_op("op_signal_unbind", s.stateful_json_op2(op_signal_unbind));
  i.register_op("op_signal_poll", s.stateful_json_op2(op_signal_poll));
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
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.signal");
  let args: BindSignalArgs = serde_json::from_value(args)?;
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let rid = resource_table.add(
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
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.signal");
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let resource_table = isolate_state.resource_table.clone();

  let future = poll_fn(move |cx| {
    let mut resource_table = resource_table.borrow_mut();
    if let Some(mut signal) =
      resource_table.get_mut::<SignalStreamResource>(rid)
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
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.signal");
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let resource = resource_table.get::<SignalStreamResource>(rid);
  if let Some(signal) = resource {
    if let Some(waker) = &signal.1 {
      // Wakes up the pending poll if exists.
      // This prevents the poll future from getting stuck forever.
      waker.clone().wake();
    }
  }
  resource_table
    .close(rid)
    .ok_or_else(OpError::bad_resource_id)?;
  Ok(JsonOp::Sync(json!({})))
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _isolate_state: &mut CoreIsolateState,
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _isolate_state: &mut CoreIsolateState,
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_poll(
  _isolate_state: &mut CoreIsolateState,
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  unimplemented!();
}
