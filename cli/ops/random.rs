// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, JsonOp};
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;
use rand::thread_rng;
use rand::Rng;

// Get Random Values

pub struct OpGetRandomValues;

impl DenoOpDispatcher for OpGetRandomValues {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, zero_copy| {
        assert!(zero_copy.is_some());

        if let Some(ref seeded_rng) = state.seeded_rng {
          let mut rng = seeded_rng.lock().unwrap();
          rng.fill(&mut zero_copy.unwrap()[..]);
        } else {
          let mut rng = thread_rng();
          rng.fill(&mut zero_copy.unwrap()[..]);
        }

        Ok(JsonOp::Sync(json!({})))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "getRandomValues";
}
