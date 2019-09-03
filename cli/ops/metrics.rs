// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::state::ThreadSafeState;
use deno::*;
use std::sync::atomic::Ordering;

pub fn op_metrics(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let m = &state.metrics;

  Ok(JsonOp::Sync(json!({
    "opsDispatched": m.ops_dispatched.load(Ordering::SeqCst) as u64,
    "opsCompleted": m.ops_completed.load(Ordering::SeqCst) as u64,
    "bytesSentControl": m.bytes_sent_control.load(Ordering::SeqCst) as u64,
    "bytesSentData": m.bytes_sent_data.load(Ordering::SeqCst) as u64,
    "bytesReceived": m.bytes_received.load(Ordering::SeqCst) as u64
  })))
}
