// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::custom_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::url;
use deno_core::Extension;
use deno_core::OpState;
use serde::Deserialize;
use std::path::Path;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_query_permission::decl(),
      op_revoke_permission::decl(),
      op_request_permission::decl(),
    ])
    .build()
}

#[derive(Deserialize)]
pub struct PermissionArgs {
  name: String,
  path: Option<String>,
  host: Option<String>,
  variable: Option<String>,
  command: Option<String>,
}

#[op]
pub fn op_query_permission(
  state: &mut OpState,
  args: PermissionArgs,
) -> Result<String, AnyError> {
  let permissions = state.borrow::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.query(path.map(Path::new)),
    "write" => permissions.write.query(path.map(Path::new)),
    "net" => permissions.net.query(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.query(args.variable.as_deref()),
    "run" => permissions.run.query(args.command.as_deref()),
    "ffi" => permissions.ffi.query(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.hrtime.query(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(perm.to_string())
}

#[op]
pub fn op_revoke_permission(
  state: &mut OpState,
  args: PermissionArgs,
) -> Result<String, AnyError> {
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.revoke(path.map(Path::new)),
    "write" => permissions.write.revoke(path.map(Path::new)),
    "net" => permissions.net.revoke(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.revoke(args.variable.as_deref()),
    "run" => permissions.run.revoke(args.command.as_deref()),
    "ffi" => permissions.ffi.revoke(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.hrtime.revoke(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(perm.to_string())
}

#[op]
pub fn op_request_permission(
  state: &mut OpState,
  args: PermissionArgs,
) -> Result<String, AnyError> {
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.request(path.map(Path::new)),
    "write" => permissions.write.request(path.map(Path::new)),
    "net" => permissions.net.request(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.request(args.variable.as_deref()),
    "run" => permissions.run.request(args.command.as_deref()),
    "ffi" => permissions.ffi.request(args.path.as_deref().map(Path::new)),
    "hrtime" => permissions.hrtime.request(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(perm.to_string())
}

fn parse_host(host_str: &str) -> Result<(String, Option<u16>), AnyError> {
  let url = url::Url::parse(&format!("http://{}/", host_str))
    .map_err(|_| uri_error("Invalid host"))?;
  if url.path() != "/" {
    return Err(uri_error("Invalid host"));
  }
  let hostname = url.host_str().unwrap();
  Ok((hostname.to_string(), url.port()))
}
