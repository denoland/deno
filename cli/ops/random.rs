// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use rand::thread_rng;
use rand::Rng;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op(
    "op_get_random_values",
    s.stateful_json_op(op_get_random_values),
  );
}

fn op_get_random_values(
  state: &State,
  _args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  assert_eq!(zero_copy.len(), 1);

  if let Some(ref mut seeded_rng) = state.borrow_mut().seeded_rng {
    seeded_rng.fill(&mut *zero_copy[0]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy[0]);
  }

  Ok(JsonOp::Sync(json!({})))
}
