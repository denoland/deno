// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, JsonOp};
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;
use std::sync::atomic::Ordering;

// Metrics

pub struct OpMetrics;

impl DenoOpDispatcher for OpMetrics {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let m = &state.metrics;

        Ok(JsonOp::Sync(json!({
          "opsDispatched": m.ops_dispatched.load(Ordering::SeqCst) as u64,
          "opsCompleted": m.ops_completed.load(Ordering::SeqCst) as u64,
          "bytesSentControl": m.bytes_sent_control.load(Ordering::SeqCst) as u64,
          "bytesSentData": m.bytes_sent_data.load(Ordering::SeqCst) as u64,
          "bytesReceived": m.bytes_received.load(Ordering::SeqCst) as u64
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "metrics";
}
