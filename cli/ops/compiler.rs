// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::op_error::OpError;
use crate::ops::json_op;
use crate::ops::JsonOpDispatcher;
use crate::state::State;
use deno_core::Buf;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(
  i: &mut CoreIsolate,
  _s: &State,
  response: Arc<Mutex<Option<Buf>>>,
) {
  let custom_assets = std::collections::HashMap::new();
  // TODO(ry) use None.
  // TODO(bartlomieju): is this op even required?
  i.register_op(
    "op_fetch_asset",
    crate::op_fetch_asset::op_fetch_asset(custom_assets),
  );

  i.register_op(
    "op_compiler_respond",
    json_op(compiler_op(response.clone(), op_compiler_respond)),
  );
}

pub fn compiler_op<D>(
  response: Arc<Mutex<Option<Buf>>>,
  dispatcher: D,
) -> impl JsonOpDispatcher
where
  D: Fn(
    Arc<Mutex<Option<Buf>>>,
    Value,
    &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, OpError>,
{
  move |_isolate_state: &mut CoreIsolateState,
        args: Value,
        zero_copy: &mut [ZeroCopyBuf]|
        -> Result<JsonOp, OpError> {
    dispatcher(response.clone(), args, zero_copy)
  }
}

fn op_compiler_respond(
  response: Arc<Mutex<Option<Buf>>>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let mut r = response.lock().unwrap();
  let str_ = args.to_string();
  *r = Some(str_.as_bytes().to_vec().into_boxed_slice());
  Ok(JsonOp::Sync(json!({})))
}
