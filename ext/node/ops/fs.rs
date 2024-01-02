// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_fs::FileSystemRc;

use crate::NodePermissions;

#[op2(fast)]
pub fn op_node_fs_exists_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<bool, AnyError>
where
  P: NodePermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_read_with_api_name(&path, Some("node:fs.existsSync()"))?;
  let fs = state.borrow::<FileSystemRc>();
  Ok(fs.lstat_sync(&path).is_ok())
}
