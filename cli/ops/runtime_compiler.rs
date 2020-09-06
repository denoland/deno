// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::futures::FutureExt;
use crate::state::State;
use crate::tsc::runtime_bundle;
use crate::tsc::runtime_compile;
use crate::tsc::runtime_transpile;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use serde_derive::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub fn init(s: &Rc<State>) {
  s.register_op_json_async("op_compile", op_compile);
  s.register_op_json_async("op_transpile", op_transpile);
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CompileArgs {
  root_name: String,
  sources: Option<HashMap<String, String>>,
  bundle: bool,
  options: Option<String>,
}

async fn op_compile(
  state: Rc<State>,
  args: Value,
  _data: BufVec,
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.compile");
  let args: CompileArgs = serde_json::from_value(args)?;
  let global_state = state.global_state.clone();
  let permissions = state.permissions.borrow().clone();
  let fut = if args.bundle {
    runtime_bundle(
      &global_state,
      permissions,
      &args.root_name,
      &args.sources,
      &args.options,
    )
    .boxed_local()
  } else {
    runtime_compile(
      &global_state,
      permissions,
      &args.root_name,
      &args.sources,
      &args.options,
    )
    .boxed_local()
  };
  let result = fut.await?;
  Ok(result)
}

#[derive(Deserialize, Debug)]
struct TranspileArgs {
  sources: HashMap<String, String>,
  options: Option<String>,
}

async fn op_transpile(
  state: Rc<State>,
  args: Value,
  _data: BufVec,
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.transpile");
  let args: TranspileArgs = serde_json::from_value(args)?;
  let global_state = state.global_state.clone();
  let permissions = state.permissions.borrow().clone();
  let result =
    runtime_transpile(&global_state, permissions, &args.sources, &args.options)
      .await?;
  Ok(result)
}
