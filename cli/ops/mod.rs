// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;

mod compiler;
mod dispatch_json;
mod dispatch_minimal;
mod dispatcher;
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

use dispatch_json::{JsonDispatcher, JsonOpHandler};
use dispatch_minimal::{MinimalDispatcher, MinimalOpHandler};
use dispatcher::Dispatch;

#[allow(clippy::new_without_default)]
pub struct DispatchManager {
  pub minimal: MinimalDispatcher,
  pub json: JsonDispatcher,
}

impl DispatchManager {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let minimal_dispatcher = MinimalDispatcher::new();
    minimal_dispatcher.register_op("read", io::op_read);
    minimal_dispatcher.register_op("write", io::op_write);

    let json_dispatcher = JsonDispatcher::new();
    json_dispatcher.register_op("exit", os::op_exit);
    json_dispatcher.register_op("is_tty", os::op_is_tty);
    json_dispatcher.register_op("env", os::op_env);
    json_dispatcher.register_op("exec_path", os::op_exec_path);
    json_dispatcher.register_op("home_dir", os::op_home_dir);
    json_dispatcher.register_op("utime", fs::op_utime);
    json_dispatcher.register_op("set_env", os::op_set_env);
    json_dispatcher.register_op("start", os::op_start);
    json_dispatcher
      .register_op("apply_source_map", errors::op_apply_source_map);
    json_dispatcher.register_op("format_error", errors::op_format_error);
    json_dispatcher.register_op("cache", compiler::op_cache);
    json_dispatcher
      .register_op("fetch_source_file", compiler::op_fetch_source_file);
    json_dispatcher.register_op("open", files::op_open);
    json_dispatcher.register_op("close", files::op_close);
    json_dispatcher.register_op("seek", files::op_seek);
    json_dispatcher.register_op("metrics", metrics::op_metrics);
    json_dispatcher.register_op("fetch", fetch::op_fetch);
    json_dispatcher.register_op("repl_start", repl::op_repl_start);
    json_dispatcher.register_op("repl_readline", repl::op_repl_readline);
    json_dispatcher.register_op("accept", net::op_accept);
    json_dispatcher.register_op("dial", net::op_dial);
    json_dispatcher.register_op("shutdown", net::op_shutdown);
    json_dispatcher.register_op("listen", net::op_listen);
    json_dispatcher.register_op("resources", resources::op_resources);
    json_dispatcher
      .register_op("get_random_values", random::op_get_random_values);
    json_dispatcher
      .register_op("global_timer_stop", timers::op_global_timer_stop);
    json_dispatcher.register_op("global_timer", timers::op_global_timer);
    json_dispatcher.register_op("now", performance::op_now);
    json_dispatcher.register_op("permissions", permissions::op_permissions);
    json_dispatcher
      .register_op("revoke_permission", permissions::op_revoke_permission);
    json_dispatcher.register_op("create_worker", workers::op_create_worker);
    json_dispatcher.register_op(
      "host_get_worker_closed",
      workers::op_host_get_worker_closed,
    );
    json_dispatcher
      .register_op("host_post_message", workers::op_host_post_message);
    json_dispatcher
      .register_op("host_get_message", workers::op_host_get_message);
    // TODO: make sure these two ops are only accessible to appropriate Worker
    json_dispatcher
      .register_op("worker_post_message", workers::op_worker_post_message);
    json_dispatcher
      .register_op("worker_get_message", workers::op_worker_get_message);
    json_dispatcher.register_op("run", process::op_run);
    json_dispatcher.register_op("run_status", process::op_run_status);
    json_dispatcher.register_op("kill", process::op_kill);
    json_dispatcher.register_op("chdir", fs::op_chdir);
    json_dispatcher.register_op("mkdir", fs::op_mkdir);
    json_dispatcher.register_op("chmod", fs::op_chmod);
    json_dispatcher.register_op("chown", fs::op_chown);
    json_dispatcher.register_op("remove", fs::op_remove);
    json_dispatcher.register_op("copy_file", fs::op_copy_file);
    json_dispatcher.register_op("stat", fs::op_stat);
    json_dispatcher.register_op("read_dir", fs::op_read_dir);
    json_dispatcher.register_op("rename", fs::op_rename);
    json_dispatcher.register_op("link", fs::op_link);
    json_dispatcher.register_op("symlink", fs::op_symlink);
    json_dispatcher.register_op("read_link", fs::op_read_link);
    json_dispatcher.register_op("truncate", fs::op_truncate);
    json_dispatcher.register_op("make_temp_dir", fs::op_make_temp_dir);
    json_dispatcher.register_op("cwd", fs::op_cwd);
    json_dispatcher.register_op("fetch_asset", compiler::op_fetch_asset);

    Self {
      minimal: minimal_dispatcher,
      json: json_dispatcher,
    }
  }

  pub fn register_minimal_op(
    &self,
    name: &str,
    handler: MinimalOpHandler,
  ) -> OpId {
    self.minimal.register_op(name, handler)
  }

  pub fn register_json_op(&self, name: &str, handler: JsonOpHandler) -> OpId {
    self.json.register_op(name, handler)
  }

  pub fn dispatch(
    &self,
    state: &ThreadSafeState,
    op_id: OpId,
    control: &[u8],
    zero_copy: Option<PinnedBuf>,
  ) -> CoreOp {
    let bytes_sent_control = control.len();
    let bytes_sent_zero_copy = zero_copy.as_ref().map(|b| b.len()).unwrap_or(0);

    let op = match op_id / 1000 {
      1 => self.minimal.dispatch(op_id, state, control, zero_copy),
      2 => self.json.dispatch(op_id, state, control, zero_copy),
      _ => panic!("unknown dispatch!"),
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
}
