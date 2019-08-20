// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::utils::*;
use crate::deno_error;
use crate::fmt_errors::JSError;
use crate::msg;
use crate::source_maps::get_orig_position;
use crate::source_maps::CachedMaps;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use std::collections::HashMap;

pub fn op_format_error(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_format_error().unwrap();
  let json_str = inner.error().unwrap();
  let error = JSError::from_json(json_str, &state.ts_compiler);
  let error_string = error.to_string();

  let mut builder = FlatBufferBuilder::new();
  let new_error = builder.create_string(&error_string);

  let inner = msg::FormatErrorRes::create(
    &mut builder,
    &msg::FormatErrorResArgs {
      error: Some(new_error),
    },
  );

  let response_buf = serialize_response(
    base.cmd_id(),
    &mut builder,
    msg::BaseArgs {
      inner_type: msg::Any::FormatErrorRes,
      inner: Some(inner.as_union_value()),
      ..Default::default()
    },
  );

  ok_buf(response_buf)
}

pub fn op_apply_source_map(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if !base.sync() {
    return Err(deno_error::no_async_support());
  }
  assert!(data.is_none());
  let inner = base.inner_as_apply_source_map().unwrap();
  let cmd_id = base.cmd_id();
  let filename = inner.filename().unwrap();
  let line = inner.line();
  let column = inner.column();

  let mut mappings_map: CachedMaps = HashMap::new();
  let (orig_filename, orig_line, orig_column) = get_orig_position(
    filename.to_owned(),
    line.into(),
    column.into(),
    &mut mappings_map,
    &state.ts_compiler,
  );

  let builder = &mut FlatBufferBuilder::new();
  let msg_args = msg::ApplySourceMapArgs {
    filename: Some(builder.create_string(&orig_filename)),
    line: orig_line as i32,
    column: orig_column as i32,
  };
  let res_inner = msg::ApplySourceMap::create(builder, &msg_args);
  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(res_inner.as_union_value()),
      inner_type: msg::Any::ApplySourceMap,
      ..Default::default()
    },
  ))
}
