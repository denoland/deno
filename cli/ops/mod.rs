// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod dispatch_minimal;
pub use dispatch_minimal::MinimalOp;

pub mod crypto;
pub mod errors;
pub mod fetch;
pub mod fs;
pub mod fs_events;
pub mod io;
pub mod net;
#[cfg(unix)]
mod net_unix;
pub mod os;
pub mod permissions;
pub mod plugin;
pub mod process;
pub mod runtime;
pub mod runtime_compiler;
pub mod signal;
pub mod timers;
pub mod tls;
pub mod tty;
pub mod web_worker;
pub mod websocket;
pub mod worker_host;

use crate::metrics::metrics_op;
use crate::program_state::ProgramState;
use deno_core::error::AnyError;
use deno_core::json_op_async;
use deno_core::json_op_sync;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

pub fn reg_json_async<F, R>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(Rc<RefCell<OpState>>, Value, BufVec) -> R + 'static,
  R: Future<Output = Result<Value, AnyError>> + 'static,
{
  rt.register_op(name, metrics_op(json_op_async(op_fn)));
}

pub fn reg_json_sync<F>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(&mut OpState, Value, &mut [ZeroCopyBuf]) -> Result<Value, AnyError>
    + 'static,
{
  rt.register_op(name, metrics_op(json_op_sync(op_fn)));
}

/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<Arc<ProgramState>>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<Arc<ProgramState>>().check_unstable(api_name)
}
