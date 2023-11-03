// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::PermissionState;
use crate::RuntimePermissions;
use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url;
use deno_core::OpState;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;

deno_core::extension!(
  deno_permissions,
  parameters = [P: RuntimePermissions],
  ops = [
    op_query_permission<P>,
    op_revoke_permission<P>,
    op_request_permission<P>,
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

pub fn parse_sys_kind(kind: &str) -> Result<&str, AnyError> {
  match kind {
    "hostname" | "osRelease" | "osUptime" | "loadavg" | "networkInterfaces"
    | "systemMemoryInfo" | "uid" | "gid" => Ok(kind),
    _ => Err(type_error(format!("unknown system info kind \"{kind}\""))),
  }
}

#[op2]
#[serde]
pub fn op_query_permission<P>(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError>
where
  P: RuntimePermissions + 'static,
{
  let permissions = state.borrow::<P>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.query_read(path.map(Path::new)),
    "write" => permissions.query_write(path.map(Path::new)),
    "net" => permissions.query_net(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.query_env(args.variable.as_deref()),
    "sys" => permissions
      .query_sys(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.query_run(args.command.as_deref()),
    "ffi" => permissions.query_ffi(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.query_hrtime(),
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
pub fn op_revoke_permission<P>(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError>
where
  P: RuntimePermissions + 'static,
{
  let mut permissions = state.borrow_mut::<P>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.revoke_read(path.map(Path::new)),
    "write" => permissions.revoke_write(path.map(Path::new)),
    "net" => permissions.revoke_net(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.revoke_env(args.variable.as_deref()),
    "sys" => permissions
      .revoke_sys(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.revoke_run(args.command.as_deref()),
    "ffi" => permissions.revoke_ffi(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.revoke_hrtime(),
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
pub fn op_request_permission<P>(
  state: &mut OpState,
  #[serde] args: PermissionArgs,
) -> Result<PermissionStatus, AnyError>
where
  P: RuntimePermissions + 'static,
{
  let mut permissions = state.borrow_mut::<P>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.request_read(path.map(Path::new)),
    "write" => permissions.request_write(path.map(Path::new)),
    "net" => permissions.request_net(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.request_env(args.variable.as_deref()),
    "sys" => permissions
      .request_sys(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.request_run(args.command.as_deref()),
    "ffi" => permissions.request_ffi(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.request_hrtime(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {n}"),
      ))
    }
  };
  Ok(PermissionStatus::from(perm))
}

fn parse_host(host_str: &str) -> Result<(String, Option<u16>), AnyError> {
  let url = url::Url::parse(&format!("http://{host_str}/"))
    .map_err(|_| uri_error("Invalid host"))?;
  if url.path() != "/" {
    return Err(uri_error("Invalid host"));
  }
  let hostname = url.host_str().unwrap();
  Ok((hostname.to_string(), url.port()))
}
