// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;
use std::sync::Mutex;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  response: Arc<Mutex<Option<String>>>,
) {
  let custom_assets = std::collections::HashMap::new();
  // TODO(ry) use None.
  // TODO(bartlomieju): is this op even required?
  rt.register_op(
    "op_fetch_asset",
    crate::op_fetch_asset::op_fetch_asset(custom_assets),
  );

  super::reg_json_sync(
    rt,
    "op_compiler_respond",
    move |_state, args, _bufs| {
      let mut response_slot = response.lock().unwrap();
      let replaced_value = response_slot.replace(args.to_string());
      assert!(
        replaced_value.is_none(),
        "op_compiler_respond found unexpected existing compiler output",
      );
      Ok(json!({}))
    },
  );
}
