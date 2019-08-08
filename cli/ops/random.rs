// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::CliOpResult;
use crate::state::ThreadSafeState;
use deno::*;
use rand::thread_rng;
use rand::Rng;

pub fn op_get_random_values(
  state: &ThreadSafeState,
  _base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if let Some(ref seeded_rng) = state.seeded_rng {
    let mut rng = seeded_rng.lock().unwrap();
    rng.fill(&mut data.unwrap()[..]);
  } else {
    let mut rng = thread_rng();
    rng.fill(&mut data.unwrap()[..]);
  }

  ok_buf(empty_buf())
}
