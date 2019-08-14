// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::GetErrorKind;
use crate::msg;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures;
use futures::Poll;
use hyper;
use hyper::rt::Future;
use tokio_threadpool;

mod dispatch_minimal;
mod io;

mod compiler;
use compiler::{op_cache, op_fetch_source_file};
mod errors;
use errors::{op_apply_source_map, op_format_error};
mod files;
use files::{op_close, op_open, op_seek};
mod fetch;
use fetch::op_fetch;
mod fs;
use fs::{
  op_chdir, op_chmod, op_chown, op_copy_file, op_cwd, op_link,
  op_make_temp_dir, op_mkdir, op_read_dir, op_read_link, op_remove, op_rename,
  op_stat, op_symlink, op_truncate, op_utime,
};
mod metrics;
use metrics::op_metrics;
mod net;
use net::{op_accept, op_dial, op_listen, op_shutdown};
mod os;
use os::{
  op_env, op_exec_path, op_exit, op_home_dir, op_is_tty, op_set_env, op_start,
};
mod performance;
use performance::op_now;
mod permissions;
use permissions::{op_permissions, op_revoke_permission};
mod process;
use process::{op_kill, op_run, op_run_status};
mod random;
use random::op_get_random_values;
mod repl;
use repl::{op_repl_readline, op_repl_start};
mod resources;
use resources::op_resources;
mod timers;
use timers::{op_global_timer, op_global_timer_stop};
mod workers;
use workers::{
  op_create_worker, op_host_get_message, op_host_get_worker_closed,
  op_host_post_message, op_worker_get_message, op_worker_post_message,
};

type CliOpResult = OpResult<ErrBox>;

type CliDispatchFn = fn(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult;

pub type OpSelector = fn(inner_type: msg::Any) -> Option<CliDispatchFn>;

#[inline]
fn empty_buf() -> Buf {
  Box::new([])
}

const FLATBUFFER_OP_ID: OpId = 44;
const OP_READ: OpId = 1;
const OP_WRITE: OpId = 2;

pub fn dispatch_all(
  state: &ThreadSafeState,
  op_id: OpId,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
  op_selector: OpSelector,
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
    FLATBUFFER_OP_ID => {
      dispatch_all_legacy(state, control, zero_copy, op_selector)
    }
    _ => panic!("bad op_id"),
  };
  state.metrics_op_dispatched(bytes_sent_control, bytes_sent_zero_copy);
  op
}

/// Processes raw messages from JavaScript.
/// This functions invoked every time Deno.core.dispatch() is called.
/// control corresponds to the first argument of Deno.core.dispatch().
/// data corresponds to the second argument of Deno.core.dispatch().
pub fn dispatch_all_legacy(
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
  op_selector: OpSelector,
) -> CoreOp {
  let base = msg::get_root_as_base(&control);
  let inner_type = base.inner_type();
  let is_sync = base.sync();
  let cmd_id = base.cmd_id();

  debug!(
    "msg_from_js {} sync {}",
    msg::enum_name_any(inner_type),
    is_sync
  );

  let op_func: CliDispatchFn = match op_selector(inner_type) {
    Some(v) => v,
    None => panic!("Unhandled message {}", msg::enum_name_any(inner_type)),
  };

  let op_result = op_func(state, &base, zero_copy);

  let state = state.clone();

  match op_result {
    Ok(Op::Sync(buf)) => {
      state.metrics_op_completed(buf.len());
      Op::Sync(buf)
    }
    Ok(Op::Async(fut)) => {
      let result_fut = Box::new(
        fut
          .or_else(move |err: ErrBox| -> Result<Buf, ()> {
            debug!("op err {}", err);
            // No matter whether we got an Err or Ok, we want a serialized message to
            // send back. So transform the DenoError into a Buf.
            let builder = &mut FlatBufferBuilder::new();
            let errmsg_offset = builder.create_string(&format!("{}", err));
            Ok(serialize_response(
              cmd_id,
              builder,
              msg::BaseArgs {
                error: Some(errmsg_offset),
                error_kind: err.kind(),
                ..Default::default()
              },
            ))
          })
          .and_then(move |buf: Buf| -> Result<Buf, ()> {
            // Handle empty responses. For sync responses we just want
            // to send null. For async we want to send a small message
            // with the cmd_id.
            let buf = if buf.len() > 0 {
              buf
            } else {
              let builder = &mut FlatBufferBuilder::new();
              serialize_response(
                cmd_id,
                builder,
                msg::BaseArgs {
                  ..Default::default()
                },
              )
            };
            state.metrics_op_completed(buf.len());
            Ok(buf)
          })
          .map_err(|err| panic!("unexpected error {:?}", err)),
      );
      Op::Async(result_fut)
    }
    Err(err) => {
      debug!("op err {}", err);
      // No matter whether we got an Err or Ok, we want a serialized message to
      // send back. So transform the DenoError into a Buf.
      let builder = &mut FlatBufferBuilder::new();
      let errmsg_offset = builder.create_string(&format!("{}", err));
      let response_buf = serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          error: Some(errmsg_offset),
          error_kind: err.kind(),
          ..Default::default()
        },
      );
      state.metrics_op_completed(response_buf.len());
      Op::Sync(response_buf)
    }
  }
}

