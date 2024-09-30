// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod bootstrap;
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

/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, feature: &str, api_name: &str) {
  state.feature_checker.check_or_exit(feature, api_name);
}

pub struct TestingFeaturesEnabled(pub bool);
