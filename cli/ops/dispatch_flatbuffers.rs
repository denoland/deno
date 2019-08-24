use super::utils::CliOpResult;
use crate::deno_error::GetErrorKind;
use crate::msg;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use hyper::rt::Future;

type CliDispatchFn = fn(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult;

use super::files::{op_read, op_write};

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
    msg::Any::Read => Some(op_read),
    msg::Any::Write => Some(op_write),

    _ => None,
  }
}
