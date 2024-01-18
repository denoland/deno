// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

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

#[op2(fast)]
pub fn op_node_cp_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[string] new_path: &str,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  let path = Path::new(path);
  let new_path = Path::new(new_path);

  state
    .borrow_mut::<P>()
    .check_read_with_api_name(path, Some("node:fs.cpSync"))?;
  state
    .borrow_mut::<P>()
    .check_write_with_api_name(new_path, Some("node:fs.cpSync"))?;

  let fs = state.borrow::<FileSystemRc>();
  fs.cp_sync(path, new_path)?;
  Ok(())
}

#[op2(async)]
pub async fn op_node_cp<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[string] new_path: String,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  let path = PathBuf::from(path);
  let new_path = PathBuf::from(new_path);

  let fs = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read_with_api_name(&path, Some("node:fs.cpSync"))?;
    state
      .borrow_mut::<P>()
      .check_write_with_api_name(&new_path, Some("node:fs.cpSync"))?;
    state.borrow::<FileSystemRc>().clone()
  };

  fs.cp_async(path, new_path).await?;
  Ok(())
}
