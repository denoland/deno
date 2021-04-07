// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::custom_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::url;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::path::Path;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_query_permission", op_query_permission);
  super::reg_json_sync(rt, "op_revoke_permission", op_revoke_permission);
  super::reg_json_sync(rt, "op_request_permission", op_request_permission);
}

#[derive(Deserialize)]
pub struct PermissionArgs {
  name: String,
  path: Option<String>,
  host: Option<String>,
}

pub fn op_query_permission(
  state: &mut OpState,
  args: PermissionArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let permissions = state.borrow::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.query(path.as_deref().map(Path::new)),
    "write" => permissions.write.query(path.as_deref().map(Path::new)),
    "net" => permissions.net.query(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.query(),
    "run" => permissions.run.query(),
    "plugin" => permissions.plugin.query(),
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

pub fn op_revoke_permission(
  state: &mut OpState,
  args: PermissionArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.revoke(path.as_deref().map(Path::new)),
    "write" => permissions.write.revoke(path.as_deref().map(Path::new)),
    "net" => permissions.net.revoke(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.revoke(),
    "run" => permissions.run.revoke(),
    "plugin" => permissions.plugin.revoke(),
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

pub fn op_request_permission(
  state: &mut OpState,
  args: PermissionArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.read.request(path.as_deref().map(Path::new)),
    "write" => permissions.write.request(path.as_deref().map(Path::new)),
    "net" => permissions.net.request(
      match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.env.request(),
    "run" => permissions.run.request(),
    "plugin" => permissions.plugin.request(),
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
