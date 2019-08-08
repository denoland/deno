// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;

pub fn op_permissions(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::PermissionsRes::create(
    builder,
    &msg::PermissionsResArgs {
      run: state.permissions.allows_run(),
      read: state.permissions.allows_read(),
      write: state.permissions.allows_write(),
      net: state.permissions.allows_net(),
      env: state.permissions.allows_env(),
      hrtime: state.permissions.allows_hrtime(),
    },
  );
  let response_buf = serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::PermissionsRes,
      ..Default::default()
    },
  );
  ok_buf(response_buf)
}

pub fn op_revoke_permission(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_permission_revoke().unwrap();
  let permission = inner.permission().unwrap();
  match permission {
    "run" => state.permissions.revoke_run(),
    "read" => state.permissions.revoke_read(),
    "write" => state.permissions.revoke_write(),
    "net" => state.permissions.revoke_net(),
    "env" => state.permissions.revoke_env(),
    "hrtime" => state.permissions.revoke_hrtime(),
    _ => Ok(()),
  }?;
  ok_buf(empty_buf())
}
