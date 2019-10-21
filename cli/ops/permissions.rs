// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "permissions",
    s.core_op(json_op(s.stateful_op(op_permissions))),
  );
  i.register_op(
    "revoke_permission",
    s.core_op(json_op(s.stateful_op(op_revoke_permission))),
  );
}

pub fn op_permissions(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  Ok(JsonOp::Sync(json!({
    "run": state.permissions.allows_run(),
    "read": state.permissions.allows_read(),
    "write": state.permissions.allows_write(),
    "net": state.permissions.allows_net(),
    "env": state.permissions.allows_env(),
    "hrtime": state.permissions.allows_hrtime(),
  })))
}

#[derive(Deserialize)]
struct RevokePermissionArgs {
  permission: String,
}

pub fn op_revoke_permission(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RevokePermissionArgs = serde_json::from_value(args)?;
  let permission = args.permission.as_ref();
  match permission {
    "run" => state.permissions.revoke_run(),
    "read" => state.permissions.revoke_read(),
    "write" => state.permissions.revoke_write(),
    "net" => state.permissions.revoke_net(),
    "env" => state.permissions.revoke_env(),
    "hrtime" => state.permissions.revoke_hrtime(),
    _ => Ok(()),
  }?;

  Ok(JsonOp::Sync(json!({})))
}
