use crate::ops::fs::open_helper;
use crate::ops::fs::write_file;
use crate::ops::fs::write_open_options;

use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

#[op]
pub async fn op_write_file_async(
  state: Rc<RefCell<OpState>>,
  path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  data: ZeroCopyBuf,
  cancel_rid: Option<ResourceId>,
) -> Result<(), AnyError> {
  let cancel_handle = match cancel_rid {
    Some(cancel_rid) => state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(cancel_rid)
      .ok(),
    None => None,
  };
  let (path, open_options) = open_helper(
    &mut *state.borrow_mut(),
    &path,
    mode,
    Some(&write_open_options(create, append)),
    "Deno.writeFile()",
  )?;
  let write_future = tokio::task::spawn_blocking(move || {
    write_file(&path, open_options, mode, data)
  });
  if let Some(cancel_handle) = cancel_handle {
    write_future.or_cancel(cancel_handle).await???;
  } else {
    write_future.await??;
  }
  Ok(())
}

#[op]
async fn op_readfile_async(
  state: Rc<RefCell<OpState>>,
  path: String,
  cancel_rid: Option<ResourceId>,
) -> Result<ZeroCopyBuf, AnyError> {
  {
    let path = Path::new(&path);
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<Permissions>()
      .read
      .check(path, Some("Deno.readFile()"))?;
  }
  let fut = tokio::task::spawn_blocking(move || {
    let path = Path::new(&path);
    Ok(std::fs::read(path).map(ZeroCopyBuf::from)?)
  });
  if let Some(cancel_rid) = cancel_rid {
    let cancel_handle = state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(cancel_rid);
    if let Ok(cancel_handle) = cancel_handle {
      return fut.or_cancel(cancel_handle).await??;
    }
  }
  fut.await?
}
