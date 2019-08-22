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
  Ok(JsonOp::Sync(json!({
    "opsDispatched": state.metrics.ops_dispatched.load(Ordering::SeqCst) as u64,
    "opsCompleted": state.metrics.ops_completed.load(Ordering::SeqCst) as u64,
    "bytesSentControl": state.metrics.bytes_sent_control.load(Ordering::SeqCst) as u64,
    "bytesSentData": state.metrics.bytes_sent_data.load(Ordering::SeqCst) as u64,
    "bytesReceived": state.metrics.bytes_received.load(Ordering::SeqCst) as u64,
  })))
}
