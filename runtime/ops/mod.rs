// Copyright 2018-2025 the Deno authors. MIT license.

pub mod bootstrap;
pub mod fs_events;
pub mod http;
pub mod permissions;
pub mod runtime;
pub mod tty;
pub mod web_worker;
pub mod worker_host;

use std::sync::Arc;

use deno_core::OpState;
use deno_features::FeatureChecker;

/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, feature: &str, api_name: &str) {
  state
    .borrow::<Arc<FeatureChecker>>()
    .check_or_exit(feature, api_name);
}

pub struct TestingFeaturesEnabled(pub bool);
