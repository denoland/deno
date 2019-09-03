// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::state::ThreadSafeState;
use deno::*;
use rand::thread_rng;
use rand::Rng;

pub fn op_get_random_values(
  state: &ThreadSafeState,
  _args: Value,
  zero_copy: Option<PinnedBuf>,
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
