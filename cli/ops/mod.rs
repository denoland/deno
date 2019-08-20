// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;

mod compiler;
mod dispatch_flatbuffers;
mod dispatch_minimal;
mod errors;
mod fetch;
mod files;
mod fs;
mod io;
mod metrics;
mod net;
mod os;
mod performance;
mod permissions;
mod process;
mod random;
mod repl;
mod resources;
mod timers;
mod utils;
mod workers;

pub const OP_FLATBUFFER: OpId = 44;
pub const OP_READ: OpId = 1;
pub const OP_WRITE: OpId = 2;

pub fn dispatch(
  state: &ThreadSafeState,
  op_id: OpId,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp {
  let bytes_sent_control = control.len();
  let bytes_sent_zero_copy = zero_copy.as_ref().map(|b| b.len()).unwrap_or(0);

  let op = match op_id {
    OP_READ => {
      dispatch_minimal::dispatch(io::op_read, state, control, zero_copy)
    }
    OP_WRITE => {
      dispatch_minimal::dispatch(io::op_write, state, control, zero_copy)
    }
    OP_FLATBUFFER => dispatch_flatbuffers::dispatch(state, control, zero_copy),
    _ => panic!("bad op_id"),
  };

  state.metrics_op_dispatched(bytes_sent_control, bytes_sent_zero_copy);
  op
}
