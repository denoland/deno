// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate_state::*;
use crate::ops;
use crate::startup_data;
use crate::workers::WorkerBehavior;
use deno_core::deno_buf;
use deno_core::deno_mod;
use deno_core::Behavior;
use deno_core::Op;
use deno_core::StartupData;
use std::sync::Arc;

pub struct WebWorkerBehavior {
  pub state: Arc<IsolateState>,
}

impl WebWorkerBehavior {
  pub fn new(state: Arc<IsolateState>) -> Self {
    Self { state }
  }
}

impl IsolateStateContainer for WebWorkerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl IsolateStateContainer for &WebWorkerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl Behavior for WebWorkerBehavior {
  fn startup_data(&mut self) -> Option<StartupData> {
    Some(startup_data::deno_isolate_init())
  }

  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod {
    self.state_resolve(specifier, referrer)
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, ops::op_selector_worker)
  }
}

impl WorkerBehavior for WebWorkerBehavior {
  fn set_internal_channels(&mut self, worker_channels: WorkerChannels) {
    self.state = Arc::new(IsolateState::new(
      self.state.flags.clone(),
      self.state.argv.clone(),
      Some(worker_channels),
    ));
  }
}
