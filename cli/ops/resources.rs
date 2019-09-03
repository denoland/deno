// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::resources::table_entries;
use crate::state::ThreadSafeState;
use deno::*;

pub fn op_resources(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let serialized_resources = table_entries();
  Ok(JsonOp::Sync(json!(serialized_resources)))
}
