// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::State;
use deno_core::*;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("resources", s.core_op(json_op(s.stateful_op(op_resources))));
}

fn op_resources(
  state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let state = state.borrow();
  let serialized_resources = state.resource_table.entries();
  Ok(JsonOp::Sync(json!(serialized_resources)))
}
