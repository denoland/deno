// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_fs::FileSystemRc;
use deno_fs::OpenOptions;
use deno_io::fs::FileResource;
use deno_permissions::OpenAccessKind;
use serde::Serialize;

use crate::NodePermissions;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FsError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error("{0}")]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
  #[cfg(windows)]
  #[class(generic)]
  #[error("Path has no root.")]
  PathHasNoRoot,
  #[cfg(not(any(unix, windows)))]
  #[class(generic)]
  #[error("Unsupported platform.")]
  UnsupportedPlatform,
  #[class(inherit)]
  #[error(transparent)]
  Fs(
    #[from]
    #[inherit]
    deno_io::fs::FsError,
  ),
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_exists_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<bool, deno_permissions::PermissionCheckError>
where
  P: NodePermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.existsSync()"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  Ok(fs.exists_sync(&path))
}

#[op2(async, stack_trace)]
pub async fn op_node_fs_exists<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<bool, FsError>
where
  P: NodePermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.exists()"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  Ok(fs.exists_async(path.into_owned()).await?)
}

#[op2(fast, stack_trace)]
pub fn op_node_cp_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[string] new_path: &str,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::Read,
    Some("node:fs.cpSync"),
  )?;
  let new_path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(new_path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.cpSync"),
  )?;

  let fs = state.borrow::<FileSystemRc>();
  fs.cp_sync(&path, &new_path)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_node_cp<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[string] new_path: String,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let (fs, path, new_path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::Read,
      Some("node:fs.cpSync"),
    )?;
    let new_path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(new_path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.cpSync"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path, new_path)
  };

  fs.cp_async(path.into_owned(), new_path.into_owned())
    .await?;
  Ok(())
}

fn get_open_options(mut flags: i32, mode: u32) -> OpenOptions {
  let mut options = OpenOptions {
    mode: Some(mode),
    ..Default::default()
  };

  if (flags & libc::O_APPEND) == libc::O_APPEND {
    options.append = true;
    flags &= !libc::O_APPEND;
  }
  if (flags & libc::O_CREAT) == libc::O_CREAT {
    options.create = true;
    flags &= !libc::O_CREAT;
  }
  if (flags & libc::O_EXCL) == libc::O_EXCL {
    options.create_new = true;
    options.write = true;
    flags &= !libc::O_EXCL;
  }
  if (flags & libc::O_RDWR) == libc::O_RDWR {
    options.read = true;
    options.write = true;
    flags &= !libc::O_RDWR;
  }
  if (flags & libc::O_TRUNC) == libc::O_TRUNC {
    options.truncate = true;
    flags &= !libc::O_TRUNC;
  }
  if (flags & libc::O_WRONLY) == libc::O_WRONLY {
    options.write = true;
    flags &= !libc::O_WRONLY;
  }

  if flags != 0 {
    options.custom_flags = Some(flags);
  }

  if !options.append
    && !options.create
    && !options.create_new
    && !options.read
    && !options.truncate
    && !options.write
  {
    options.read = true;
  }
  options
}

fn open_options_to_access_kind(open_options: &OpenOptions) -> OpenAccessKind {
  let read = open_options.read;
  let write = open_options.write || open_options.append;
  match (read, write) {
    (true, true) => OpenAccessKind::ReadWrite,
    (false, true) => OpenAccessKind::Write,
    (true, false) | (false, false) => OpenAccessKind::Read,
  }
}

#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_open_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[smi] flags: i32,
  #[smi] mode: u32,
) -> Result<ResourceId, FsError>
where
  P: NodePermissions + 'static,
{
  let path = Path::new(path);
  let options = get_open_options(flags, mode);

  let fs = state.borrow::<FileSystemRc>().clone();
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(path),
    open_options_to_access_kind(&options),
    Some("node:fs.openSync"),
  )?;
  let file = fs.open_sync(&path, options)?;
  let rid = state
    .resource_table
    .add(FileResource::new(file, "fsFile".to_string()));
  Ok(rid)
}

#[op2(async, stack_trace)]
#[smi]
pub async fn op_node_open<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] flags: i32,
  #[smi] mode: u32,
) -> Result<ResourceId, FsError>
where
  P: NodePermissions + 'static,
{
  let path = PathBuf::from(path);
  let options = get_open_options(flags, mode);

  let (fs, path) = {
    let mut state = state.borrow_mut();
    (
      state.borrow::<FileSystemRc>().clone(),
      state.borrow_mut::<P>().check_open(
        Cow::Owned(path),
        open_options_to_access_kind(&options),
        Some("node:fs.open"),
      )?,
    )
  };
  let file = fs.open_async(path.as_owned(), options).await?;

  let rid = state
    .borrow_mut()
    .resource_table
    .add(FileResource::new(file, "fsFile".to_string()));
  Ok(rid)
}

#[derive(Debug, Serialize)]
pub struct StatFs {
  #[serde(rename = "type")]
  pub typ: u64,
  pub bsize: u64,
  pub blocks: u64,
  pub bfree: u64,
  pub bavail: u64,
  pub files: u64,
  pub ffree: u64,
}

