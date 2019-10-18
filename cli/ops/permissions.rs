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
    "request_permission",
    s.core_op(json_op(s.stateful_op(op_request_permission))),
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
  Ok(JsonOp::Sync(json!({
    "state": state.permissions.get_permission_string(args.name)?
  })))
}

pub fn op_request_permission(
  state: &ThreadSafeState,
  value: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(value)?;
  let name = args.name.as_ref();
  match name {
    "run" => state.permissions.request_run(),
    "read" => state.permissions.request_read(args.path.unwrap().as_ref()),
    "write" => state.permissions.request_write(args.path.unwrap().as_ref()),
    "net" => state.permissions.request_net(args.url.unwrap().as_ref()),
    "env" => state.permissions.request_env(),
    "hrtime" => state.permissions.request_hrtime(),
    _ => Ok(()),
  }?;
  Ok(JsonOp::Sync(json!({
    "state": state.permissions.get_permission_string(args.name)?
  })))
}

pub fn op_revoke_permission(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  match args.name.as_ref() {
    "run" => state.permissions.revoke_run(),
    "read" => state.permissions.revoke_read(),
    "write" => state.permissions.revoke_write(),
    "net" => state.permissions.revoke_net(),
    "env" => state.permissions.revoke_env(),
    "hrtime" => state.permissions.revoke_hrtime(),
    _ => Ok(()),
  }?;
  Ok(JsonOp::Sync(json!({
    "state": state.permissions.get_permission_string(args.name)?
  })))
}
