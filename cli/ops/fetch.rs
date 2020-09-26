// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;

pub fn init(state: &mut deno_core::OpState) {
  super::reg_json_async(state, "op_fetch", deno_fetch::op_fetch::<Permissions>);
  super::reg_json_async(state, "op_fetch_read", deno_fetch::op_fetch_read);
  super::reg_json_sync(
    state,
    "op_create_http_client",
    deno_fetch::op_create_http_client::<Permissions>,
  );
}
