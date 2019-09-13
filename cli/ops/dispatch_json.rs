// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::ops::compiler;
use crate::ops::errors;
use crate::ops::fetch;
use crate::ops::files;
use crate::ops::fs;
use crate::ops::metrics;
use crate::ops::net;
use crate::ops::os;
use crate::ops::performance;
use crate::ops::permissions;
use crate::ops::process;
use crate::ops::random;
use crate::ops::repl;
use crate::ops::resources;
use crate::ops::timers;
use crate::ops::workers;
use crate::ops::*;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use futures::Poll;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;

pub type AsyncJsonOp = Box<dyn Future<Item = Value, Error = ErrBox> + Send>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
}

fn json_err(err: ErrBox) -> Value {
  use crate::deno_error::GetErrorKind;
  json!({
    "message": err.to_string(),
    "kind": err.kind() as u32,
  })
}

#[allow(dead_code)]
pub type JsonOpHandler = fn(
  state: &ThreadSafeState,
  args: Value,
  zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox>;

fn serialize_result(
  promise_id: Option<u64>,
  result: Result<Value, ErrBox>,
) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": json_err(err), "promiseId": promise_id }),
  };
  let mut vec = serde_json::to_vec(&value).unwrap();
  debug!("JSON response pre-align, len={}", vec.len());
  // Align to 32bit word, padding with the space character.
  vec.resize((vec.len() + 3usize) & !3usize, b' ');
  vec.into_boxed_slice()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

#[allow(dead_code)]
trait _JsonOpDispatcher {
  fn dispatch_json(
    op_id: OpId,
    state: &ThreadSafeState,
    control: &[u8],
    zero_copy: Option<PinnedBuf>,
  ) -> CoreOp;

  fn register_json_op(name: &str, handler: JsonOpHandler) -> OpId;
}

fn op_selector(op_id: OpId) -> JsonOpHandler {
  match op_id {
    OP_EXIT => os::op_exit,
    OP_IS_TTY => os::op_is_tty,
    OP_ENV => os::op_env,
    OP_EXEC_PATH => os::op_exec_path,
    OP_HOME_DIR => os::op_home_dir,
    OP_UTIME => fs::op_utime,
    OP_SET_ENV => os::op_set_env,
    OP_START => os::op_start,
    OP_APPLY_SOURCE_MAP => errors::op_apply_source_map,
    OP_FORMAT_ERROR => errors::op_format_error,
    OP_CACHE => compiler::op_cache,
    OP_FETCH_SOURCE_FILE => compiler::op_fetch_source_file,
    OP_OPEN => files::op_open,
    OP_CLOSE => files::op_close,
    OP_SEEK => files::op_seek,
    OP_METRICS => metrics::op_metrics,
    OP_FETCH => fetch::op_fetch,
    OP_REPL_START => repl::op_repl_start,
    OP_REPL_READLINE => repl::op_repl_readline,
    OP_ACCEPT => net::op_accept,
    OP_DIAL => net::op_dial,
    OP_SHUTDOWN => net::op_shutdown,
    OP_LISTEN => net::op_listen,
    OP_RESOURCES => resources::op_resources,
    OP_GET_RANDOM_VALUES => random::op_get_random_values,
    OP_GLOBAL_TIMER_STOP => timers::op_global_timer_stop,
    OP_GLOBAL_TIMER => timers::op_global_timer,
    OP_NOW => performance::op_now,
    OP_PERMISSIONS => permissions::op_permissions,
    OP_REVOKE_PERMISSION => permissions::op_revoke_permission,
    OP_CREATE_WORKER => workers::op_create_worker,
    OP_HOST_GET_WORKER_CLOSED => workers::op_host_get_worker_closed,
    OP_HOST_POST_MESSAGE => workers::op_host_post_message,
    OP_HOST_GET_MESSAGE => workers::op_host_get_message,
    // TODO: make sure these two ops are only accessible to appropriate Workers
    OP_WORKER_POST_MESSAGE => workers::op_worker_post_message,
    OP_WORKER_GET_MESSAGE => workers::op_worker_get_message,
    OP_RUN => process::op_run,
    OP_RUN_STATUS => process::op_run_status,
    OP_KILL => process::op_kill,
    OP_CHDIR => fs::op_chdir,
    OP_MKDIR => fs::op_mkdir,
    OP_CHMOD => fs::op_chmod,
    OP_CHOWN => fs::op_chown,
    OP_REMOVE => fs::op_remove,
    OP_COPY_FILE => fs::op_copy_file,
    OP_STAT => fs::op_stat,
    OP_READ_DIR => fs::op_read_dir,
    OP_RENAME => fs::op_rename,
    OP_LINK => fs::op_link,
    OP_SYMLINK => fs::op_symlink,
    OP_READ_LINK => fs::op_read_link,
    OP_TRUNCATE => fs::op_truncate,
    OP_MAKE_TEMP_DIR => fs::op_make_temp_dir,
    OP_CWD => fs::op_cwd,
    OP_FETCH_ASSET => compiler::op_fetch_asset,
    _ => panic!("bad op_id"),
  }
}

/// This is type called "OpDispatcher"
pub fn dispatch(
  op_id: OpId,
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp {
  let async_args: AsyncArgs = serde_json::from_slice(control).unwrap();
  let promise_id = async_args.promise_id;
  let is_sync = promise_id.is_none();

  // Select and run JsonOpHandler
  let handler = op_selector(op_id);
  let result = serde_json::from_slice(control)
    .map_err(ErrBox::from)
    .and_then(move |args| handler(state, args, zero_copy));

  // Convert to CoreOp
  match result {
    Ok(JsonOp::Sync(sync_value)) => {
      assert!(promise_id.is_none());
      CoreOp::Sync(serialize_result(promise_id, Ok(sync_value)))
    }
    Ok(JsonOp::Async(fut)) => {
      assert!(promise_id.is_some());
      let fut2 = Box::new(fut.then(move |result| -> Result<Buf, ()> {
        Ok(serialize_result(promise_id, result))
      }));
      CoreOp::Async(fut2)
    }
    Err(sync_err) => {
      let buf = serialize_result(promise_id, Err(sync_err));
      if is_sync {
        CoreOp::Sync(buf)
      } else {
        CoreOp::Async(Box::new(futures::future::ok(buf)))
      }
    }
  }
}

// This is just type conversion. Implement From trait?
// See https://github.com/tokio-rs/tokio/blob/ffd73a64e7ec497622b7f939e38017afe7124dc4/tokio-fs/src/lib.rs#L76-L85
fn convert_blocking_json<F>(f: F) -> Poll<Value, ErrBox>
where
  F: FnOnce() -> Result<Value, ErrBox>,
{
  use futures::Async::*;
  match tokio_threadpool::blocking(f) {
    Ok(Ready(Ok(v))) => Ok(Ready(v)),
    Ok(Ready(Err(err))) => Err(err),
    Ok(NotReady) => Ok(NotReady),
    Err(err) => panic!("blocking error {}", err),
  }
}

pub fn blocking_json<F>(is_sync: bool, f: F) -> Result<JsonOp, ErrBox>
where
  F: 'static + Send + FnOnce() -> Result<Value, ErrBox>,
{
  if is_sync {
    Ok(JsonOp::Sync(f()?))
  } else {
    Ok(JsonOp::Async(Box::new(futures::sync::oneshot::spawn(
      tokio_util::poll_fn(move || convert_blocking_json(f)),
      &tokio_executor::DefaultExecutor::current(),
    ))))
  }
}
