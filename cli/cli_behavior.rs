// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate_state::*;
use crate::ops;
use deno::deno_buf;
use deno::Behavior;
use deno::Op;
use deno::StartupData;
use std::sync::Arc;

/// Implements deno::Behavior for the main Deno command-line.
pub struct CliBehavior {
  startup_data: Option<StartupData>,
  pub state: Arc<IsolateState>,
}

impl CliBehavior {
  pub fn new(
    startup_data: Option<StartupData>,
    state: Arc<IsolateState>,
  ) -> Self {
    Self {
      startup_data,
      state,
    }
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
  fn startup_data(&mut self) -> Option<StartupData> {
    self.startup_data.take()
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, ops::op_selector_std)
  }
}
