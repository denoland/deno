// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::permissions;
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

#[derive(Deserialize)]
struct PermissionArgs {
  name: String,
  url: Option<String>,
  path: Option<String>,
}

fn get_current_permission(
  state: &ThreadSafeState,
  args: &PermissionArgs,
) -> Result<permissions::PermissionAccessorState, ErrBox> {
  state.permissions.get_permission_state(
    &args.name,
    &args.path.as_ref().map(String::as_str),
    &args.url.as_ref().map(String::as_str),
  )
}

fn permission_state_to_json_op(
  state: permissions::PermissionAccessorState,
) -> JsonOp {
  JsonOp::Sync(json!({
    "state": state.to_string()
  }))
}

pub fn op_query_permission(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let perm = get_current_permission(state, &args)?;
  Ok(permission_state_to_json_op(perm))
}

pub fn op_request_permission(
  state: &ThreadSafeState,
  value: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: PermissionArgs = serde_json::from_value(value)?;
  let path = args.path.as_ref();
  let url = args.url.as_ref();
  let name = args.name.as_ref();
  let perm = get_current_permission(state, &args)?;
  if perm != permissions::PermissionAccessorState::Ask {
    return Ok(permission_state_to_json_op(perm));
  }

  match name {
    "run" => state.permissions.request_run(),
    "read" => state.permissions.request_read(&path.map(String::as_str)),
    "write" => state.permissions.request_write(&path.map(String::as_str)),
    "net" => state.permissions.request_net(&url.map(String::as_str)),
    "env" => state.permissions.request_env(),
    "hrtime" => state.permissions.request_hrtime(),
    _ => Ok(()),
  }?;

  let perm1 = get_current_permission(state, &args)?;
  Ok(permission_state_to_json_op(perm1))
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
    _ => {}
  };

  let perm = get_current_permission(state, &args)?;
  Ok(permission_state_to_json_op(perm))
}
