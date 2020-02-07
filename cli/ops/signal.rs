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
use futures::future::{poll_fn, FutureExt};
#[cfg(unix)]
use serde_json;
#[cfg(unix)]
use std::task::Waker;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "signal_bind",
    s.core_op(json_op(s.stateful_op(op_signal_bind))),
  );
  i.register_op(
    "signal_unbind",
    s.core_op(json_op(s.stateful_op(op_signal_unbind))),
  );
  i.register_op(
    "signal_poll",
    s.core_op(json_op(s.stateful_op(op_signal_poll))),
  );
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
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
fn op_signal_poll(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SignalArgs = serde_json::from_value(args)?;
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

  Ok(JsonOp::AsyncUnref(future.boxed_local()))
}

#[cfg(unix)]
pub fn op_signal_unbind(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let mut table = state.lock_resource_table();
  let resource = table.get::<SignalStreamResource>(rid);
  if let Some(signal) = resource {
    if let Some(waker) = &signal.1 {
      // Wakes up the pending poll if exists.
      // This prevents the poll future from getting stuck forever.
      waker.clone().wake();
    }
  }
  table.close(rid).ok_or_else(bad_resource)?;
  Ok(JsonOp::Sync(json!({})))
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_poll(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  unimplemented!();
}
