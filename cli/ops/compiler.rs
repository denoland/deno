// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ops::dispatch_json::JsonOp;
use crate::ops::dispatch_json::Value;
use crate::ops::json_op;
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpManager;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(s: &Rc<State>, response: Arc<Mutex<Option<String>>>) {
  let custom_assets = std::collections::HashMap::new();
  // TODO(ry) use None.
  // TODO(bartlomieju): is this op even required?
  s.register_op(
    "op_fetch_asset",
    crate::op_fetch_asset::op_fetch_asset(custom_assets),
  );

  s.register_op(
    "op_compiler_respond",
    json_op(compiler_op(response, op_compiler_respond)),
  );
}

pub fn compiler_op<D>(
  response: Arc<Mutex<Option<String>>>,
  dispatcher: D,
) -> impl Fn(Rc<State>, Value, BufVec) -> Result<JsonOp, ErrBox>
where
  D: Fn(Arc<Mutex<Option<String>>>, Value, BufVec) -> Result<JsonOp, ErrBox>,
{
  move |_state: Rc<State>,
        args: Value,
        zero_copy: BufVec|
        -> Result<JsonOp, ErrBox> {
    dispatcher(response.clone(), args, zero_copy)
  }
}

fn op_compiler_respond(
  response: Arc<Mutex<Option<String>>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<JsonOp, ErrBox> {
  let mut r = response.lock().unwrap();
  assert!(
    r.is_none(),
    "op_compiler_respond found unexpected existing compiler output"
  );
  *r = Some(args.to_string());
  Ok(JsonOp::Sync(json!({})))
}
