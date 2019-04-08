// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate_state::*;
use crate::ops;
use deno::deno_buf;
use deno::Dispatch;
use deno::Op;
use std::sync::Arc;

/// Implements deno::Dispatch for the main Deno command-line.
pub struct CliDispatch {
  pub state: Arc<IsolateState>,
}

impl CliDispatch {
  pub fn new(state: Arc<IsolateState>) -> Self {
    Self { state }
  }
}

impl Dispatch for CliDispatch {
  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(&self.state, control, zero_copy, ops::op_selector_std)
  }
}
