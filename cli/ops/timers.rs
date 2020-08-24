// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  i.register_op(
    "op_global_timer_stop",
    s.stateful_json_op(op_global_timer_stop),
  );
  i.register_op("op_global_timer", s.stateful_json_op(op_global_timer));
  i.register_op("op_now", s.stateful_json_op(op_now));
}

fn op_global_timer_stop(
  state: &Rc<State>,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.global_timer.borrow_mut().cancel();
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

fn op_global_timer(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let deadline = Instant::now() + Duration::from_millis(val);
  let f = state
    .global_timer
    .borrow_mut()
    .new_timeout(deadline)
    .then(move |_| futures::future::ok(json!({})));

  Ok(JsonOp::Async(f.boxed_local()))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &Rc<State>,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let seconds = state.start_time.elapsed().as_secs();
  let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if let Err(op_error) = state.check_hrtime() {
    if op_error.kind_str == "PermissionDenied" {
      subsec_nanos -= subsec_nanos % reduced_time_precision;
    } else {
      return Err(op_error);
    }
  }

  Ok(JsonOp::Sync(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  })))
}
