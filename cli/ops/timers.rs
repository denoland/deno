// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::time::Duration;
use std::time::Instant;

pub fn op_global_timer_stop(
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

pub fn op_global_timer(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  let deadline = Instant::now() + Duration::from_millis(val as u64);
  let f = t
    .new_timeout(deadline)
    .then(move |_| futures::future::ok(json!({})));

  Ok(JsonOp::Async(Box::new(f)))
}
