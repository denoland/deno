// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use std::path::Path;
use std::rc::Rc;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
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
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let permissions = state.permissions.borrow();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.query_read(&path.as_deref().map(Path::new)),
    "write" => permissions.query_write(&path.as_deref().map(Path::new)),
    "net" => permissions.query_net_url(&args.url.as_deref())?,
    "env" => permissions.query_env(),
    "run" => permissions.query_run(),
    "plugin" => permissions.query_plugin(),
    "hrtime" => permissions.query_hrtime(),
    n => return Err(OpError::other(format!("No such permission name: {}", n))),
  };
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}

pub fn op_revoke_permission(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let mut permissions = state.permissions.borrow_mut();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.revoke_read(&path.as_deref().map(Path::new)),
    "write" => permissions.revoke_write(&path.as_deref().map(Path::new)),
    "net" => permissions.revoke_net(&args.url.as_deref())?,
    "env" => permissions.revoke_env(),
    "run" => permissions.revoke_run(),
    "plugin" => permissions.revoke_plugin(),
    "hrtime" => permissions.revoke_hrtime(),
    n => return Err(OpError::other(format!("No such permission name: {}", n))),
  };
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}

pub fn op_request_permission(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let permissions = &mut state.permissions.borrow_mut();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.request_read(&path.as_deref().map(Path::new)),
    "write" => permissions.request_write(&path.as_deref().map(Path::new)),
    "net" => permissions.request_net(&args.url.as_deref())?,
    "env" => permissions.request_env(),
    "run" => permissions.request_run(),
    "plugin" => permissions.request_plugin(),
    "hrtime" => permissions.request_hrtime(),
    n => return Err(OpError::other(format!("No such permission name: {}", n))),
  };
  Ok(JsonOp::Sync(json!({ "state": perm.to_string() })))
}
