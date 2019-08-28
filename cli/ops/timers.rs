// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::time::Duration;
use std::time::Instant;

// Global Timer Stop

pub struct OpGlobalTimerStop;

impl DenoOpDispatcher for OpGlobalTimerStop {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let mut t = state.global_timer.lock().unwrap();
        t.cancel();
        Ok(JsonOp::Sync(json!({})))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "globalTimerStop";
}

// Global Timer

pub struct OpGlobalTimer;

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

impl DenoOpDispatcher for OpGlobalTimer {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: GlobalTimerArgs = serde_json::from_value(args)?;
        let val = args.timeout;

        let mut t = state.global_timer.lock().unwrap();
        let deadline = Instant::now() + Duration::from_millis(val as u64);
        let f = t
          .new_timeout(deadline)
          .then(move |_| futures::future::ok(json!({})));

        Ok(JsonOp::Async(Box::new(f)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "globalTimer";
}
