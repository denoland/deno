// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;
use deno_websocket::op_ws_check_permission;
use deno_websocket::op_ws_close;
use deno_websocket::op_ws_create;
use deno_websocket::op_ws_next_event;
use deno_websocket::op_ws_send;
use deno_websocket::WsCaData;
use deno_websocket::WsUserAgent;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  user_agent: String,
  ca_data: Option<Vec<u8>>,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<WsUserAgent>(WsUserAgent(user_agent));
    if let Some(ca_data) = ca_data {
      state.put::<WsCaData>(WsCaData(ca_data));
    }
  }
  super::reg_json_sync(
    rt,
    "op_ws_check_permission",
    op_ws_check_permission::<Permissions>,
  );
  super::reg_json_async(rt, "op_ws_create", op_ws_create::<Permissions>);
  super::reg_json_async(rt, "op_ws_send", op_ws_send);
  super::reg_json_async(rt, "op_ws_close", op_ws_close);
  super::reg_json_async(rt, "op_ws_next_event", op_ws_next_event);
}
