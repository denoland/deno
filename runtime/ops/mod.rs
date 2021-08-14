// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub mod fs;
pub mod fs_events;
pub mod http;
pub mod io;
pub mod os;
pub mod permissions;
pub mod process;
pub mod runtime;
pub mod signal;
pub mod tty;
mod utils;
pub mod web_worker;
pub mod worker_host;

use crate::metrics::metrics_op;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Serialize;
use deno_core::JsRuntime;
use deno_core::OpState;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

pub fn reg_async<F, A, B, R, RV>(
  rt: &mut JsRuntime,
  name: &'static str,
  op_fn: F,
) where
  F: Fn(Rc<RefCell<OpState>>, A, B) -> R + 'static,
  A: DeserializeOwned,
  B: DeserializeOwned,
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: Serialize + 'static,
{
  rt.register_op(name, metrics_op(name, op_async(op_fn)));
}

pub fn reg_sync<F, A, B, R>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(&mut OpState, A, B) -> Result<R, AnyError> + 'static,
  A: DeserializeOwned,
  B: DeserializeOwned,
  R: Serialize + 'static,
{
  rt.register_op(name, metrics_op(name, op_sync(op_fn)));
}

/// `UnstableChecker` is a struct so it can be placed inside `GothamState`;
/// using type alias for a bool could work, but there's a high chance
/// that there might be another type alias pointing to a bool, which
/// would override previously used alias.
pub struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{}'. The --unstable flag must be provided.",
        api_name
      );
      std::process::exit(70);
    }
  }
}
/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

pub struct TestingFeaturesEnabled(pub bool);
