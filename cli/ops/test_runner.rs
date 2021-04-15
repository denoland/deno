// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_runtime::permissions::Permissions;
use deno_runtime::ops::worker_host::create_worker_permissions;
use deno_runtime::ops::worker_host::PermissionsArg;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_sync(rt, "op_pledge_test_permissions", op_pledge_test_permissions);
  super::reg_sync(
    rt,
    "op_restore_test_permissions",
    op_restore_test_permissions,
  );
}

struct PermissionsHolder(Permissions);

pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let parent_permissions = state.borrow::<Permissions>().clone();
  let worker_permissions = {
    let permissions : PermissionsArg = serde_json::from_value(args)?;
    create_worker_permissions(parent_permissions.clone(), permissions)?
  };

  state.put::<PermissionsHolder>(PermissionsHolder(parent_permissions.clone()));
  state.put::<Permissions>(worker_permissions);

  Ok(json!({}))
}

pub fn op_restore_test_permissions(
  state: &mut OpState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  if let Some(permissions_holder) = state.try_borrow::<PermissionsHolder>() {
      let permissions = permissions_holder.0.clone();
      state.put::<Permissions>(permissions);
  }

  Ok(json!({}))
}
