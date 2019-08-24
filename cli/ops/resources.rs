// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::utils::ok_buf;
use super::utils::CliOpResult;
use crate::msg;
use crate::resources::table_entries;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;

pub fn op_resources(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let builder = &mut FlatBufferBuilder::new();
  let serialized_resources = table_entries();

  let res: Vec<_> = serialized_resources
    .iter()
    .map(|(key, value)| {
      let repr = builder.create_string(value);

      msg::Resource::create(
        builder,
        &msg::ResourceArgs {
          rid: *key,
          repr: Some(repr),
        },
      )
    })
    .collect();

  let resources = builder.create_vector(&res);
  let inner = msg::ResourcesRes::create(
    builder,
    &msg::ResourcesResArgs {
      resources: Some(resources),
    },
  );

  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ResourcesRes,
      ..Default::default()
    },
  ))
}
