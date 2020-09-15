// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use serde_derive::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_global_timer_stop", op_global_timer_stop);
  super::reg_json_async(rt, "op_global_timer", op_global_timer);
  super::reg_json_sync(rt, "op_now", op_now);
}

fn op_global_timer_stop(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let cli_state = super::cli_state(state);
  cli_state.global_timer.borrow_mut().cancel();
  Ok(json!({}))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

async fn op_global_timer(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let deadline = Instant::now() + Duration::from_millis(val);
  let timer_fut = {
    super::cli_state2(&state)
      .global_timer
      .borrow_mut()
      .new_timeout(deadline)
      .boxed_local()
  };
  let _ = timer_fut.await;
  Ok(json!({}))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let cli_state = super::cli_state(state);
  let seconds = cli_state.start_time.elapsed().as_secs();
  let mut subsec_nanos = cli_state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if cli_state.check_hrtime().is_err() {
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }

  Ok(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  }))
}
