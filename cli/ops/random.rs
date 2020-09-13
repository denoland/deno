// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::ErrBox;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rand::thread_rng;
use rand::Rng;
use serde_json::Value;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_get_random_values", op_get_random_values);
}

fn op_get_random_values(
  state: &mut OpState,
  _args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1);
  let cli_state = super::cli_state(state);
  if let Some(seeded_rng) = &cli_state.seeded_rng {
    seeded_rng.borrow_mut().fill(&mut *zero_copy[0]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut *zero_copy[0]);
  }

  Ok(json!({}))
}
