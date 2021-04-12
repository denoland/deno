// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub mod errors;
pub mod runtime_compiler;

use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_runtime::metrics::metrics_op;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

pub fn reg_async<F, R>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(Rc<RefCell<OpState>>, Value, Option<ZeroCopyBuf>) -> R + 'static,
  R: Future<Output = Result<Value, AnyError>> + 'static,
{
  let op_id = rt.next_op_id();
  rt.register_op(name, metrics_op(op_id, op_async(op_fn)));
}

pub fn reg_sync<F>(rt: &mut JsRuntime, name: &'static str, op_fn: F)
where
  F: Fn(&mut OpState, Value, Option<ZeroCopyBuf>) -> Result<Value, AnyError>
    + 'static,
{
  let op_id = rt.next_op_id();
  rt.register_op(name, metrics_op(op_id, op_sync(op_fn)));
}
