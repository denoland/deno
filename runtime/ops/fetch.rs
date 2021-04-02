// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;
use deno_fetch::reqwest;
use deno_fetch::HttpClientDefaults;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  user_agent: String,
  ca_data: Option<Vec<u8>>,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<reqwest::Client>({
      deno_fetch::create_http_client(user_agent.clone(), ca_data.clone())
        .unwrap()
    });
    state.put::<HttpClientDefaults>(HttpClientDefaults {
      user_agent,
      ca_data,
    });
  }
  super::reg_json_sync(rt, "op_fetch", deno_fetch::op_fetch::<Permissions>);
  super::reg_json_async(rt, "op_fetch_send", deno_fetch::op_fetch_send);
  super::reg_json_async(
    rt,
    "op_fetch_request_write",
    deno_fetch::op_fetch_request_write,
  );
  super::reg_json_async(
    rt,
    "op_fetch_response_read",
    deno_fetch::op_fetch_response_read,
  );
  super::reg_json_sync(
    rt,
    "op_create_http_client",
    deno_fetch::op_create_http_client::<Permissions>,
  );
}
