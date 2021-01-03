// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::custom_error;
use deno_core::error::uri_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
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
struct PermissionArgs {
  name: String,
  path: Option<String>,
  host: Option<String>,
}

pub fn op_query_permission(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let permissions = state.borrow::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.query_read(&path.as_deref().map(Path::new)),
    "write" => permissions.query_write(&path.as_deref().map(Path::new)),
    "net" => permissions.query_net(
      &match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.query_env(),
    "run" => permissions.query_run(),
    "plugin" => permissions.query_plugin(),
    "hrtime" => permissions.query_hrtime(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(json!({ "state": perm.to_string() }))
}

pub fn op_revoke_permission(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.revoke_read(&path.as_deref().map(Path::new)),
    "write" => permissions.revoke_write(&path.as_deref().map(Path::new)),
    "net" => permissions.revoke_net(
      &match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.revoke_env(),
    "run" => permissions.revoke_run(),
    "plugin" => permissions.revoke_plugin(),
    "hrtime" => permissions.revoke_hrtime(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(json!({ "state": perm.to_string() }))
}

pub fn op_request_permission(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: PermissionArgs = serde_json::from_value(args)?;
  let permissions = state.borrow_mut::<Permissions>();
  let path = args.path.as_deref();
  let perm = match args.name.as_ref() {
    "read" => permissions.request_read(&path.as_deref().map(Path::new)),
    "write" => permissions.request_write(&path.as_deref().map(Path::new)),
    "net" => permissions.request_net(
      &match args.host.as_deref() {
        None => None,
        Some(h) => Some(parse_host(h)?),
      }
      .as_ref(),
    ),
    "env" => permissions.request_env(),
    "run" => permissions.request_run(),
    "plugin" => permissions.request_plugin(),
    "hrtime" => permissions.request_hrtime(),
    n => {
      return Err(custom_error(
        "ReferenceError",
        format!("No such permission name: {}", n),
      ))
    }
  };
  Ok(json!({ "state": perm.to_string() }))
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
