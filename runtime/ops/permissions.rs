// Copyright 2018-2025 the Deno authors. MIT license.

use ::deno_permissions::PermissionState;
use ::deno_permissions::PermissionsContainer;
use deno_core::OpState;
use deno_core::op2;
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
  state: &'static str,
  partial: bool,
}

impl From<PermissionState> for PermissionStatus {
  fn from(state: PermissionState) -> Self {
    PermissionStatus {
      state: match state {
        PermissionState::Granted | PermissionState::GrantedPartial => "granted",
        PermissionState::Ignored
        | PermissionState::DeniedPartial
        | PermissionState::Denied => "denied",
        PermissionState::Prompt => "prompt",
      },
      partial: state == PermissionState::GrantedPartial,
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PermissionError {
  #[class(reference)]
  #[error("No such permission name: {0}")]
  InvalidPermissionName(String),
  #[class(inherit)]
  #[error("{0}")]
  PathResolve(#[from] ::deno_permissions::PathResolveError),
  #[class(uri)]
  #[error("{0}")]
  NetDescriptorParse(#[from] ::deno_permissions::NetDescriptorParseError),
  #[class(inherit)]
  #[error("{0}")]
  SysDescriptorParse(#[from] ::deno_permissions::SysDescriptorParseError),
  #[class(inherit)]
  #[error("{0}")]
  RunDescriptorParse(#[from] ::deno_permissions::RunDescriptorParseError),
}

#[op2]
#[serde]
pub fn op_query_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, PermissionError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.query_read(args.path.as_deref())?,
    "write" => permissions.query_write(args.path.as_deref())?,
    "net" => permissions.query_net(args.host.as_deref())?,
    "env" => permissions.query_env(args.variable.as_deref()),
    "sys" => permissions.query_sys(args.kind.as_deref())?,
    "run" => permissions.query_run(args.command.as_deref())?,
    "ffi" => permissions.query_ffi(args.path.as_deref())?,
    "import" => permissions.query_import(args.host.as_deref())?,
    _ => return Err(PermissionError::InvalidPermissionName(args.name)),
  };
  Ok(PermissionStatus::from(perm))
}

#[op2]
#[serde]
pub fn op_revoke_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, PermissionError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.revoke_read(args.path.as_deref())?,
    "write" => permissions.revoke_write(args.path.as_deref())?,
    "net" => permissions.revoke_net(args.host.as_deref())?,
    "env" => permissions.revoke_env(args.variable.as_deref()),
    "sys" => permissions.revoke_sys(args.kind.as_deref())?,
    "run" => permissions.revoke_run(args.command.as_deref())?,
    "ffi" => permissions.revoke_ffi(args.path.as_deref())?,
    "import" => permissions.revoke_import(args.host.as_deref())?,
    _ => return Err(PermissionError::InvalidPermissionName(args.name)),
  };
  Ok(PermissionStatus::from(perm))
}

#[op2(stack_trace)]
#[serde]
pub fn op_request_permission(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, PermissionError> {
  let permissions = state.borrow::<PermissionsContainer>();
  let perm = match args.name.as_ref() {
    "read" => permissions.request_read(args.path.as_deref())?,
    "write" => permissions.request_write(args.path.as_deref())?,
    "net" => permissions.request_net(args.host.as_deref())?,
    "env" => permissions.request_env(args.variable.as_deref()),
    "sys" => permissions.request_sys(args.kind.as_deref())?,
    "run" => permissions.request_run(args.command.as_deref())?,
    "ffi" => permissions.request_ffi(args.path.as_deref())?,
    "import" => permissions.request_import(args.host.as_deref())?,
    _ => return Err(PermissionError::InvalidPermissionName(args.name)),
  };
  Ok(PermissionStatus::from(perm))
}
