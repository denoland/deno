// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::http_util;
use crate::permissions::Permissions;
use deno_fetch::reqwest;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  user_agent: String,
  maybe_ca_file: Option<&str>,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<reqwest::Client>({
      http_util::create_http_client(user_agent, maybe_ca_file).unwrap()
    });
  }
  super::reg_json_async(rt, "op_fetch", deno_fetch::op_fetch::<Permissions>);
  super::reg_json_async(rt, "op_fetch_read", deno_fetch::op_fetch_read);
  super::reg_json_sync(
    rt,
    "op_create_http_client",
    deno_fetch::op_create_http_client::<Permissions>,
  );
}
