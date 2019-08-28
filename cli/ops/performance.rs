// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, JsonOp};
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;

// Now

/// Returns a milliseconds and nanoseconds subsec
/// since the start time of the deno runtime.
/// If the High precision flag is not set, the
/// nanoseconds are rounded on 2ms.
pub struct OpNow;

impl DenoOpDispatcher for OpNow {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let seconds = state.start_time.elapsed().as_secs();
        let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
        let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

        // If the permission is not enabled
        // Round the nano result on 2 milliseconds
        // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
        if !state.permissions.allows_hrtime() {
          subsec_nanos -= subsec_nanos % reduced_time_precision
        }

        Ok(JsonOp::Sync(json!({
          "seconds": seconds,
          "subsecNanos": subsec_nanos,
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "now";
}
