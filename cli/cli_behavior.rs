// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate_state::*;
use crate::ops;
use deno::deno_buf;
use deno::Behavior;
use deno::Op;
use std::sync::Arc;

/// Implements deno::Behavior for the main Deno command-line.
pub struct CliBehavior {
  pub state: Arc<IsolateState>,
}

impl CliBehavior {
  pub fn new(state: Arc<IsolateState>) -> Self {
    Self { state }
  }
}

impl IsolateStateContainer for &CliBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl IsolateStateContainer for CliBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl Behavior for CliBehavior {
  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, ops::op_selector_std)
  }
}
