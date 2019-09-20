// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use deno::*;
use std::collections::HashMap;
use std::sync::RwLock;

mod compiler;
//mod dispatch_json;
//mod dispatch_minimal;
//mod dispatcher;
//mod serializer;
mod serializer_json;
mod serializer_minimal;
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

use serializer_minimal::serialize_minimal;
use serializer_json::serialize_json;

pub type CliOp = dyn Fn(&ThreadSafeState, &[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static;

#[derive(Default)]
pub struct OpRegistry {
  pub ops: RwLock<Vec<Box<CliOp>>>,
  pub phone_book: RwLock<HashMap<String, OpId>>,
}

impl OpRegistry {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let mut registry = OpRegistry::default();

    registry.register_op("read", serialize_minimal(io::op_read));
    registry.register_op("write", serialize_minimal(io::op_write));

    registry.register_op("exit", serialize_json(os::op_exit));
    registry.register_op("is_tty", serialize_json(os::op_is_tty));
    registry.register_op("env", serialize_json(os::op_env));
    registry.register_op("exec_path", serialize_json(os::op_exec_path));
    registry.register_op("home_dir", serialize_json(os::op_home_dir));
    registry.register_op("utime", serialize_json(fs::op_utime));
    registry.register_op("set_env", serialize_json(os::op_set_env));
    registry.register_op("start", serialize_json(os::op_start));
    registry
      .register_op("apply_source_map", serialize_json(errors::op_apply_source_map));
    registry.register_op("format_error", serialize_json(errors::op_format_error));
    registry.register_op("cache", serialize_json(compiler::op_cache));
    registry
      .register_op("fetch_source_files", serialize_json(compiler::op_fetch_source_files));
    registry.register_op("open", serialize_json(files::op_open));
    registry.register_op("close", serialize_json(files::op_close));
    registry.register_op("seek", serialize_json(files::op_seek));
    registry.register_op("metrics", serialize_json(metrics::op_metrics));
    registry.register_op("fetch", serialize_json(fetch::op_fetch));
    registry.register_op("repl_start", serialize_json(repl::op_repl_start));
    registry.register_op("repl_readline", serialize_json(repl::op_repl_readline));
    registry.register_op("accept", serialize_json(net::op_accept));
    registry.register_op("dial", serialize_json(net::op_dial));
    registry.register_op("shutdown", serialize_json(net::op_shutdown));
    registry.register_op("listen", serialize_json(net::op_listen));
    registry.register_op("resources", serialize_json(resources::op_resources));
    registry
      .register_op("get_random_values", serialize_json(random::op_get_random_values));
    registry
      .register_op("global_timer_stop", serialize_json(timers::op_global_timer_stop));
    registry.register_op("global_timer", serialize_json(timers::op_global_timer));
    registry.register_op("now", serialize_json(performance::op_now));
    registry.register_op("permissions", serialize_json(permissions::op_permissions));
    registry
      .register_op("revoke_permission", serialize_json(permissions::op_revoke_permission));
    registry.register_op("create_worker", serialize_json(workers::op_create_worker));
    registry.register_op(
      "host_get_worker_closed",
      serialize_json(workers::op_host_get_worker_closed),
    );
    registry
      .register_op("host_post_message", serialize_json(workers::op_host_post_message));
    registry
      .register_op("host_get_message", serialize_json(workers::op_host_get_message));
    // TODO: make sure these two ops are only accessible to appropriate Worker
    registry
      .register_op("worker_post_message", serialize_json(workers::op_worker_post_message));
    registry
      .register_op("worker_get_message", serialize_json(workers::op_worker_get_message));
    registry.register_op("run", serialize_json(process::op_run));
    registry.register_op("run_status", serialize_json(process::op_run_status));
    registry.register_op("kill", serialize_json(process::op_kill));
    registry.register_op("chdir", serialize_json(fs::op_chdir));
    registry.register_op("mkdir", serialize_json(fs::op_mkdir));
    registry.register_op("chmod", serialize_json(fs::op_chmod));
    registry.register_op("chown", serialize_json(fs::op_chown));
    registry.register_op("remove", serialize_json(fs::op_remove));
    registry.register_op("copy_file", serialize_json(fs::op_copy_file));
    registry.register_op("stat", serialize_json(fs::op_stat));
    registry.register_op("read_dir", serialize_json(fs::op_read_dir));
    registry.register_op("rename", serialize_json(fs::op_rename));
    registry.register_op("link", serialize_json(fs::op_link));
    registry.register_op("symlink", serialize_json(fs::op_symlink));
    registry.register_op("read_link", serialize_json(fs::op_read_link));
    registry.register_op("truncate", serialize_json(fs::op_truncate));
    registry.register_op("make_temp_dir", serialize_json(fs::op_make_temp_dir));
    registry.register_op("cwd", serialize_json(fs::op_cwd));
    registry.register_op("fetch_asset", serialize_json(compiler::op_fetch_asset));

    registry
  }

  pub fn get_op_map(&self) -> HashMap<String, OpId> {
    self.phone_book.read().unwrap().clone()
  }

  pub fn register_op(
    &mut self,
    name: &str,
    serialized_op: Box<CliOp>,
  ) -> OpId {
    // TODO: first check the phone_book and only then add to ops vector
    let mut ops = self.ops.write().unwrap();
    ops.push(serialized_op);
    let op_id = (ops.len() - 1) as u32;

    self
      .phone_book
      .write()
      .unwrap()
      .entry(name.to_string())
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(op_id);

    op_id
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

    let ops = self.ops
      .read()
      .unwrap();

    let op_handler = &*ops
      .get(op_id as usize)
      .expect("Op not found!");

//    let phone_book = self.phone_book.read().unwrap();
//    eprintln!("found op handler {} {:?}", op_id, phone_book);
    let op = op_handler(state, control, zero_copy);

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
