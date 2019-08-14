// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures::Future;
use std;
use std::time::Duration;
use std::time::Instant;

pub fn op_global_timer_stop(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if !base.sync() {
    return Err(deno_error::no_async_support());
  }
  assert!(data.is_none());
  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  t.cancel();
  Ok(Op::Sync(empty_buf()))
}

pub fn op_global_timer(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if base.sync() {
    return Err(deno_error::no_sync_support());
  }
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_global_timer().unwrap();
  let val = inner.timeout();
  assert!(val >= 0);

  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  let deadline = Instant::now() + Duration::from_millis(val as u64);
  let f = t.new_timeout(deadline);

  Ok(Op::Async(Box::new(f.then(move |_| {
    let builder = &mut FlatBufferBuilder::new();
    let inner =
      msg::GlobalTimerRes::create(builder, &msg::GlobalTimerResArgs {});
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::GlobalTimerRes,
        ..Default::default()
      },
    ))
  }))))
}
