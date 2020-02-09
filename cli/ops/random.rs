// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::State;
use deno_core::*;
use rand::thread_rng;
use rand::Rng;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "get_random_values",
    s.core_op(json_op(s.stateful_op(op_get_random_values))),
  );
}

fn op_get_random_values(
  state: &State,
  _args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  assert!(zero_copy.is_some());

  if let Some(ref mut seeded_rng) = state.borrow_mut().seeded_rng {
    seeded_rng.fill(&mut zero_copy.unwrap()[..]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut zero_copy.unwrap()[..]);
  }

  Ok(JsonOp::Sync(json!({})))
}
