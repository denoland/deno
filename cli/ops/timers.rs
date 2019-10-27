// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::time::Duration;
use std::time::Instant;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "global_timer_stop",
    s.core_op(json_op(s.stateful_op(op_global_timer_stop))),
  );
  i.register_op(
    "global_timer",
    s.core_op(json_op(s.stateful_op(op_global_timer))),
  );
  i.register_op("now", s.core_op(json_op(s.stateful_op(op_now))));
}

fn op_global_timer_stop(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  t.cancel();
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

fn op_global_timer(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  let deadline = Instant::now() + Duration::from_millis(val);
  let f = t
    .new_timeout(deadline)
    .then(move |_| futures::future::ok(json!({})));

  Ok(JsonOp::Async(Box::new(f)))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let seconds = state.start_time.elapsed().as_secs();
  let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.permissions.allow_hrtime.is_allow() {
    subsec_nanos -= subsec_nanos % reduced_time_precision
  }

  Ok(JsonOp::Sync(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  })))
}
