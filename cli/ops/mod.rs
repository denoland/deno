// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod dispatch_minimal;
pub use dispatch_minimal::MinimalOp;
//
//pub mod compiler;
pub mod errors;
pub mod fetch;
pub mod fs;
pub mod fs_events;
pub mod idna;
pub mod io;
pub mod net;
#[cfg(unix)]
mod net_unix;
pub mod os;
pub mod permissions;
//pub mod plugin;
pub mod process;
pub mod random;
pub mod repl;
//pub mod resources;
pub mod runtime;
pub mod runtime_compiler;
//pub mod signal;
//pub mod timers;
//pub mod tls;
//pub mod tty;
//pub mod web_worker;
pub mod websocket;
//pub mod worker_host;

use crate::metrics::metrics_op;
use deno_core::json_op_async;
use deno_core::json_op_sync;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde_json::Value;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

pub fn reg_json_async<F, R>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(Rc<RefCell<OpState>>, Value, BufVec) -> R + 'static,
  R: Future<Output = Result<Value, ErrBox>> + 'static,
{
  rt.register_op(name, metrics_op(json_op_async(op_fn)));
}

pub fn reg_json_sync<F>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(&mut OpState, Value, &mut [ZeroCopyBuf]) -> Result<Value, ErrBox>
    + 'static,
{
  rt.register_op(name, metrics_op(json_op_sync(op_fn)));
}
