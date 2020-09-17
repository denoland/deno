// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde_json::Value;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_resources", op_resources);
  super::reg_json_sync(rt, "op_close", op_close);
}

fn op_resources(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let serialized_resources = state.resource_table.entries();
  Ok(json!(serialized_resources))
}

/// op_close removes a resource from the resource table.
fn op_close(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  struct CloseArgs {
    rid: i32,
  }
  let args: CloseArgs = serde_json::from_value(args)?;
  state
    .resource_table
    .close(args.rid as u32)
    .ok_or_else(bad_resource_id)?;
  Ok(json!({}))
}
