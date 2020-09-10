// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use serde_derive::Deserialize;
use serde_json::Value;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_global_timer_stop", op_global_timer_stop);
  s.register_op_json_async("op_global_timer", op_global_timer);
  s.register_op_json_sync("op_now", op_now);
}

fn op_global_timer_stop(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.global_timer.borrow_mut().cancel();
  Ok(json!({}))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

async fn op_global_timer(
  state: Rc<State>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let deadline = Instant::now() + Duration::from_millis(val);
  let timer_fut = state
    .global_timer
    .borrow_mut()
    .new_timeout(deadline)
    .boxed_local();
  let _ = timer_fut.await;
  Ok(json!({}))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let seconds = state.start_time.elapsed().as_secs();
  let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if state.check_hrtime().is_err() {
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }

  Ok(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  }))
}
