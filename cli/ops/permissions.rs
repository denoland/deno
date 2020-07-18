// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use std::path::Path;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op(
    "op_query_permission",
    s.stateful_json_op(op_query_permission),
  );
  i.register_op(
    "op_revoke_permission",
    s.stateful_json_op(op_revoke_permission),
  );
  i.register_op(
    "op_request_permission",
    s.stateful_json_op(op_request_permission),
  );
}

#[derive(Deserialize)]
struct PermissionArgs {
  name: String,
  url: Option<String>,
  path: Option<String>,
}

pub fn op_query_permission(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let state = state.borrow();
  let path = args.path.as_deref();
  let perm = state.permissions.get_permission_state(
    &args.name,
    &args.url.as_deref(),
    &path.as_deref().map(Path::new),
  )?;
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}

pub fn op_revoke_permission(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let mut state = state.borrow_mut();
  let permissions = &mut state.permissions;
  match args.name.as_ref() {
    "run" => permissions.allow_run.revoke(),
    "read" => permissions.allow_read.revoke(),
    "write" => permissions.allow_write.revoke(),
    "net" => permissions.allow_net.revoke(),
    "env" => permissions.allow_env.revoke(),
    "plugin" => permissions.allow_plugin.revoke(),
    "hrtime" => permissions.allow_hrtime.revoke(),
    _ => {}
  };
  let path = args.path.as_deref();
  let perm = permissions.get_permission_state(
    &args.name,
    &args.url.as_deref(),
    &path.as_deref().map(Path::new),
  )?;
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}

pub fn op_request_permission(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let mut state = state.borrow_mut();
  let permissions = &mut state.permissions;
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "run" => Ok(permissions.request_run()),
    "read" => Ok(permissions.request_read(&path.as_deref().map(Path::new))),
    "write" => Ok(permissions.request_write(&path.as_deref().map(Path::new))),
    "net" => permissions.request_net(&args.url.as_deref()),
    "env" => Ok(permissions.request_env()),
    "plugin" => Ok(permissions.request_plugin()),
    "hrtime" => Ok(permissions.request_hrtime()),
    n => Err(OpError::other(format!("No such permission name: {}", n))),
  }?;
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}
