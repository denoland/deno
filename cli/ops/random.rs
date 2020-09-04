// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::Value;
use crate::state::State;
use deno_core::ErrBox;
use deno_core::OpManager;
use deno_core::ZeroCopyBuf;
use rand::thread_rng;
use rand::Rng;
use std::rc::Rc;

pub fn init(s: &Rc<State>) {
  s.register_op(
    "op_get_random_values",
    s.stateful_json_op_sync(op_get_random_values),
  );
}

fn op_get_random_values(
  state: &State,
  _args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1);

  if let Some(seeded_rng) = &state.seeded_rng {
    seeded_rng.borrow_mut().fill(&mut *zero_copy[0]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy[0]);
  }

  Ok(json!({}))
}
