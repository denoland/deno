// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::utils::*;
use crate::msg;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;

pub fn op_metrics(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::MetricsRes::create(
    builder,
    &msg::MetricsResArgs::from(&state.metrics),
  );
  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::MetricsRes,
      ..Default::default()
    },
  ))
}
