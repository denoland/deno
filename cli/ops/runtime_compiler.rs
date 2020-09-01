// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, Value};
use crate::futures::FutureExt;
use crate::state::State;
use crate::tsc::runtime_bundle;
use crate::tsc::runtime_compile;
use crate::tsc::runtime_transpile;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::ResourceTable;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  let t = &CoreIsolate::state(i).borrow().resource_table.clone();

  i.register_op("op_compile", s.stateful_json_op_async(t, op_compile));
  i.register_op("op_transpile", s.stateful_json_op_async(t, op_transpile));
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
  _resource_table: Rc<RefCell<ResourceTable>>,
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
  _resource_table: Rc<RefCell<ResourceTable>>,
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
