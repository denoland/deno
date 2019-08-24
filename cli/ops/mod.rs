// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;

mod compiler;
mod dispatch_flatbuffers;
mod dispatch_json;
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

// Warning! These values are duplicated in the TypeScript code (js/dispatch.ts),
// update with care.
pub const OP_FLATBUFFER: OpId = 44;
pub const OP_READ: OpId = 1;
pub const OP_WRITE: OpId = 2;
pub const OP_EXIT: OpId = 3;
pub const OP_IS_TTY: OpId = 4;
pub const OP_ENV: OpId = 5;
pub const OP_EXEC_PATH: OpId = 6;
pub const OP_UTIME: OpId = 7;
pub const OP_SET_ENV: OpId = 8;
pub const OP_HOME_DIR: OpId = 9;
pub const OP_START: OpId = 10;
pub const OP_APPLY_SOURCE_MAP: OpId = 11;
pub const OP_FORMAT_ERROR: OpId = 12;
pub const OP_CACHE: OpId = 13;
pub const OP_FETCH_SOURCE_FILE: OpId = 14;
pub const OP_OPEN: OpId = 15;
pub const OP_CLOSE: OpId = 16;
pub const OP_SEEK: OpId = 17;
pub const OP_FETCH: OpId = 18;

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
    OP_EXIT => dispatch_json::dispatch(os::op_exit, state, control, zero_copy),
    OP_IS_TTY => {
      dispatch_json::dispatch(os::op_is_tty, state, control, zero_copy)
    }
    OP_ENV => dispatch_json::dispatch(os::op_env, state, control, zero_copy),
    OP_EXEC_PATH => {
      dispatch_json::dispatch(os::op_exec_path, state, control, zero_copy)
    }
    OP_HOME_DIR => {
      dispatch_json::dispatch(os::op_home_dir, state, control, zero_copy)
    }
    OP_UTIME => {
      dispatch_json::dispatch(fs::op_utime, state, control, zero_copy)
    }
    OP_SET_ENV => {
      dispatch_json::dispatch(os::op_set_env, state, control, zero_copy)
    }
    OP_START => {
      dispatch_json::dispatch(os::op_start, state, control, zero_copy)
    }
    OP_APPLY_SOURCE_MAP => dispatch_json::dispatch(
      errors::op_apply_source_map,
      state,
      control,
      zero_copy,
    ),
    OP_FORMAT_ERROR => dispatch_json::dispatch(
      errors::op_format_error,
      state,
      control,
      zero_copy,
    ),
    OP_CACHE => {
      dispatch_json::dispatch(compiler::op_cache, state, control, zero_copy)
    }
    OP_FETCH_SOURCE_FILE => dispatch_json::dispatch(
      compiler::op_fetch_source_file,
      state,
      control,
      zero_copy,
    ),
    OP_OPEN => {
      dispatch_json::dispatch(files::op_open, state, control, zero_copy)
    }
    OP_CLOSE => {
      dispatch_json::dispatch(files::op_close, state, control, zero_copy)
    }
    OP_SEEK => {
      dispatch_json::dispatch(files::op_seek, state, control, zero_copy)
    }
    OP_FETCH => {
      dispatch_json::dispatch(fetch::op_fetch, state, control, zero_copy)
    }
    OP_FLATBUFFER => dispatch_flatbuffers::dispatch(state, control, zero_copy),
    _ => panic!("bad op_id"),
  };

  state.metrics_op_dispatched(bytes_sent_control, bytes_sent_zero_copy);

  match op {
    Op::Sync(buf) => {
      state.metrics_op_completed(buf.len());
      Op::Sync(buf)
    }
    Op::Async(fut) => {
      use crate::futures::Future;
      let state = state.clone();
      let result_fut = Box::new(fut.map(move |buf: Buf| {
        state.clone().metrics_op_completed(buf.len());
        buf
      }));
      Op::Async(result_fut)
    }
  }
}