/// Standard ops set for most isolates
pub fn op_selector_std(inner_type: msg::Any) -> Option<CliDispatchFn> {
  match inner_type {
    msg::Any::Accept => Some(op_accept),
    msg::Any::ApplySourceMap => Some(op_apply_source_map),
    msg::Any::Cache => Some(op_cache),
    msg::Any::Chdir => Some(op_chdir),
    msg::Any::Chmod => Some(op_chmod),
    msg::Any::Chown => Some(op_chown),
    msg::Any::Close => Some(op_close),
    msg::Any::CopyFile => Some(op_copy_file),
    msg::Any::CreateWorker => Some(op_create_worker),
    msg::Any::Cwd => Some(op_cwd),
    msg::Any::Dial => Some(op_dial),
    msg::Any::Environ => Some(op_env),
    msg::Any::ExecPath => Some(op_exec_path),
    msg::Any::Exit => Some(op_exit),
    msg::Any::Fetch => Some(op_fetch),
    msg::Any::FetchSourceFile => Some(op_fetch_source_file),
    msg::Any::FormatError => Some(op_format_error),
    msg::Any::GetRandomValues => Some(op_get_random_values),
    msg::Any::GlobalTimer => Some(op_global_timer),
    msg::Any::GlobalTimerStop => Some(op_global_timer_stop),
    msg::Any::HostGetMessage => Some(op_host_get_message),
    msg::Any::HostGetWorkerClosed => Some(op_host_get_worker_closed),
    msg::Any::HostPostMessage => Some(op_host_post_message),
    msg::Any::IsTTY => Some(op_is_tty),
    msg::Any::Kill => Some(op_kill),
    msg::Any::Link => Some(op_link),
    msg::Any::Listen => Some(op_listen),
    msg::Any::MakeTempDir => Some(op_make_temp_dir),
    msg::Any::Metrics => Some(op_metrics),
    msg::Any::Mkdir => Some(op_mkdir),
    msg::Any::Now => Some(op_now),
    msg::Any::Open => Some(op_open),
    msg::Any::PermissionRevoke => Some(op_revoke_permission),
    msg::Any::Permissions => Some(op_permissions),
    msg::Any::ReadDir => Some(op_read_dir),
    msg::Any::Readlink => Some(op_read_link),
    msg::Any::Remove => Some(op_remove),
    msg::Any::Rename => Some(op_rename),
    msg::Any::ReplReadline => Some(op_repl_readline),
    msg::Any::ReplStart => Some(op_repl_start),
    msg::Any::Resources => Some(op_resources),
    msg::Any::Run => Some(op_run),
    msg::Any::RunStatus => Some(op_run_status),
    msg::Any::Seek => Some(op_seek),
    msg::Any::SetEnv => Some(op_set_env),
    msg::Any::Shutdown => Some(op_shutdown),
    msg::Any::Start => Some(op_start),
    msg::Any::Stat => Some(op_stat),
    msg::Any::Symlink => Some(op_symlink),
    msg::Any::Truncate => Some(op_truncate),
    msg::Any::HomeDir => Some(op_home_dir),
    msg::Any::Utime => Some(op_utime),

    // TODO(ry) split these out so that only the appropriate Workers can access
    // them.
    msg::Any::WorkerGetMessage => Some(op_worker_get_message),
    msg::Any::WorkerPostMessage => Some(op_worker_post_message),

    _ => None,
  }
}

fn serialize_response(
  cmd_id: u32,
  builder: &mut FlatBufferBuilder<'_>,
  mut args: msg::BaseArgs<'_>,
) -> Buf {
  args.cmd_id = cmd_id;
  let base = msg::Base::create(builder, &args);
  msg::finish_base_buffer(builder, base);
  let data = builder.finished_data();
  // println!("serialize_response {:x?}", data);
  data.into()
}

#[inline]
fn ok_buf(buf: Buf) -> CliOpResult {
  Ok(Op::Sync(buf))
}

// This is just type conversion. Implement From trait?
// See https://github.com/tokio-rs/tokio/blob/ffd73a64e7ec497622b7f939e38017afe7124dc4/tokio-fs/src/lib.rs#L76-L85
fn convert_blocking<F>(f: F) -> Poll<Buf, ErrBox>
where
  F: FnOnce() -> Result<Buf, ErrBox>,
{
  use futures::Async::*;
  match tokio_threadpool::blocking(f) {
    Ok(Ready(Ok(v))) => Ok(v.into()),
    Ok(Ready(Err(err))) => Err(err),
    Ok(NotReady) => Ok(NotReady),
    Err(err) => panic!("blocking error {}", err),
  }
}

fn blocking<F>(is_sync: bool, f: F) -> CliOpResult
where
  F: 'static + Send + FnOnce() -> Result<Buf, ErrBox>,
{
  if is_sync {
    let result_buf = f()?;
    Ok(Op::Sync(result_buf))
  } else {
    Ok(Op::Async(Box::new(futures::sync::oneshot::spawn(
      tokio_util::poll_fn(move || convert_blocking(f)),
      &tokio_executor::DefaultExecutor::current(),
    ))))
  }
}
