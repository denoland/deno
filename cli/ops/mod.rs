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
mod tls;
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
pub const OP_DIAL_TLS: OpId = 58;

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
    OP_FETCH_SOURCE_FILES => dispatch_json::dispatch(
      compiler::op_fetch_source_files,
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
    OP_METRICS => {
      dispatch_json::dispatch(metrics::op_metrics, state, control, zero_copy)
    }
    OP_FETCH => {
      dispatch_json::dispatch(fetch::op_fetch, state, control, zero_copy)
    }
    OP_REPL_START => {
      dispatch_json::dispatch(repl::op_repl_start, state, control, zero_copy)
    }
    OP_REPL_READLINE => {
      dispatch_json::dispatch(repl::op_repl_readline, state, control, zero_copy)
    }
    OP_ACCEPT => {
      dispatch_json::dispatch(net::op_accept, state, control, zero_copy)
    }
    OP_DIAL => dispatch_json::dispatch(net::op_dial, state, control, zero_copy),
    OP_SHUTDOWN => {
      dispatch_json::dispatch(net::op_shutdown, state, control, zero_copy)
    }
    OP_LISTEN => {
      dispatch_json::dispatch(net::op_listen, state, control, zero_copy)
    }
    OP_RESOURCES => dispatch_json::dispatch(
      resources::op_resources,
      state,
      control,
      zero_copy,
    ),
    OP_GET_RANDOM_VALUES => dispatch_json::dispatch(
      random::op_get_random_values,
      state,
      control,
      zero_copy,
    ),
    OP_GLOBAL_TIMER_STOP => dispatch_json::dispatch(
      timers::op_global_timer_stop,
      state,
      control,
      zero_copy,
    ),
    OP_GLOBAL_TIMER => dispatch_json::dispatch(
      timers::op_global_timer,
      state,
      control,
      zero_copy,
    ),
    OP_NOW => {
      dispatch_json::dispatch(performance::op_now, state, control, zero_copy)
    }
    OP_PERMISSIONS => dispatch_json::dispatch(
      permissions::op_permissions,
      state,
      control,
      zero_copy,
    ),
    OP_REVOKE_PERMISSION => dispatch_json::dispatch(
      permissions::op_revoke_permission,
      state,
      control,
      zero_copy,
    ),
    OP_CREATE_WORKER => dispatch_json::dispatch(
      workers::op_create_worker,
      state,
      control,
      zero_copy,
    ),
    OP_HOST_GET_WORKER_CLOSED => dispatch_json::dispatch(
      workers::op_host_get_worker_closed,
      state,
      control,
      zero_copy,
    ),
    OP_HOST_POST_MESSAGE => dispatch_json::dispatch(
      workers::op_host_post_message,
      state,
      control,
      zero_copy,
    ),
    OP_HOST_GET_MESSAGE => dispatch_json::dispatch(
      workers::op_host_get_message,
      state,
      control,
      zero_copy,
    ),
    // TODO: make sure these two ops are only accessible to appropriate Workers
    OP_WORKER_POST_MESSAGE => dispatch_json::dispatch(
      workers::op_worker_post_message,
      state,
      control,
      zero_copy,
    ),
    OP_WORKER_GET_MESSAGE => dispatch_json::dispatch(
      workers::op_worker_get_message,
      state,
      control,
      zero_copy,
    ),
    OP_RUN => {
      dispatch_json::dispatch(process::op_run, state, control, zero_copy)
    }
    OP_RUN_STATUS => {
      dispatch_json::dispatch(process::op_run_status, state, control, zero_copy)
    }
    OP_KILL => {
      dispatch_json::dispatch(process::op_kill, state, control, zero_copy)
    }
    OP_CHDIR => {
      dispatch_json::dispatch(fs::op_chdir, state, control, zero_copy)
    }
    OP_MKDIR => {
      dispatch_json::dispatch(fs::op_mkdir, state, control, zero_copy)
    }
    OP_CHMOD => {
      dispatch_json::dispatch(fs::op_chmod, state, control, zero_copy)
    }
    OP_CHOWN => {
      dispatch_json::dispatch(fs::op_chown, state, control, zero_copy)
    }
    OP_REMOVE => {
      dispatch_json::dispatch(fs::op_remove, state, control, zero_copy)
    }
    OP_COPY_FILE => {
      dispatch_json::dispatch(fs::op_copy_file, state, control, zero_copy)
    }
    OP_STAT => dispatch_json::dispatch(fs::op_stat, state, control, zero_copy),
    OP_READ_DIR => {
      dispatch_json::dispatch(fs::op_read_dir, state, control, zero_copy)
    }
    OP_RENAME => {
      dispatch_json::dispatch(fs::op_rename, state, control, zero_copy)
    }
    OP_LINK => dispatch_json::dispatch(fs::op_link, state, control, zero_copy),
    OP_SYMLINK => {
      dispatch_json::dispatch(fs::op_symlink, state, control, zero_copy)
    }
    OP_READ_LINK => {
      dispatch_json::dispatch(fs::op_read_link, state, control, zero_copy)
    }
    OP_TRUNCATE => {
      dispatch_json::dispatch(fs::op_truncate, state, control, zero_copy)
    }
    OP_MAKE_TEMP_DIR => {
      dispatch_json::dispatch(fs::op_make_temp_dir, state, control, zero_copy)
    }
    OP_CWD => dispatch_json::dispatch(fs::op_cwd, state, control, zero_copy),
    OP_FETCH_ASSET => dispatch_json::dispatch(
      compiler::op_fetch_asset,
      state,
      control,
      zero_copy,
    ),
    OP_DIAL_TLS => {
      dispatch_json::dispatch(tls::op_dial_tls, state, control, zero_copy)
    }
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
