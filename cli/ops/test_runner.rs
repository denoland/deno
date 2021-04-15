// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_runtime::ops::worker_host::create_worker_permissions;
use deno_runtime::ops::worker_host::PermissionsArg;
use deno_runtime::permissions::Permissions;
use uuid::Uuid;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_sync(rt, "op_pledge_test_permissions", op_pledge_test_permissions);
  super::reg_sync(
    rt,
    "op_restore_test_permissions",
    op_restore_test_permissions,
  );
}

#[derive(Clone)]
struct PermissionsHolder(Uuid, Permissions);

pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow::<Permissions>().clone();
  let worker_permissions = {
    let permissions: PermissionsArg = serde_json::from_value(args)?;
    create_worker_permissions(parent_permissions.clone(), permissions)?
  };

  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));
  state.put::<Permissions>(worker_permissions);

  Ok(json!(token))
}

pub fn op_restore_test_permissions(
  state: &mut OpState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  if let Some(permissions_holder) = state.try_take::<PermissionsHolder>() {
    let token: Uuid = serde_json::from_value(args)?;
    if token != permissions_holder.0 {
      panic!("restore test permissions token does not match the stored token");
    }

    let permissions = permissions_holder.1.clone();
    state.put::<Permissions>(permissions);
  }

  Ok(json!({}))
}
