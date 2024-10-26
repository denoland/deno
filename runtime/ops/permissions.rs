// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use ::deno_permissions::PermissionState;
use ::deno_permissions::PermissionsContainer;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use serde::Deserialize;
use serde::Serialize;

deno_core::extension!(
  deno_permissions,
  ops = [
    op_query_permission,
    op_revoke_permission,
    op_request_permission,
  ],
);

#[derive(Deserialize)]
pub struct PermissionArgs {
  name: String,
  path: Option<String>,
  host: Option<String>,
  variable: Option<String>,
  kind: Option<String>,
  command: Option<String>,
}

#[derive(Serialize)]
pub struct PermissionStatus {
  state: String,
  partial: bool,
}

impl From<PermissionState> for PermissionStatus {
  fn from(state: PermissionState) -> Self {
    PermissionStatus {
      state: if state == PermissionState::GrantedPartial {
        PermissionState::Granted.to_string()
      } else {
        state.to_string()
      },
      partial: state == PermissionState::GrantedPartial,
    }
  }
}

#[op2]
#[serde]
pub fn op_query_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.query_read(args.path.as_deref())?,
    "write" => permissions.query_write(args.path.as_deref())?,
    "net" => permissions.query_net(args.host.as_deref())?,
    "env" => permissions.query_env(args.variable.as_deref()),
    "sys" => permissions.query_sys(args.kind.as_deref())?,
    "run" => permissions.query_run(args.command.as_deref())?,
    "ffi" => permissions.query_ffi(args.path.as_deref())?,
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {n}"),
      ))
    }
  };
  Ok(PermissionStatus::from(perm))
}

#[op2]
#[serde]
pub fn op_revoke_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.revoke_read(args.path.as_deref())?,
    "write" => permissions.revoke_write(args.path.as_deref())?,
    "net" => permissions.revoke_net(args.host.as_deref())?,
    "env" => permissions.revoke_env(args.variable.as_deref()),
    "sys" => permissions.revoke_sys(args.kind.as_deref())?,
    "run" => permissions.revoke_run(args.command.as_deref())?,
    "ffi" => permissions.revoke_ffi(args.path.as_deref())?,
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {n}"),
      ))
    }
  };
  Ok(PermissionStatus::from(perm))
}

#[op2]
#[serde]
pub fn op_request_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.request_read(args.path.as_deref())?,
    "write" => permissions.request_write(args.path.as_deref())?,
    "net" => permissions.request_net(args.host.as_deref())?,
    "env" => permissions.request_env(args.variable.as_deref()),
    "sys" => permissions.request_sys(args.kind.as_deref())?,
    "run" => permissions.request_run(args.command.as_deref())?,
    "ffi" => permissions.request_ffi(args.path.as_deref())?,
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {n}"),
      ))
    }
  };
  Ok(PermissionStatus::from(perm))
}
