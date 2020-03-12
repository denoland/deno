// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::compilers::runtime_compile;
use crate::compilers::runtime_transpile;
use crate::op_error::OpError;
use crate::state::State;
use deno_core::*;
use std::collections::HashMap;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_compile", s.stateful_json_op(op_compile));
  i.register_op("op_transpile", s.stateful_json_op(op_transpile));
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CompileArgs {
  root_name: String,
  sources: Option<HashMap<String, String>>,
  bundle: bool,
  options: Option<String>,
}

fn op_compile(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CompileArgs = serde_json::from_value(args)?;
  Ok(JsonOp::Async(runtime_compile(
    state.borrow().global_state.clone(),
    &args.root_name,
    &args.sources,
    args.bundle,
    &args.options,
  )))
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

fn op_transpile(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: TranspileArgs = serde_json::from_value(args)?;
  Ok(JsonOp::Async(runtime_transpile(
    state.borrow().global_state.clone(),
    &args.sources,
    &args.options,
  )))
}
