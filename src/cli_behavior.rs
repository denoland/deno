// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::isolate_state::IsolateStateContainer;
use crate::isolate_state::IsolateState;
use crate::ops;
use deno_core::deno_buf;
use deno_core::deno_mod;
use deno_core::Behavior;
use deno_core::Op;
use deno_core::StartupData;
use std::sync::atomic::Ordering;
use std::sync::Arc;

// Buf represents a byte array returned from a "Op". The message might be empty
// (which will be translated into a null object on the javascript side) or it is
// a heap allocated opaque sequence of bytes.  Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

/// Implements deno_core::Behavior for the main Deno command-line.
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

impl Behavior for CliBehavior {
  fn startup_data(&mut self) -> Option<StartupData> {
    self.startup_data.take()
  }

  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod {
    self
      .state
      .metrics
      .resolve_count
      .fetch_add(1, Ordering::Relaxed);
    let mut modules = self.state.modules.lock().unwrap();
    modules.resolve_cb(&self.state.dir, specifier, referrer)
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_cli(self, control, zero_copy)
  }
}

impl IsolateStateContainer for CliBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl IsolateStateContainer for &CliBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}