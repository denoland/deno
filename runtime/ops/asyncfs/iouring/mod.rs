use crate::ops::fs::write_open_options;

use crate::ops::fs::OpenOptions;
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
use std::path::PathBuf;
use std::rc::Rc;
use tokio_uring::fs::File;

#[inline]
pub(crate) fn open_helper_async(
  state: &mut OpState,
  path: &str,
  mode: Option<u32>,
  options: Option<&OpenOptions>,
  api_name: &str,
) -> Result<(PathBuf, tokio_uring::fs::OpenOptions), AnyError> {
  let path = Path::new(path).to_path_buf();

  let mut open_options = tokio_uring::fs::OpenOptions::new();

  if let Some(mode) = mode {
    // mode only used if creating the file on Unix
    // if not specified, defaults to 0o666
    #[cfg(unix)]
    {
      use std::os::unix::fs::OpenOptionsExt;
      open_options.mode(mode & 0o777);
    }
    #[cfg(not(unix))]
    let _ = mode; // avoid unused warning
  }

  let permissions = state.borrow_mut::<Permissions>();

  match options {
    None => {
      permissions.read.check(&path, Some(api_name))?;
      open_options
        .read(true)
        .create(false)
        .write(false)
        .truncate(false)
        .append(false)
        .create_new(false);
    }
    Some(options) => {
      if options.read {
        permissions.read.check(&path, Some(api_name))?;
      }

      if options.write || options.append {
        permissions.write.check(&path, Some(api_name))?;
      }

      open_options
        .read(options.read)
        .create(options.create)
        .write(options.write)
        .truncate(options.truncate)
        .append(options.append)
        .create_new(options.create_new);
    }
  }

  Ok((path, open_options))
}

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
  let (path, open_options) = open_helper_async(
    &mut *state.borrow_mut(),
    &path,
    mode,
    Some(&write_open_options(create, append)),
    "Deno.writeFile()",
  )?;
  let write_future = async move {
    let file = open_options.open(&path).await?;
    let (res, _) = file.write_all_at(data, 0).await;
    res?;
    Ok::<(), AnyError>(())
  };
  if let Some(cancel_handle) = cancel_handle {
    write_future.or_cancel(cancel_handle).await??;
  } else {
    write_future.await?;
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

  // TODO(@littledivy): Stat file to get the size.
  let fut = async move {
    let path = Path::new(&path);
    let file = File::open(&path).await?;
    let mut buf = vec![0; 1024 * 16];
    let mut offset: u64 = 0;
    loop {
      let (res, mut ret) =
        file.read_at(buf.split_off(offset as usize), offset).await;
      let nread = res? as u64;
      offset = offset + nread;
      if nread == 0 {
        break Ok::<ZeroCopyBuf, AnyError>(ret.into());
      }
      ret.resize(offset as usize * 2, 0);
      buf = ret;
    }
  };
  if let Some(cancel_rid) = cancel_rid {
    let cancel_handle = state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(cancel_rid);
    if let Ok(cancel_handle) = cancel_handle {
      return fut.or_cancel(cancel_handle).await?;
    }
  }
  fut.await
}
