// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;

mod compiler;
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
mod workers;

// Warning! These values are duplicated in the TypeScript code (js/dispatch.ts),
// update with care.
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
pub const OP_FETCH_SOURCE_FILES: OpId = 14;
pub const OP_OPEN: OpId = 15;
pub const OP_CLOSE: OpId = 16;
pub const OP_SEEK: OpId = 17;
pub const OP_FETCH: OpId = 18;
pub const OP_METRICS: OpId = 19;
pub const OP_REPL_START: OpId = 20;
pub const OP_REPL_READLINE: OpId = 21;
pub const OP_ACCEPT: OpId = 22;
pub const OP_DIAL: OpId = 23;
pub const OP_SHUTDOWN: OpId = 24;
pub const OP_LISTEN: OpId = 25;
pub const OP_RESOURCES: OpId = 26;
pub const OP_GET_RANDOM_VALUES: OpId = 27;
pub const OP_GLOBAL_TIMER_STOP: OpId = 28;
pub const OP_GLOBAL_TIMER: OpId = 29;
pub const OP_NOW: OpId = 30;
pub const OP_PERMISSIONS: OpId = 31;
pub const OP_REVOKE_PERMISSION: OpId = 32;
pub const OP_CREATE_WORKER: OpId = 33;
pub const OP_HOST_GET_WORKER_CLOSED: OpId = 34;
pub const OP_HOST_POST_MESSAGE: OpId = 35;
pub const OP_HOST_GET_MESSAGE: OpId = 36;
pub const OP_WORKER_POST_MESSAGE: OpId = 37;
pub const OP_WORKER_GET_MESSAGE: OpId = 38;
pub const OP_RUN: OpId = 39;
pub const OP_RUN_STATUS: OpId = 40;
pub const OP_KILL: OpId = 41;
pub const OP_CHDIR: OpId = 42;
pub const OP_MKDIR: OpId = 43;
pub const OP_CHMOD: OpId = 44;
pub const OP_CHOWN: OpId = 45;
pub const OP_REMOVE: OpId = 46;
pub const OP_COPY_FILE: OpId = 47;
pub const OP_STAT: OpId = 48;
pub const OP_READ_DIR: OpId = 49;
pub const OP_RENAME: OpId = 50;
pub const OP_LINK: OpId = 51;
pub const OP_SYMLINK: OpId = 52;
pub const OP_READ_LINK: OpId = 53;
pub const OP_TRUNCATE: OpId = 54;
pub const OP_MAKE_TEMP_DIR: OpId = 55;
pub const OP_CWD: OpId = 56;
pub const OP_FETCH_ASSET: OpId = 57;

pub type OpDispatcher = fn(
  op_id: OpId,
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp;

fn dispatch_selector(op_id: OpId) -> OpDispatcher {
  match op_id {
    | OP_READ
    | OP_WRITE => dispatch_minimal::dispatch,
    | OP_EXIT
    | OP_IS_TTY
    | OP_ENV
    | OP_EXEC_PATH
    | OP_HOME_DIR
    | OP_UTIME
    | OP_SET_ENV
    | OP_START
    | OP_APPLY_SOURCE_MAP
    | OP_FORMAT_ERROR
    | OP_CACHE
    | OP_FETCH_SOURCE_FILE
    | OP_OPEN
    | OP_CLOSE
    | OP_SEEK
    | OP_METRICS
    | OP_FETCH
    | OP_REPL_START
    | OP_REPL_READLINE
    | OP_ACCEPT
    | OP_DIAL
    | OP_SHUTDOWN
    | OP_LISTEN
    | OP_RESOURCES
    | OP_GET_RANDOM_VALUES
    | OP_GLOBAL_TIMER_STOP
    | OP_GLOBAL_TIMER
    | OP_NOW
    | OP_PERMISSIONS
    | OP_REVOKE_PERMISSION
    | OP_CREATE_WORKER
    | OP_HOST_GET_WORKER_CLOSED
    | OP_HOST_POST_MESSAGE
    | OP_HOST_GET_MESSAGE
    // TODO: make sure these two ops are only accessible to appropriate Workers
    | OP_WORKER_POST_MESSAGE
    | OP_WORKER_GET_MESSAGE
    | OP_RUN
    | OP_RUN_STATUS
    | OP_KILL
    | OP_CHDIR
    | OP_MKDIR
    | OP_CHMOD
    | OP_CHOWN
    | OP_REMOVE
    | OP_COPY_FILE
    | OP_STAT
    | OP_READ_DIR
    | OP_RENAME
    | OP_LINK
    | OP_SYMLINK
    | OP_READ_LINK
    | OP_TRUNCATE
    | OP_MAKE_TEMP_DIR
    | OP_CWD
    | OP_FETCH_ASSET => dispatch_json::dispatch,
    _ => panic!("Unknown dispatcher for op id {:?}", op_id),
  }
}

/// This is main cli dispatch
pub fn dispatch(
  state: &ThreadSafeState,
  op_id: OpId,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp {
  let bytes_sent_control = control.len();
  let bytes_sent_zero_copy = zero_copy.as_ref().map(|b| b.len()).unwrap_or(0);

  let dispatcher = dispatch_selector(op_id);
  let op = dispatcher(op_id, state, control, zero_copy);
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
