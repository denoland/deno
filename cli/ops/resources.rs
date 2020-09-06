// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::state::State;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use serde_derive::Deserialize;
use serde_json::Value;
use std::rc::Rc;

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_resources", op_resources);
  s.register_op_json_sync("op_close", op_close);
}

fn op_resources(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let resource_table = state.resource_table.borrow();
  let serialized_resources = resource_table.entries();
  Ok(json!(serialized_resources))
}

/// op_close removes a resource from the resource table.
fn op_close(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  #[derive(Deserialize)]
  struct CloseArgs {
    rid: i32,
  }
  let args: CloseArgs = serde_json::from_value(args)?;
  state
    .resource_table
    .borrow_mut()
    .close(args.rid as u32)
    .ok_or_else(ErrBox::bad_resource_id)?;
  Ok(json!({}))
}
