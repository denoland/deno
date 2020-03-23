// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::*;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_resources", s.stateful_json_op(op_resources));
  i.register_op("op_close", s.stateful_json_op(op_close));
}

fn op_resources(
  state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let state = state.borrow();
  let serialized_resources = state.resource_table.entries();
  Ok(JsonOp::Sync(json!(serialized_resources)))
}

/// op_close removes a resource from the resource table.
fn op_close(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  #[derive(Deserialize)]
  struct CloseArgs {
    rid: i32,
  }
  let args: CloseArgs = serde_json::from_value(args)?;
  let mut state = state.borrow_mut();
  state
    .resource_table
    .close(args.rid as u32)
    .ok_or_else(OpError::bad_resource_id)?;
  Ok(JsonOp::Sync(json!({})))
}
