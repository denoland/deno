// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "query_permission",
    s.core_op(json_op(s.stateful_op(op_query_permission))),
  );
  i.register_op(
    "revoke_permission",
    s.core_op(json_op(s.stateful_op(op_revoke_permission))),
  );
}

#[derive(Deserialize)]
struct PermissionArgs {
  name: String,
  url: Option<String>,
  path: Option<String>,
}

pub fn op_query_permission(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let perm = state.permissions.get_permission_state(
    &args.name,
    &args.url.as_ref().map(String::as_str),
    &args.path.as_ref().map(String::as_str),
  )?;
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}

pub fn op_revoke_permission(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  match args.name.as_ref() {
    "run" => state.permissions.allow_run.revoke(),
    "read" => state.permissions.allow_read.revoke(),
    "write" => state.permissions.allow_write.revoke(),
    "net" => state.permissions.allow_net.revoke(),
    "env" => state.permissions.allow_env.revoke(),
    "hrtime" => state.permissions.allow_hrtime.revoke(),
    _ => {}
  };
  let perm = state.permissions.get_permission_state(
    &args.name,
    &args.url.as_ref().map(String::as_str),
    &args.path.as_ref().map(String::as_str),
  )?;
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}
