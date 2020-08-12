// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_resources", s.stateful_json_op2(op_resources));
  i.register_op("op_close", s.stateful_json_op2(op_close));
}

fn op_resources(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let serialized_resources = isolate_state.resource_table.borrow().entries();
  Ok(JsonOp::Sync(json!(serialized_resources)))
}

/// op_close removes a resource from the resource table.
fn op_close(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  #[derive(Deserialize)]
  struct CloseArgs {
    rid: i32,
  }
  let args: CloseArgs = serde_json::from_value(args)?;
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  resource_table
    .close(args.rid as u32)
    .ok_or_else(OpError::bad_resource_id)?;
  Ok(JsonOp::Sync(json!({})))
}
