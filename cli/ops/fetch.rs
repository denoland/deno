// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::state::CliState;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_fetch", deno_fetch::op_fetch::<CliState>);
  super::reg_json_async(rt, "op_fetch_read", deno_fetch::op_fetch_read);
  super::reg_json_sync(
    rt,
    "op_create_http_client",
    deno_fetch::op_create_http_client::<CliState>,
  );
}
