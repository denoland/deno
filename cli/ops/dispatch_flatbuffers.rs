use super::utils::CliOpResult;
use crate::deno_error::GetErrorKind;
use crate::msg;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use hyper::rt::Future;

use super::compiler::{op_cache, op_fetch_source_file};
use super::errors::{op_apply_source_map, op_format_error};
use super::fetch::op_fetch;
use super::files::{op_close, op_open, op_read, op_seek, op_write};
use super::fs::{
  op_chdir, op_chmod, op_chown, op_copy_file, op_cwd, op_link,
  op_make_temp_dir, op_mkdir, op_read_dir, op_read_link, op_remove, op_rename,
  op_stat, op_symlink, op_truncate,
};
use super::metrics::op_metrics;
use super::net::{op_accept, op_dial, op_listen, op_shutdown};
use super::os::{op_home_dir, op_set_env, op_start};
use super::performance::op_now;
use super::permissions::{op_permissions, op_revoke_permission};
use super::process::{op_kill, op_run, op_run_status};
use super::random::op_get_random_values;
use super::repl::{op_repl_readline, op_repl_start};
use super::resources::op_resources;
use super::timers::{op_global_timer, op_global_timer_stop};
use super::workers::{
  op_create_worker, op_host_get_message, op_host_get_worker_closed,
  op_host_post_message, op_worker_get_message, op_worker_post_message,
};

type CliDispatchFn = fn(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult;

/// Processes raw messages from JavaScript.
/// This functions invoked every time Deno.core.dispatch() is called.
/// control corresponds to the first argument of Deno.core.dispatch().
/// data corresponds to the second argument of Deno.core.dispatch().
pub fn dispatch(
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
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

  let op_func: CliDispatchFn = match op_selector_std(inner_type) {
    Some(v) => v,
    None => panic!("Unhandled message {}", msg::enum_name_any(inner_type)),
  };

  let op_result = op_func(state, &base, zero_copy);

  match op_result {
    Ok(Op::Sync(buf)) => Op::Sync(buf),
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
      Op::Sync(response_buf)
    }
  }
}

pub fn serialize_response(
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
    msg::Any::Fetch => Some(op_fetch),
    msg::Any::FetchSourceFile => Some(op_fetch_source_file),
    msg::Any::FormatError => Some(op_format_error),
    msg::Any::GetRandomValues => Some(op_get_random_values),
    msg::Any::GlobalTimer => Some(op_global_timer),
    msg::Any::GlobalTimerStop => Some(op_global_timer_stop),
    msg::Any::HostGetMessage => Some(op_host_get_message),
    msg::Any::HostGetWorkerClosed => Some(op_host_get_worker_closed),
    msg::Any::HostPostMessage => Some(op_host_post_message),
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
    msg::Any::Read => Some(op_read),
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
    msg::Any::Write => Some(op_write),

    // TODO(ry) split these out so that only the appropriate Workers can access
    // them.
    msg::Any::WorkerGetMessage => Some(op_worker_get_message),
    msg::Any::WorkerPostMessage => Some(op_worker_post_message),

    _ => None,
  }
}
