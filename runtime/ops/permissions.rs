// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use ::deno_permissions::parse_sys_kind;
use ::deno_permissions::PermissionDescriptorParser;
use ::deno_permissions::PermissionState;
use ::deno_permissions::PermissionsContainer;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

deno_core::extension!(
  deno_permissions,
  ops = [
    op_query_permission,
    op_revoke_permission,
    op_request_permission,
  ],
  options = {
    permission_desc_parser: Arc<dyn PermissionDescriptorParser>,
  },
  state = |state, options| {
    state.put(options.permission_desc_parser);
  },
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
  let permissions_container = state.borrow::<PermissionsContainer>();
  // todo(dsherret): don't have this function use the properties of
  // permission container
  let desc_parser = &permissions_container.descriptor_parser;
  let permissions = permissions_container.inner.lock();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.query(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_read(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "write" => permissions.write.query(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_write(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "net" => permissions.net.query(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(desc_parser.parse_net_descriptor(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.query(args.variable.as_deref()),
    "sys" => permissions
      .sys
      .query(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.run.query(
      args
        .command
        .as_deref()
        .map(|request| desc_parser.parse_run_query(request))
        .transpose()?
        .as_ref(),
    ),
    "ffi" => permissions.ffi.query(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_ffi(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
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
  // todo(dsherret): don't have this function use the properties of
  // permission container
  let permissions_container = state.borrow_mut::<PermissionsContainer>();
  let desc_parser = &permissions_container.descriptor_parser;
  let mut permissions = permissions_container.inner.lock();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.revoke(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_read(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "write" => permissions.write.revoke(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_write(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "net" => permissions.net.revoke(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(desc_parser.parse_net_descriptor(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.revoke(args.variable.as_deref()),
    "sys" => permissions
      .sys
      .revoke(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.run.revoke(
      args
        .command
        .as_deref()
        .map(|request| desc_parser.parse_run_query(request))
        .transpose()?
        .as_ref(),
    ),
    "ffi" => permissions.ffi.revoke(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_ffi(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
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
  // todo(dsherret): don't have this function use the properties of
  // permission container
  let permissions_container = state.borrow_mut::<PermissionsContainer>();
  let desc_parser = &permissions_container.descriptor_parser;
  let mut permissions = permissions_container.inner.lock();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.request(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_read(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "write" => permissions.write.request(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_write(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    "net" => permissions.net.request(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(desc_parser.parse_net_descriptor(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.request(args.variable.as_deref()),
    "sys" => permissions
      .sys
      .request(args.kind.as_deref().map(parse_sys_kind).transpose()?),
    "run" => permissions.run.request(
      args
        .command
        .as_deref()
        .map(|request| desc_parser.parse_run_query(request))
        .transpose()?
        .as_ref(),
    ),
    "ffi" => permissions.ffi.request(
      path
        .map(|path| {
          Result::<_, AnyError>::Ok(
            desc_parser.parse_path_query(path)?.into_ffi(),
          )
        })
        .transpose()?
        .as_ref(),
    ),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {n}"),
      ))
    }
  };
  Ok(PermissionStatus::from(perm))
}
