// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno_core::*;
use rand::thread_rng;
use rand::Rng;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "get_random_values",
    s.core_op(json_op(s.stateful_op(op_get_random_values))),
  );
}

fn op_get_random_values(
  state: &ThreadSafeState,
  _args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  assert!(zero_copy.is_some());

  if let Some(ref seeded_rng) = state.seeded_rng {
    let mut rng = seeded_rng.lock().unwrap();
    rng.fill(&mut zero_copy.unwrap()[..]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut zero_copy.unwrap()[..]);
  }

  Ok(JsonOp::Sync(json!({})))
}
