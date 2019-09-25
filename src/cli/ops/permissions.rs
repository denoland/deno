// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::state::ThreadSafeState;
use deno::*;

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
