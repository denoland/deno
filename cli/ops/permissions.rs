// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;

// Permissions

pub struct OpPermissions;

impl DenoOpDispatcher for OpPermissions {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let state = state.clone();
        Ok(JsonOp::Sync(json!({
          "run": state.permissions.allows_run(),
          "read": state.permissions.allows_read(),
          "write": state.permissions.allows_write(),
          "net": state.permissions.allows_net(),
          "env": state.permissions.allows_env(),
          "hrtime": state.permissions.allows_hrtime(),
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "permissions";
}

// Revoke Permission

pub struct OpRevokePermission;

#[derive(Deserialize)]
struct RevokePermissionArgs {
  permission: String,
}

impl DenoOpDispatcher for OpRevokePermission {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: RevokePermissionArgs = serde_json::from_value(args)?;
        let permission = args.permission.as_ref();
        let state = state.clone();
        match permission {
          "run" => state.permissions.revoke_run(),
          "read" => state.permissions.revoke_read(),
          "write" => state.permissions.revoke_write(),
          "net" => state.permissions.revoke_net(),
          "env" => state.permissions.revoke_env(),
          "hrtime" => state.permissions.revoke_hrtime(),
          _ => Ok(()),
        }?;

        Ok(JsonOp::Sync(json!({})))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "revokePermission";
}