#[op2(stack_trace)]
#[serde]
pub fn op_node_statfs<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: &str,
  bigint: bool,
) -> Result<StatFs, FsError>
where
  P: NodePermissions + 'static,
{
  let path = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Borrowed(Path::new(path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.statfs"),
    )?;
    state
      .borrow_mut::<P>()
      .check_sys("statfs", "node:fs.statfs")?;
    path
  };
  #[cfg(unix)]
  {
    use std::os::unix::ffi::OsStrExt;

    let path = path.as_os_str();
    let mut cpath = path.as_bytes().to_vec();
    cpath.push(0);
    if bigint {
      #[cfg(not(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd"
      )))]
      // SAFETY: `cpath` is NUL-terminated and result is pointer to valid statfs memory.
      let (code, result) = unsafe {
        let mut result: libc::statfs64 = std::mem::zeroed();
        (libc::statfs64(cpath.as_ptr() as _, &mut result), result)
      };
      #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd"
      ))]
      // SAFETY: `cpath` is NUL-terminated and result is pointer to valid statfs memory.
      let (code, result) = unsafe {
        let mut result: libc::statfs = std::mem::zeroed();
        (libc::statfs(cpath.as_ptr() as _, &mut result), result)
      };
      if code == -1 {
        return Err(std::io::Error::last_os_error().into());
      }
      Ok(StatFs {
        #[cfg(not(target_os = "openbsd"))]
        typ: result.f_type as _,
        #[cfg(target_os = "openbsd")]
        typ: 0 as _,
        bsize: result.f_bsize as _,
        blocks: result.f_blocks as _,
        bfree: result.f_bfree as _,
        bavail: result.f_bavail as _,
        files: result.f_files as _,
        ffree: result.f_ffree as _,
      })
    } else {
      // SAFETY: `cpath` is NUL-terminated and result is pointer to valid statfs memory.
      let (code, result) = unsafe {
        let mut result: libc::statfs = std::mem::zeroed();
        (libc::statfs(cpath.as_ptr() as _, &mut result), result)
      };
      if code == -1 {
        return Err(std::io::Error::last_os_error().into());
      }
      Ok(StatFs {
        #[cfg(not(target_os = "openbsd"))]
        typ: result.f_type as _,
        #[cfg(target_os = "openbsd")]
        typ: 0 as _,
        bsize: result.f_bsize as _,
        blocks: result.f_blocks as _,
        bfree: result.f_bfree as _,
        bavail: result.f_bavail as _,
        files: result.f_files as _,
        ffree: result.f_ffree as _,
      })
    }
  }
  #[cfg(windows)]
  {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceW;

    let _ = bigint;
    // Using a vfs here doesn't make sense, it won't align with the windows API
    // call below.
    #[allow(clippy::disallowed_methods)]
    let path = path.canonicalize()?;
    let root = path.ancestors().last().ok_or(FsError::PathHasNoRoot)?;
    let mut root = OsStr::new(root).encode_wide().collect::<Vec<_>>();
    root.push(0);
    let mut sectors_per_cluster = 0;
    let mut bytes_per_sector = 0;
    let mut available_clusters = 0;
    let mut total_clusters = 0;
    let mut code = 0;
    let mut retries = 0;
    // We retry here because libuv does: https://github.com/libuv/libuv/blob/fa6745b4f26470dae5ee4fcbb1ee082f780277e0/src/win/fs.c#L2705
    while code == 0 && retries < 2 {
      // SAFETY: Normal GetDiskFreeSpaceW usage.
      code = unsafe {
        GetDiskFreeSpaceW(
          root.as_ptr(),
          &mut sectors_per_cluster,
          &mut bytes_per_sector,
          &mut available_clusters,
          &mut total_clusters,
        )
      };
      retries += 1;
    }
    if code == 0 {
      return Err(std::io::Error::last_os_error().into());
    }
    Ok(StatFs {
      typ: 0,
      bsize: (bytes_per_sector * sectors_per_cluster) as _,
      blocks: total_clusters as _,
      bfree: available_clusters as _,
      bavail: available_clusters as _,
      files: 0,
      ffree: 0,
    })
  }
  #[cfg(not(any(unix, windows)))]
  {
    let _ = path;
    let _ = bigint;
    Err(FsError::UnsupportedPlatform)
  }
}

#[op2(fast, stack_trace)]
pub fn op_node_lutimes_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lutimes"),
  )?;

  let fs = state.borrow::<FileSystemRc>();
  fs.lutime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_node_lutimes<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lutimesSync"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  fs.lutime_async(
    path.into_owned(),
    atime_secs,
    atime_nanos,
    mtime_secs,
    mtime_nanos,
  )
  .await?;

  Ok(())
}

#[op2(stack_trace)]
pub fn op_node_lchown_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchownSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchown_sync(&path, uid, gid)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_node_lchown<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lchown"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.lchown_async(path.into_owned(), uid, gid).await?;
  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_node_lchmod_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[smi] mode: u32,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchmodSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchmod_sync(&path, mode)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_node_lchmod<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] mode: u32,
) -> Result<(), FsError>
where
  P: NodePermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lchmod"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.lchmod_async(path.into_owned(), mode).await?;
  Ok(())
}
