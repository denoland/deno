// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::futures::FutureExt;
use crate::op_error::OpError;
use crate::state::State;
use crate::tsc::runtime_bundle;
use crate::tsc::runtime_compile;
use crate::tsc::runtime_transpile;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;

pub fn init(i: &mut CoreIsolate, s: &State) {
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
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.compile");
  let args: CompileArgs = serde_json::from_value(args)?;
  let s = state.borrow();
  let global_state = s.global_state.clone();
  let permissions = s.permissions.clone();
  let fut = async move {
    let fut = if args.bundle {
      runtime_bundle(
        global_state,
        permissions,
        &args.root_name,
        &args.sources,
        &args.options,
      )
      .boxed_local()
    } else {
      runtime_compile(
        global_state,
        permissions,
        &args.root_name,
        &args.sources,
        &args.options,
      )
      .boxed_local()
    };

    fut.await
  }
  .boxed_local();
  Ok(JsonOp::Async(fut))
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

fn op_transpile(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.transpile");
  let args: TranspileArgs = serde_json::from_value(args)?;
  let s = state.borrow();
  let global_state = s.global_state.clone();
  let permissions = s.permissions.clone();
  let fut = async move {
    runtime_transpile(global_state, permissions, &args.sources, &args.options)
      .await
  }
  .boxed_local();
  Ok(JsonOp::Async(fut))
}
