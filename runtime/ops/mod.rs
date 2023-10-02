// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub mod fs_events;
pub mod http;
pub mod os;
pub mod permissions;
pub mod process;
pub mod runtime;
pub mod signal;
pub mod tty;
mod utils;
pub mod web_worker;
pub mod worker_host;

use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;

/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, api_name: &str) {
  state.feature_checker.check_legacy_unstable(api_name);
}

/// Helper for checking unstable features. Used for async ops.
pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.feature_checker.check_legacy_unstable(api_name);
}

pub struct TestingFeaturesEnabled(pub bool);
