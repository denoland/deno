// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::state::State;
use deno_core::CoreIsolate;

pub fn init(i: &mut CoreIsolate, _s: &State) {
  let custom_assets = std::collections::HashMap::new();
  // TODO(ry) use None.
  // TODO(bartlomieju): is this op even required?
  i.register_op(
    "op_fetch_asset",
    deno_typescript::op_fetch_asset(custom_assets),
  );
}
