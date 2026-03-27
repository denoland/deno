// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
#[cfg(feature = "sync_fs")]
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_fs::FileSystemRc;
use deno_fs::FsFileType;
use deno_fs::OpenOptions;
use deno_io::fs::FileResource;
use deno_io::fs::FsResult;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use serde::Serialize;
#[cfg(feature = "sync_fs")]
use tokio::task::JoinError;

use crate::ops::constant::UV_FS_COPYFILE_EXCL;

/// When `sync_fs` is enabled, `FileSystemRc` is `Arc` (Send) and we can
/// offload work to a blocking thread. Otherwise, run inline.
macro_rules! maybe_spawn_blocking {
  ($f:expr) => {{
    #[cfg(feature = "sync_fs")]
    {
      spawn_blocking($f).await.unwrap()
    }
    #[cfg(not(feature = "sync_fs"))]
    {
      ($f)()
    }
  }};
}

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
  #[class(inherit)]
  #[error(transparent)]
  Cp(#[from] CpError),
  #[class(inherit)]
  #[error(transparent)]
  NodeFs(#[from] NodeFsError),
  #[cfg(feature = "sync_fs")]
  #[class(inherit)]
  #[error(transparent)]
  JoinError(#[from] JoinError),
}

#[derive(Debug, Default)]
pub struct NodeFsErrorContext {
  message: Option<String>,
  path: Option<String>,
  dest: Option<String>,
  syscall: Option<String>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
// Intentionally set the error message to be empty, so that in the JS side it will fall back to the UV error message
// https://github.com/denoland/deno/blob/f123d84e7d6e8036c3c38ecec6e25deb87a8829b/ext/node/polyfills/internal/errors.ts#L268-L270
#[error("")]
#[property("os_errno" = self.os_errno)]
#[property("message" = self.context.message.clone().unwrap_or_default().clone())]
#[property("path" = self.context.path.clone().unwrap_or_default().clone())]
#[property("dest" = self.context.dest.clone().unwrap_or_default().clone())]
#[property("syscall" = self.context.syscall.clone().unwrap_or_default().clone())]
pub struct NodeFsError {
  os_errno: i32,
  context: NodeFsErrorContext,
}

fn map_fs_error_to_node_fs_error(
  err: deno_io::fs::FsError,
  context: NodeFsErrorContext,
) -> FsError {
  let os_errno = match err {
    deno_io::fs::FsError::Io(ref io_err) => io_err.raw_os_error(),
    _ => None,
  };

  if let Some(os_errno) = os_errno {
    return NodeFsError { os_errno, context }.into();
  }

  FsError::Fs(err)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("{message}")]
#[property("kind" = self.kind())]
#[property("message" = self.message())]
#[property("path" = self.path())]
pub enum CpError {
  EInval { message: String, path: String },
  DirToNonDir { message: String, path: String },
  NonDirToDir { message: String, path: String },
  EExist { message: String, path: String },
  EIsDir { message: String, path: String },
  Socket { message: String, path: String },
  Fifo { message: String, path: String },
  Unknown { message: String, path: String },
  SymlinkToSubdirectory { message: String, path: String },
}

impl CpError {
  fn kind(&self) -> &'static str {
    match self {
      CpError::EInval { .. } => "EINVAL",
      CpError::DirToNonDir { .. } => "DIR_TO_NON_DIR",
      CpError::NonDirToDir { .. } => "NON_DIR_TO_DIR",
      CpError::EExist { .. } => "EEXIST",
      CpError::EIsDir { .. } => "EISDIR",
      CpError::Socket { .. } => "SOCKET",
      CpError::Fifo { .. } => "FIFO",
      CpError::Unknown { .. } => "UNKNOWN",
      CpError::SymlinkToSubdirectory { .. } => "SYMLINK_TO_SUBDIRECTORY",
    }
  }

  fn message(&self) -> String {
    match self {
      CpError::EInval { message, .. } => message.clone(),
      CpError::DirToNonDir { message, .. } => message.clone(),
      CpError::NonDirToDir { message, .. } => message.clone(),
      CpError::EExist { message, .. } => message.clone(),
      CpError::EIsDir { message, .. } => message.clone(),
      CpError::Socket { message, .. } => message.clone(),
      CpError::Fifo { message, .. } => message.clone(),
      CpError::Unknown { message, .. } => message.clone(),
      CpError::SymlinkToSubdirectory { message, .. } => message.clone(),
    }
  }

  fn path(&self) -> String {
    match self {
      CpError::EInval { path, .. } => path.clone(),
      CpError::DirToNonDir { path, .. } => path.clone(),
      CpError::NonDirToDir { path, .. } => path.clone(),
      CpError::EExist { path, .. } => path.clone(),
      CpError::EIsDir { path, .. } => path.clone(),
      CpError::Socket { path, .. } => path.clone(),
      CpError::Fifo { path, .. } => path.clone(),
      CpError::Unknown { path, .. } => path.clone(),
      CpError::SymlinkToSubdirectory { path, .. } => path.clone(),
    }
  }
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_exists_sync(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<bool, deno_permissions::PermissionCheckError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.existsSync()"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  Ok(fs.exists_sync(&path))
}

#[op2(stack_trace)]
pub async fn op_node_fs_exists(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<bool, FsError> {
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.exists()"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  Ok(fs.exists_async(path.into_owned()).await?)
}

fn get_open_options(flags: i32, mode: Option<u32>) -> OpenOptions {
  let mut options = OpenOptions::from(flags);
  options.mode = mode;
  options
}

fn open_options_to_access_kind(open_options: &OpenOptions) -> OpenAccessKind {
  let read = open_options.read;
  let write = open_options.write || open_options.append || open_options.create;
  match (read, write) {
    (true, true) => OpenAccessKind::ReadWrite,
    (false, true) => OpenAccessKind::Write,
    (true, false) | (false, false) => OpenAccessKind::Read,
  }
}

#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_open_sync(
  state: &mut OpState,
  #[string] path: &str,
  #[smi] flags: i32,
  #[smi] mode: u32,
) -> Result<ResourceId, FsError> {
  let path = Path::new(path);
  let options = get_open_options(flags, Some(mode));

  let fs = state.borrow::<FileSystemRc>().clone();
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
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

#[op2(stack_trace)]
#[smi]
pub async fn op_node_open(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] flags: i32,
  #[smi] mode: u32,
) -> Result<ResourceId, FsError> {
  let path = PathBuf::from(path);
  let options = get_open_options(flags, Some(mode));

  let (fs, path) = {
    let mut state = state.borrow_mut();
    (
      state.borrow::<FileSystemRc>().clone(),
      state.borrow_mut::<PermissionsContainer>().check_open(
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
pub fn op_node_statfs_sync(
  state: &mut OpState,
  #[string] path: &str,
  bigint: bool,
) -> Result<StatFs, FsError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.statfsSync"),
  )?;
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("statfs", "node:fs.statfsSync")?;

  statfs(path, bigint)
}

#[op2(stack_trace)]
#[serde]
#[allow(clippy::unused_async, reason = "sometimes async")]
pub async fn op_node_statfs(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  bigint: bool,
) -> Result<StatFs, FsError> {
  let path = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.statfs"),
    )?;
    state
      .borrow_mut::<PermissionsContainer>()
      .check_sys("statfs", "node:fs.statfs")?;
    path
  };

  maybe_spawn_blocking!(move || statfs(path, bigint))
}

// TODO(dsherret): move this method onto FileSystem trait as this is completely
// bypassing the FileSystem trait.
fn statfs(path: CheckedPath, bigint: bool) -> Result<StatFs, FsError> {
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
    #[allow(clippy::disallowed_methods, reason = "TODO: move onto RealFs")]
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
pub fn op_node_lutimes_sync(
  state: &mut OpState,
  #[string] path: &str,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lutimes"),
  )?;

  let fs = state.borrow::<FileSystemRc>();
  fs.lutime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)?;
  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_lutimes(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsError> {
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
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
pub fn op_node_lchown_sync(
  state: &mut OpState,
  #[string] path: &str,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchownSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchown_sync(&path, uid, gid)?;
  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_lchown(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsError> {
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
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
pub fn op_node_lchmod_sync(
  state: &mut OpState,
  #[string] path: &str,
  #[smi] mode: u32,
) -> Result<(), FsError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchmodSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchmod_sync(&path, mode)?;
  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_lchmod(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] mode: u32,
) -> Result<(), FsError> {
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lchmod"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.lchmod_async(path.into_owned(), mode).await?;
  Ok(())
}

#[op2(stack_trace)]
#[string]
pub fn op_node_mkdtemp_sync(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<String, FsError> {
  // https://github.com/nodejs/node/blob/2ea31e53c61463727c002c2d862615081940f355/deps/uv/src/unix/os390-syscalls.c#L409
  for _ in 0..libc::TMP_MAX {
    let path = temp_path_append_suffix(path);
    let checked_path = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Borrowed(Path::new(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.mkdtempSync()"),
    )?;
    let fs = state.borrow::<FileSystemRc>();

    match fs.mkdir_sync(&checked_path, false, Some(0o700)) {
      Ok(()) => return Ok(path),
      Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(err) => return Err(FsError::Fs(err)),
    }
  }

  Err(FsError::Io(std::io::Error::new(
    std::io::ErrorKind::AlreadyExists,
    "too many temp dirs exist",
  )))
}

#[op2(stack_trace)]
#[string]
pub async fn op_node_mkdtemp(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<String, FsError> {
  // https://github.com/nodejs/node/blob/2ea31e53c61463727c002c2d862615081940f355/deps/uv/src/unix/os390-syscalls.c#L409
  for _ in 0..libc::TMP_MAX {
    let path = temp_path_append_suffix(&path);
    let (fs, checked_path) = {
      let mut state = state.borrow_mut();
      let checked_path =
        state.borrow_mut::<PermissionsContainer>().check_open(
          Cow::Owned(PathBuf::from(path.clone())),
          OpenAccessKind::WriteNoFollow,
          Some("node:fs.mkdtemp()"),
        )?;
      (state.borrow::<FileSystemRc>().clone(), checked_path)
    };

    match fs
      .mkdir_async(checked_path.into_owned(), false, Some(0o700))
      .await
    {
      Ok(()) => return Ok(path),
      Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(err) => return Err(FsError::Fs(err)),
    }
  }

  Err(FsError::Io(std::io::Error::new(
    std::io::ErrorKind::AlreadyExists,
    "too many temp dirs exist",
  )))
}

fn temp_path_append_suffix(prefix: &str) -> String {
  use rand::Rng;
  use rand::distributions::Alphanumeric;
  use rand::rngs::OsRng;

  let suffix: String =
    (0..6).map(|_| OsRng.sample(Alphanumeric) as char).collect();
  format!("{}{}", prefix, suffix)
}

/// Create a file resource from a raw file descriptor.
/// This is used for wrapping PTYs and other non-socket file descriptors
/// that can't be wrapped as Unix streams.
#[cfg(unix)]
#[op2(fast)]
#[smi]
pub fn op_node_file_from_fd(
  state: &mut OpState,
  fd: i32,
) -> Result<ResourceId, FsError> {
  use std::fs::File as StdFile;
  use std::os::unix::io::FromRawFd;

  if fd < 0 {
    return Err(FsError::Io(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "Invalid file descriptor",
    )));
  }

  // SAFETY: The caller is responsible for passing a valid fd that they own.
  // The fd will be owned by the created File from this point on.
  let std_file = unsafe { StdFile::from_raw_fd(fd) };

  let file = Rc::new(deno_io::StdFileResourceInner::file(std_file, None));
  let rid = state
    .resource_table
    .add(FileResource::new(file, "pipe".to_string()));
  Ok(rid)
}

#[cfg(not(unix))]
#[op2(fast)]
#[smi]
pub fn op_node_file_from_fd(
  _state: &mut OpState,
  _fd: i32,
) -> Result<ResourceId, FsError> {
  Err(FsError::Io(std::io::Error::new(
    std::io::ErrorKind::Unsupported,
    "op_node_file_from_fd is not supported on this platform",
  )))
}

#[op2(fast, stack_trace)]
pub fn op_node_rmdir_sync(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<(), FsError> {
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.rmdirSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.rmdir_sync(&path)?;
  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_rmdir(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<(), FsError> {
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.rmdir"),
    )?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.rmdir_async(path.into_owned()).await?;
  Ok(())
}

const CP_IS_DEST_EXISTS_FLAG: u64 = 1u64 << 32;
const CP_IS_SRC_DIRECTORY_FLAG: u64 = 1u64 << 33;
const CP_IS_SRC_FILE_FLAG: u64 = 1u64 << 34;
const CP_IS_SRC_CHAR_DEVICE_FLAG: u64 = 1u64 << 35;
const CP_IS_SRC_BLOCK_DEVICE_FLAG: u64 = 1u64 << 36;
const CP_IS_SRC_SYMLINK_FLAG: u64 = 1u64 << 37;
const CP_IS_SRC_SOCKET_FLAG: u64 = 1u64 << 38;
const CP_IS_SRC_FIFO_FLAG: u64 = 1u64 << 39;

/// Bit-packed stat metadata passed to JS for `cp` operations.
///
/// Layout (u64):
/// - bits 0..31  : src mode (`st_mode`)
/// - bit 32      : destination exists
/// - bits 33..39 : source type flags (dir, file, char/block device, symlink, socket, fifo)
fn compact_stat_info(
  src_stat: &deno_io::fs::FsStat,
  is_dest_exists: bool,
) -> u64 {
  let mut packed = src_stat.mode as u64;
  if is_dest_exists {
    packed |= CP_IS_DEST_EXISTS_FLAG;
  }
  if src_stat.is_file {
    packed |= CP_IS_SRC_FILE_FLAG;
  }
  if src_stat.is_directory {
    packed |= CP_IS_SRC_DIRECTORY_FLAG;
  }
  if src_stat.is_char_device {
    packed |= CP_IS_SRC_CHAR_DEVICE_FLAG;
  }
  if src_stat.is_block_device {
    packed |= CP_IS_SRC_BLOCK_DEVICE_FLAG;
  }
  if src_stat.is_symlink {
    packed |= CP_IS_SRC_SYMLINK_FLAG;
  }
  if src_stat.is_socket {
    packed |= CP_IS_SRC_SOCKET_FLAG;
  }
  if src_stat.is_fifo {
    packed |= CP_IS_SRC_FIFO_FLAG;
  }
  packed
}

struct CpCheckPathsSyncResult {
  is_dest_exists: bool,
  is_src_directory: bool,
  is_src_file: bool,
  is_src_char_device: bool,
  is_src_block_device: bool,
  is_src_symlink: bool,
  is_src_socket: bool,
  is_src_fifo: bool,
  src_mode: u32,
  src_dev: u64,
  src_ino: u64,
}

// To save the cost of multiple round trips between Rust and JS,
// we pack the necessary metadata for `cp` operations into a single u64 value and pass it to JS.
#[derive(Debug)]
struct CpCheckPathsResult {
  src_dev: u64,
  src_ino: u64,
  stat_info: u64,
}

#[derive(Clone, Copy)]
struct CpSyncOptions<'a> {
  dereference: bool,
  recursive: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  verbatim_symlinks: bool,
  mode: u32,
  filter: Option<v8::Local<'a, v8::Function>>,
}

enum CpSyncRunStatus {
  Completed,
  JsException,
}

fn call_cp_sync_filter<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  filter: v8::Local<'a, v8::Function>,
  src: &str,
  dest: &str,
) -> Option<bool> {
  v8::tc_scope!(tc_scope, scope);

  let recv = v8::undefined(tc_scope);
  let src = v8::String::new(tc_scope, src).unwrap();
  let dest = v8::String::new(tc_scope, dest).unwrap();

  let result = filter.call(tc_scope, recv.into(), &[src.into(), dest.into()]);
  if tc_scope.has_caught() || tc_scope.has_terminated() {
    tc_scope.rethrow();
    return None;
  }

  Some(result.unwrap().boolean_value(tc_scope))
}

fn cp_sync_copy_dir<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
  let entries =
    fs.read_dir_sync(&src_path.as_checked_path())
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(src.to_string()),
            syscall: Some("opendir".into()),
            ..Default::default()
          },
        )
      })?;

  for entry in entries {
    let src_item = Path::new(src)
      .join(&entry.name)
      .to_string_lossy()
      .to_string();
    let dest_item = Path::new(dest)
      .join(&entry.name)
      .to_string_lossy()
      .to_string();

    if let Some(filter) = opts.filter {
      let Some(should_copy) =
        call_cp_sync_filter(scope, filter, &src_item, &dest_item)
      else {
        return Ok(CpSyncRunStatus::JsException);
      };

      if !should_copy {
        continue;
      }
    }

    let stat_info = check_paths_impl_sync(
      state,
      fs,
      &src_item,
      &dest_item,
      opts.dereference,
    )?;

    match cp_sync_dispatch(
      state, fs, scope, &stat_info, &src_item, &dest_item, opts,
    )? {
      CpSyncRunStatus::Completed => {}
      CpSyncRunStatus::JsException => return Ok(CpSyncRunStatus::JsException),
    }
  }

  Ok(CpSyncRunStatus::Completed)
}

fn cp_sync_mkdir_and_copy<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  src_mode: u32,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;
  fs.mkdir_sync(&dest_path.as_checked_path(), false, None)
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some("mkdir".into()),
          ..Default::default()
        },
      )
    })?;

  let result = cp_sync_copy_dir(state, fs, scope, src, dest, opts)?;
  if let CpSyncRunStatus::JsException = result {
    return Ok(result);
  }

  set_dest_mode(fs, &dest_path.as_checked_path(), src_mode)?;
  Ok(CpSyncRunStatus::Completed)
}

fn cp_sync_on_dir<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  stat_info: &CpCheckPathsSyncResult,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  if !stat_info.is_dest_exists {
    return cp_sync_mkdir_and_copy(
      state,
      fs,
      scope,
      stat_info.src_mode,
      src,
      dest,
      opts,
    );
  }

  cp_sync_copy_dir(state, fs, scope, src, dest, opts)
}

fn cp_sync_dispatch<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  stat_info: &CpCheckPathsSyncResult,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  if stat_info.is_src_directory && opts.recursive {
    return cp_sync_on_dir(state, fs, scope, stat_info, src, dest, opts);
  } else if stat_info.is_src_directory {
    return Err(
      CpError::EIsDir {
        message: format!("{} is a directory (not copied)", src),
        path: src.to_string(),
      }
      .into(),
    );
  } else if stat_info.is_src_file
    || stat_info.is_src_char_device
    || stat_info.is_src_block_device
  {
    op_node_cp_on_file_sync(state, fs, src, dest, stat_info, &opts)?;
    return Ok(CpSyncRunStatus::Completed);
  } else if stat_info.is_src_symlink {
    op_node_cp_on_link_sync(
      state,
      fs,
      src,
      dest,
      stat_info.is_dest_exists,
      opts.verbatim_symlinks,
    )?;
    return Ok(CpSyncRunStatus::Completed);
  } else if stat_info.is_src_socket {
    return Err(
      CpError::Socket {
        message: format!("cannot copy a socket file: {}", dest),
        path: dest.to_string(),
      }
      .into(),
    );
  } else if stat_info.is_src_fifo {
    return Err(
      CpError::Fifo {
        message: format!("cannot copy a FIFO pipe: {}", dest),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Err(
    CpError::Unknown {
      message: format!("cannot copy an unknown file type: {}", dest),
      path: dest.to_string(),
    }
    .into(),
  )
}

/// Check if two stat results refer to the same file (same dev + ino).
fn are_identical_stat(
  src_stat: &deno_io::fs::FsStat,
  dest_stat: &deno_io::fs::FsStat,
) -> bool {
  match (dest_stat.ino, src_stat.ino) {
    (Some(dest_ino), Some(src_ino)) if dest_ino > 0 && dest_stat.dev > 0 => {
      dest_ino == src_ino && dest_stat.dev == src_stat.dev
    }
    _ => false,
  }
}

fn is_src_subdir(src: &str, dest: &str) -> bool {
  let src_resolved =
    std::path::absolute(src).unwrap_or_else(|_| PathBuf::from(src));
  let dest_resolved =
    std::path::absolute(dest).unwrap_or_else(|_| PathBuf::from(dest));

  let src_components = src_resolved.components().collect::<Vec<_>>();
  let dest_components = dest_resolved.components().collect::<Vec<_>>();

  src_components
    .iter()
    .enumerate()
    .all(|(i, c)| dest_components.get(i) == Some(c))
}

/// As this function attempts to get the absolute path of the target,
/// callers of this function should ensure that the permissions are granted
/// with at minimum using `.check_read_all()`.
fn cp_symlink_type(
  fs: &FileSystemRc,
  target: &CheckedPathBuf,
  link_path: &CheckedPathBuf,
) -> Option<FsFileType> {
  #[cfg(windows)]
  {
    // Mirror node polyfill behavior: resolve target relative to the link
    // path, infer dir/file from stat, and default to file on any error.
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/lib/fs.js#L1806-L1837
    if let Ok(absolute_target) = std::path::absolute(
      Path::new(link_path.as_os_str())
        .join("..")
        .join(target.as_os_str()),
    ) {
      let checked_target = CheckedPathBuf::unsafe_new(absolute_target);
      let checked_target = checked_target.as_checked_path();
      if let Ok(stat) = fs.stat_sync(&checked_target)
        && stat.is_directory
      {
        return Some(FsFileType::Directory);
      }
    }

    Some(FsFileType::File)
  }

  #[cfg(not(windows))]
  {
    let _ = (fs, target, link_path);
    None
  }
}

#[allow(clippy::unused_async, reason = "sometimes it's async")]
async fn cp_create_symlink(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  target: String,
  link_path: String,
) -> Result<(), FsError> {
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_write_all("node:fs.symlink")?;
    permissions.check_read_all("node:fs.symlink")?;
  }
  let fs = fs.clone();
  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    // PERMISSIONS: ok because we verified --allow-write and --allow-read above
    let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&target));
    let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(&link_path));
    let file_type = cp_symlink_type(&fs, &oldpath, &newpath);
    let oldpath = oldpath.as_checked_path();
    let newpath = newpath.as_checked_path();

    fs.symlink_sync(&oldpath, &newpath, file_type)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(target),
            dest: Some(link_path),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        )
      })?;
    Ok(())
  })
}

fn check_cp_path(
  state: &Rc<RefCell<OpState>>,
  path: &str,
  access_kind: OpenAccessKind,
) -> Result<CheckedPathBuf, FsError> {
  let mut state = state.borrow_mut();
  Ok(
    state
      .borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(path)),
        access_kind,
        Some("node:fs.cp"),
      )?
      .into_owned(),
  )
}

fn check_paths_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsSyncResult, FsError> {
  let (src_stat_result, dest_result, syscall) = if dereference {
    let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
    let dest_path = check_cp_path(state, dest, OpenAccessKind::Read)?;
    (
      fs.stat_sync(&src_path.as_checked_path()),
      fs.stat_sync(&dest_path.as_checked_path()),
      "stat".to_string(),
    )
  } else {
    let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
    let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
    (
      fs.lstat_sync(&src_path.as_checked_path()),
      fs.lstat_sync(&dest_path.as_checked_path()),
      "lstat".to_string(),
    )
  };

  let src_stat = src_stat_result.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some(syscall.to_string()),
        ..Default::default()
      },
    )
  })?;

  let dest_stat = match dest_result {
    Ok(stat) => Some(stat),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => {
      return Err(map_fs_error_to_node_fs_error(
        e,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some(syscall),
          ..Default::default()
        },
      ));
    }
  };

  let src_dev = src_stat.dev;
  let src_ino = src_stat.ino.unwrap_or(0);

  if let Some(ref dest_stat) = dest_stat {
    if are_identical_stat(&src_stat, dest_stat) {
      return Err(
        CpError::EInval {
          message: "src and dest cannot be the same".to_string(),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if src_stat.is_directory && !dest_stat.is_directory {
      return Err(
        CpError::DirToNonDir {
          message: format!(
            "cannot overwrite non-directory {} with directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if !src_stat.is_directory && dest_stat.is_directory {
      return Err(
        CpError::NonDirToDir {
          message: format!(
            "cannot overwrite directory {} with non-directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
  }

  if src_stat.is_directory && is_src_subdir(src, dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          src, dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Ok(CpCheckPathsSyncResult {
    src_dev,
    src_ino,
    src_mode: src_stat.mode,
    is_dest_exists: dest_stat.is_some(),
    is_src_directory: src_stat.is_directory,
    is_src_file: src_stat.is_file,
    is_src_char_device: src_stat.is_char_device,
    is_src_block_device: src_stat.is_block_device,
    is_src_symlink: src_stat.is_symlink,
    is_src_socket: src_stat.is_socket,
    is_src_fifo: src_stat.is_fifo,
  })
}

/// Validates src and dest paths for a cp operation.
/// Checks identity, directory type conflicts, and subdirectory relationships.
#[allow(clippy::unused_async, reason = "sometimes async depending on cfg")]
async fn check_paths_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsResult, FsError> {
  let open_access_kind = if dereference {
    OpenAccessKind::Read
  } else {
    OpenAccessKind::ReadNoFollow
  };

  let src_path = check_cp_path(state, src, open_access_kind)?;
  let dest_path = check_cp_path(state, dest, open_access_kind)?;

  let fs = fs.clone();
  let (src_stat_result, dest_result, syscall) =
    maybe_spawn_blocking!(move || -> (FsResult<_>, FsResult<_>, String) {
      let src_path = src_path.as_checked_path();
      let dest_path = dest_path.as_checked_path();
      if dereference {
        (
          fs.stat_sync(&src_path),
          fs.stat_sync(&dest_path),
          "stat".to_string(),
        )
      } else {
        (
          fs.lstat_sync(&src_path),
          fs.lstat_sync(&dest_path),
          "lstat".to_string(),
        )
      }
    });

  let src_stat = src_stat_result.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some(syscall.clone()),
        ..Default::default()
      },
    )
  })?;

  let dest_stat = match dest_result {
    Ok(stat) => Some(stat),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => {
      return Err(map_fs_error_to_node_fs_error(
        e,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some(syscall),
          ..Default::default()
        },
      ));
    }
  };

  let src_dev = src_stat.dev;
  let src_ino = src_stat.ino.unwrap_or(0);

  if let Some(ref dest_stat) = dest_stat {
    if are_identical_stat(&src_stat, dest_stat) {
      return Err(
        CpError::EInval {
          message: "src and dest cannot be the same".to_string(),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if src_stat.is_directory && !dest_stat.is_directory {
      return Err(
        CpError::DirToNonDir {
          message: format!(
            "cannot overwrite non-directory {} with directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if !src_stat.is_directory && dest_stat.is_directory {
      return Err(
        CpError::NonDirToDir {
          message: format!(
            "cannot overwrite directory {} with non-directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
  }

  if src_stat.is_directory && is_src_subdir(src, dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          src, dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Ok(CpCheckPathsResult {
    src_dev,
    src_ino,
    stat_info: compact_stat_info(&src_stat, dest_stat.is_some()),
  })
}

/// Validates src and dest paths for recursive cp operations.
/// Returns stat info for the source file
#[op2(stack_trace)]
#[bigint]
pub async fn op_node_cp_check_paths_recursive(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<u64, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let result = check_paths_impl(&state, &fs, &src, &dest, dereference).await?;

  Ok(result.stat_info)
}

/// Validates src and dest paths, checks parent paths, and ensures
/// parent directory exists
/// Returns stat info for the source file
#[op2(stack_trace)]
#[bigint]
pub async fn op_node_cp_validate_and_prepare(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<u64, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let check_result =
    check_paths_impl(&state, &fs, &src, &dest, dereference).await?;

  check_parent_paths_impl(
    &state,
    &fs,
    &src,
    check_result.src_dev,
    check_result.src_ino,
    &dest,
  )
  .await?;

  ensure_parent_dir_impl(&state, &fs, &dest).await?;

  Ok(check_result.stat_info)
}

/// Validates src and dest paths, checks parent paths, and ensures
/// parent directory exists for cpSync.
/// Returns stat info for the source file.
fn op_node_cp_validate_and_prepare_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsSyncResult, FsError> {
  let check_result = check_paths_impl_sync(state, fs, src, dest, dereference)?;

  check_parent_paths_impl_sync(
    state,
    fs,
    src,
    check_result.src_dev,
    check_result.src_ino,
    dest,
  )?;

  ensure_parent_dir_impl_sync(state, fs, dest)?;

  Ok(check_result)
}

#[op2(fast, reentrant, stack_trace)]
pub fn op_node_cp_sync<'a>(
  state: Rc<RefCell<OpState>>,
  scope: &mut v8::PinScope<'a, '_>,
  #[string] src: &str,
  #[string] dest: &str,
  dereference: bool,
  recursive: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  verbatim_symlinks: bool,
  #[smi] mode: u32,
  filter: v8::Local<'a, v8::Value>,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let stat_info =
    op_node_cp_validate_and_prepare_sync(&state, &fs, src, dest, dereference)?;

  let filter = v8::Local::<v8::Function>::try_from(filter).ok();

  let opts = CpSyncOptions {
    dereference,
    recursive,
    force,
    error_on_exist,
    preserve_timestamps,
    verbatim_symlinks,
    mode,
    filter,
  };

  match cp_sync_dispatch(&state, &fs, scope, &stat_info, src, dest, opts)? {
    // Treat JsException as success here so the pending V8 exception can propagate
    // naturally, preserving the original JS stack instead of rethrowing from Rust.
    CpSyncRunStatus::Completed | CpSyncRunStatus::JsException => Ok(()),
  }
}

/// Recursively check if dest parent is a subdirectory of src.
/// It works for all file types including symlinks since it
/// checks the src and dest inodes. It starts from the deepest
/// parent and stops once it reaches the src parent or the root path.
async fn check_parent_paths_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  src_dev: u64,
  src_ino: u64,
  dest: &str,
) -> Result<(), FsError> {
  let src_parent = Path::new(src)
    .parent()
    .map(Cow::Borrowed)
    .unwrap_or_default();
  let src_parent = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&src_parent)))
      .unwrap_or_else(|_| {
        fs.cwd()
          .map(|cwd| cwd.join(&src_parent))
          .unwrap_or_else(|_| src_parent.into_owned())
      }),
  );

  let mut current = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  current = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&current)))
      .unwrap_or_else(|_| std::path::absolute(&current).unwrap_or(current)),
  );

  loop {
    if current == src_parent {
      return Ok(());
    }

    // Check if current is the root
    if current.parent().is_none() || current.parent() == Some(&current) {
      return Ok(());
    }

    let current_str = current.to_string_lossy();
    let checked_path =
      match check_cp_path(state, &current_str, OpenAccessKind::Read) {
        Ok(p) => p,
        // When read permission is ignored, the check returns NotFound.
        // Treat it like a non-existent directory: stop walking.
        Err(FsError::Permission(e))
          if e.kind() == std::io::ErrorKind::NotFound =>
        {
          return Ok(());
        }
        Err(e) => return Err(e),
      };
    let stat_result = fs.stat_async(checked_path).await;
    match stat_result {
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
      Err(e) => {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(current_str.to_string()),
            syscall: Some("stat".to_string()),
            ..Default::default()
          },
        ));
      }
      Ok(dest_stat) => {
        // Check if src and current parent are identical (same dev + ino)
        if let Some(dest_ino) = dest_stat.ino
          && dest_ino == src_ino
          && dest_stat.dev == src_dev
        {
          return Err(
            CpError::EInval {
              message: format!(
                "cannot copy {} to a subdirectory of self {}",
                src, dest
              ),
              path: dest.to_string(),
            }
            .into(),
          );
        }
      }
    }

    current = match current.parent() {
      Some(p) => p.to_path_buf(),
      None => return Ok(()),
    };
  }
}

fn check_parent_paths_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  src_dev: u64,
  src_ino: u64,
  dest: &str,
) -> Result<(), FsError> {
  let src_parent = Path::new(src)
    .parent()
    .map(Cow::Borrowed)
    .unwrap_or_default();
  let src_parent = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&src_parent)))
      .unwrap_or_else(|_| {
        fs.cwd()
          .map(|cwd| cwd.join(&src_parent))
          .unwrap_or_else(|_| src_parent.into_owned())
      }),
  );

  let mut current = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  current = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&current)))
      .unwrap_or_else(|_| std::path::absolute(&current).unwrap_or(current)),
  );

  loop {
    if current == src_parent {
      return Ok(());
    }

    // Check if current is the root
    if current.parent().is_none() || current.parent() == Some(&current) {
      return Ok(());
    }

    let current_str = current.to_string_lossy();
    let checked_path =
      match check_cp_path(state, &current_str, OpenAccessKind::Read) {
        Ok(p) => p,
        // When read permission is ignored, the check returns NotFound.
        // Treat it like a non-existent directory: stop walking.
        Err(FsError::Permission(e))
          if e.kind() == std::io::ErrorKind::NotFound =>
        {
          return Ok(());
        }
        Err(e) => return Err(e),
      };
    match fs.stat_sync(&checked_path.as_checked_path()) {
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
      Err(e) => {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(current_str.to_string()),
            syscall: Some("stat".to_string()),
            ..Default::default()
          },
        ));
      }
      Ok(dest_stat) => {
        // Check if src and current parent are identical (same dev + ino)
        if let Some(dest_ino) = dest_stat.ino
          && dest_ino == src_ino
          && dest_stat.dev == src_dev
        {
          return Err(
            CpError::EInval {
              message: format!(
                "cannot copy {} to a subdirectory of self {}",
                src, dest
              ),
              path: dest.to_string(),
            }
            .into(),
          );
        }
      }
    }

    current = match current.parent() {
      Some(p) => p.to_path_buf(),
      None => return Ok(()),
    };
  }
}

/// Ensures the parent directory of `dest` exists, creating it recursively
/// if needed.
async fn ensure_parent_dir_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  dest: &str,
) -> Result<(), FsError> {
  let dest_parent = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();

  let parent_str = dest_parent.to_string_lossy();
  let checked_parent = check_cp_path(state, &parent_str, OpenAccessKind::Read)?;
  let exists = fs.exists_async(checked_parent).await.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(parent_str.to_string()),
        syscall: Some("exists".into()),
        ..Default::default()
      },
    )
  })?;
  if !exists {
    let checked_parent =
      check_cp_path(state, &parent_str, OpenAccessKind::Write)?;
    fs.mkdir_async(checked_parent, true, None)
      .await
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(parent_str.to_string()),
            syscall: Some("mkdir".into()),
            ..Default::default()
          },
        )
      })?;
  }
  Ok(())
}

fn ensure_parent_dir_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  dest: &str,
) -> Result<(), FsError> {
  let dest_parent = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();

  let parent_str = dest_parent.to_string_lossy();
  let checked_parent = check_cp_path(state, &parent_str, OpenAccessKind::Read)?;
  let exists = fs.exists_sync(&checked_parent.as_checked_path());

  if !exists {
    let checked_parent =
      check_cp_path(state, &parent_str, OpenAccessKind::Write)?;
    fs.mkdir_sync(&checked_parent.as_checked_path(), true, None)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(parent_str.to_string()),
            syscall: Some("mkdir".into()),
            ..Default::default()
          },
        )
      })?;
  }

  Ok(())
}

fn handle_timestamps_and_mode(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
  src_mode: u32,
) -> Result<(), FsError> {
  // Make sure the file is writable before setting the timestamp
  // otherwise open fails with EPERM when invoked with 'r+' (through utimes call)
  if file_is_not_writable(src_mode) {
    let mode = src_mode | 0o200;
    set_dest_mode(fs, dest_path, mode)?;
  }

  // Set timestamps from a fresh stat of src (atime is modified by read).
  set_dest_timestamps_and_mode(fs, src_path, dest_path, src_mode)?;
  Ok(())
}

fn file_is_not_writable(mode: u32) -> bool {
  (mode & 0o200) == 0
}

fn cp_mode_has_copyfile_excl(mode: u32) -> bool {
  let copyfile_excl = UV_FS_COPYFILE_EXCL as u32;
  (mode & copyfile_excl) == copyfile_excl
}

fn set_dest_mode(
  fs: &FileSystemRc,
  dest_path: &CheckedPath,
  mode: u32,
) -> Result<(), FsError> {
  if mode == 0 {
    return Ok(());
  }

  fs.chmod_sync(dest_path, mode as _).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(dest_path.to_string_lossy().to_string()),
        syscall: Some("chmod".into()),
        ..Default::default()
      },
    )
  })?;
  Ok(())
}

fn set_dest_timestamps_and_mode(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
  src_mode: u32,
) -> Result<(), FsError> {
  // Re-stat src to get fresh atime/mtime
  let src_stat = fs.stat_sync(src_path).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src_path.to_string_lossy().to_string()),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  if let (Some(atime), Some(mtime)) = (src_stat.atime, src_stat.mtime) {
    // FsStat stores times as milliseconds since the Unix epoch.
    // utime_async expects split values: whole seconds + nanoseconds remainder.
    let atime_secs = (atime / 1000) as i64;
    // Remaining milliseconds are converted to nanoseconds.
    let atime_nanos = ((atime % 1000) * 1_000_000) as u32;
    let mtime_secs = (mtime / 1000) as i64;
    // Same conversion for mtime: ms remainder -> ns.
    let mtime_nanos = ((mtime % 1000) * 1_000_000) as u32;
    fs.utime_sync(dest_path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest_path.to_string_lossy().to_string()),
            syscall: Some("utime".into()),
            ..Default::default()
          },
        )
      })?;
  }

  set_dest_mode(fs, dest_path, src_mode)?;
  Ok(())
}

/// Handle copying a single file.
///
/// If dest does not exist, copies src to dest directly.
/// If dest exists and force is true, removes dest and copies.
/// If dest exists and error_on_exist is true, returns an EExist error.
/// Otherwise does nothing.
///
/// When preserve_timestamps is true, copies the timestamps from src to dest
/// and ensures the file is writable before doing so.
#[op2(stack_trace)]
pub async fn op_node_cp_on_file(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  #[smi] src_mode: u32,
  dest_exists: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  #[smi] mode: u32,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  if dest_exists {
    if force {
      // Remove dest, then copy
      let dest_path = check_cp_path(&state, &dest, OpenAccessKind::Write)?;
      fs.remove_async(dest_path, false).await.map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.clone()),
            syscall: Some("unlink".into()),
            ..Default::default()
          },
        )
      })?;
    } else if error_on_exist {
      return Err(
        CpError::EExist {
          message: format!("{} already exists", dest),
          path: dest.to_string(),
        }
        .into(),
      );
    } else {
      // Neither force nor errorOnExist: do nothing
      return Ok(());
    }
  }

  // Copy file: read from src, write to dest
  if cp_mode_has_copyfile_excl(mode) {
    let dest_path = check_cp_path(&state, &dest, OpenAccessKind::ReadNoFollow)?;
    match fs.lstat_async(dest_path.clone()).await {
      Ok(_) => {
        return Err(
          CpError::EExist {
            message: format!("{} already exists", dest),
            path: dest.to_string(),
          }
          .into(),
        );
      }
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => {
        return Err(map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.to_string()),
            syscall: Some("lstat".into()),
            ..Default::default()
          },
        ));
      }
    }
  }

  let src_path = check_cp_path(&state, &src, OpenAccessKind::Read)?;
  let dest_path = check_cp_path(&state, &dest, OpenAccessKind::Write)?;

  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    fs.copy_file_sync(
      &src_path.as_checked_path(),
      &dest_path.as_checked_path(),
    )
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src_path.to_string_lossy().to_string()),
          dest: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("copyfile".into()),
          ..Default::default()
        },
      )
    })?;
    if preserve_timestamps {
      handle_timestamps_and_mode(
        &fs,
        &src_path.as_checked_path(),
        &dest_path.as_checked_path(),
        src_mode,
      )?;
    } else {
      set_dest_mode(&fs, &dest_path.as_checked_path(), src_mode)?;
    }
    Ok(())
  })
}

fn op_node_cp_on_file_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  stat_info: &CpCheckPathsSyncResult,
  opts: &CpSyncOptions,
) -> Result<(), FsError> {
  if stat_info.is_dest_exists {
    if opts.force {
      // Remove dest, then copy.
      let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;
      fs.remove_sync(&dest_path.as_checked_path(), false)
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(dest.to_string()),
              syscall: Some("unlink".into()),
              ..Default::default()
            },
          )
        })?;
    } else if opts.error_on_exist {
      return Err(
        CpError::EExist {
          message: format!("{} already exists", dest),
          path: dest.to_string(),
        }
        .into(),
      );
    } else {
      // Neither force nor errorOnExist: do nothing.
      return Ok(());
    }
  }

  if cp_mode_has_copyfile_excl(opts.mode) {
    let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
    match fs.lstat_sync(&dest_path.as_checked_path()) {
      Ok(_) => {
        return Err(
          CpError::EExist {
            message: format!("{} already exists", dest),
            path: dest.to_string(),
          }
          .into(),
        );
      }
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => {
        return Err(map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.to_string()),
            syscall: Some("lstat".into()),
            ..Default::default()
          },
        ));
      }
    }
  }

  let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
  let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;

  fs.copy_file_sync(&src_path.as_checked_path(), &dest_path.as_checked_path())
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.to_string()),
          dest: Some(dest.to_string()),
          syscall: Some("copyfile".into()),
          ..Default::default()
        },
      )
    })?;

  if opts.preserve_timestamps {
    handle_timestamps_and_mode(
      fs,
      &src_path.as_checked_path(),
      &dest_path.as_checked_path(),
      stat_info.src_mode,
    )?;
  } else {
    set_dest_mode(fs, &dest_path.as_checked_path(), stat_info.src_mode)?;
  }

  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_cp_on_link(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dest_exists: bool,
  verbatim_symlinks: bool,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let src_path = check_cp_path(&state, &src, OpenAccessKind::ReadNoFollow)?;
  let resolved_src_buf =
    fs.read_link_async(src_path.clone()).await.map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.clone()),
          syscall: Some("readlink".into()),
          ..Default::default()
        },
      )
    })?;
  let mut resolved_src = resolved_src_buf.to_string_lossy().to_string();

  // Resolve relative symlink targets
  if !verbatim_symlinks
    && !Path::new(&resolved_src).is_absolute()
    && let Some(parent) = Path::new(&src).parent()
  {
    resolved_src = parent.join(&resolved_src).to_string_lossy().to_string();
  }

  if !dest_exists {
    cp_create_symlink(&state, &fs, resolved_src.to_string(), dest).await?;
    return Ok(());
  }

  // Dest exists — try to read it as a symlink
  let dest_path = check_cp_path(&state, &dest, OpenAccessKind::ReadNoFollow)?;
  let resolved_dest_result = fs.read_link_async(dest_path).await;
  let resolved_dest = match resolved_dest_result {
    Ok(p) => {
      let s = p.to_string_lossy().to_string();
      // If relative, resolve against dirname(dest)
      if !Path::new(&s).is_absolute() {
        if let Some(parent) = Path::new(&dest).parent() {
          parent.join(&s).to_string_lossy().to_string()
        } else {
          s
        }
      } else {
        s
      }
    }
    Err(e) => {
      let kind = e.kind();
      // EINVAL or UNKNOWN means dest is a regular file/directory, not a symlink
      if kind == std::io::ErrorKind::InvalidInput
        || kind == std::io::ErrorKind::Other
      {
        cp_create_symlink(
          &state,
          &fs,
          resolved_src.to_string(),
          dest.to_string(),
        )
        .await?;
        return Ok(());
      }

      #[cfg(windows)]
      {
        use winapi::shared::winerror::ERROR_NOT_A_REPARSE_POINT;

        let os_error = e.into_io_error();
        let Some(errno) = os_error.raw_os_error() else {
          return Err(FsError::Io(os_error));
        };

        let errno = errno as u32;
        if errno != ERROR_NOT_A_REPARSE_POINT {
          return Err(
            NodeFsError {
              os_errno: errno as _,
              context: NodeFsErrorContext {
                path: Some(resolved_src),
                dest: Some(dest),
                syscall: Some("symlink".into()),
                ..Default::default()
              },
            }
            .into(),
          );
        }

        return cp_create_symlink(
          &state,
          &fs,
          resolved_src.to_string(),
          dest.to_string(),
        )
        .await;
      }
      #[cfg(not(windows))]
      {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(resolved_src),
            dest: Some(dest),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        ));
      }
    }
  };

  // Check subdirectory relationships
  let src_path = check_cp_path(&state, &src, OpenAccessKind::Read)?;
  let src_stat = fs.stat_async(src_path).await.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  let src_is_dir = src_stat.is_directory;
  if src_is_dir && is_src_subdir(&resolved_src, &resolved_dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          resolved_src, resolved_dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  // Do not copy if src is a subdir of dest since unlinking
  // dest would remove src contents and create a broken symlink.
  if src_is_dir && is_src_subdir(&resolved_dest, &resolved_src) {
    return Err(
      CpError::SymlinkToSubdirectory {
        message: format!(
          "cannot overwrite {} with {}",
          resolved_dest, resolved_src
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_write_all("node:fs.cp")?;
    permissions.check_read_all("node:fs.cp")?;
  }

  // Unlink dest and create new symlink
  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    let src_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
    let dest_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&dest));
    let src_path = src_path_buf.as_checked_path();
    let dest_path = dest_path_buf.as_checked_path();

    fs.remove_sync(&dest_path, false).map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("unlink".into()),
          ..Default::default()
        },
      )
    })?;

    let file_type = cp_symlink_type(&fs, &src_path_buf, &dest_path_buf);
    fs.symlink_sync(&src_path, &dest_path, file_type)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(src_path.to_string_lossy().to_string()),
            dest: Some(dest_path.to_string_lossy().to_string()),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        )
      })
  })
}

fn op_node_cp_on_link_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dest_exists: bool,
  verbatim_symlinks: bool,
) -> Result<(), FsError> {
  let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
  let resolved_src_buf = fs
    .read_link_sync(&src_path.as_checked_path())
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.to_string()),
          syscall: Some("readlink".into()),
          ..Default::default()
        },
      )
    })?;
  let mut resolved_src = resolved_src_buf.to_string_lossy().to_string();

  // Resolve relative symlink targets.
  if !verbatim_symlinks
    && !Path::new(&resolved_src).is_absolute()
    && let Some(parent) = Path::new(src).parent()
  {
    resolved_src = parent.join(&resolved_src).to_string_lossy().to_string();
  }

  if !dest_exists {
    {
      let mut state = state.borrow_mut();
      state
        .borrow_mut::<PermissionsContainer>()
        .check_write_all("node:fs.symlink")?;
      state
        .borrow_mut::<PermissionsContainer>()
        .check_read_all("node:fs.symlink")?;
    }

    let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
    let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
    let file_type = cp_symlink_type(fs, &oldpath, &newpath);
    fs.symlink_sync(
      &oldpath.as_checked_path(),
      &newpath.as_checked_path(),
      file_type,
    )
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(resolved_src),
          dest: Some(dest.to_string()),
          syscall: Some("symlink".into()),
          ..Default::default()
        },
      )
    })?;
    return Ok(());
  }

  // Dest exists — try to read it as a symlink.
  let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
  let resolved_dest_result = fs.read_link_sync(&dest_path.as_checked_path());
  let resolved_dest = match resolved_dest_result {
    Ok(p) => {
      let s = p.to_string_lossy().to_string();
      // If relative, resolve against dirname(dest).
      if !Path::new(&s).is_absolute() {
        if let Some(parent) = Path::new(dest).parent() {
          parent.join(&s).to_string_lossy().to_string()
        } else {
          s
        }
      } else {
        s
      }
    }
    Err(e) => {
      let kind = e.kind();
      // EINVAL or UNKNOWN means dest is a regular file/directory, not a symlink.
      if kind == std::io::ErrorKind::InvalidInput
        || kind == std::io::ErrorKind::Other
      {
        {
          let mut state = state.borrow_mut();
          state
            .borrow_mut::<PermissionsContainer>()
            .check_write_all("node:fs.symlink")?;
          state
            .borrow_mut::<PermissionsContainer>()
            .check_read_all("node:fs.symlink")?;
        }

        let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
        let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
        let file_type = cp_symlink_type(fs, &oldpath, &newpath);
        fs.symlink_sync(
          &oldpath.as_checked_path(),
          &newpath.as_checked_path(),
          file_type,
        )
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(resolved_src.clone()),
              dest: Some(dest.to_string()),
              syscall: Some("symlink".into()),
              ..Default::default()
            },
          )
        })?;
        return Ok(());
      }

      #[cfg(windows)]
      {
        use winapi::shared::winerror::ERROR_NOT_A_REPARSE_POINT;

        let os_error = e.into_io_error();
        let Some(errno) = os_error.raw_os_error() else {
          return Err(FsError::Io(os_error));
        };

        let errno = errno as u32;
        if errno != ERROR_NOT_A_REPARSE_POINT {
          return Err(
            NodeFsError {
              os_errno: errno as _,
              context: NodeFsErrorContext {
                path: Some(resolved_src),
                dest: Some(dest.to_string()),
                syscall: Some("symlink".into()),
                ..Default::default()
              },
            }
            .into(),
          );
        }

        {
          let mut state = state.borrow_mut();
          state
            .borrow_mut::<PermissionsContainer>()
            .check_write_all("node:fs.symlink")?;
          state
            .borrow_mut::<PermissionsContainer>()
            .check_read_all("node:fs.symlink")?;
        }

        let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
        let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
        let file_type = cp_symlink_type(fs, &oldpath, &newpath);
        fs.symlink_sync(
          &oldpath.as_checked_path(),
          &newpath.as_checked_path(),
          file_type,
        )
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(resolved_src.clone()),
              dest: Some(dest.to_string()),
              syscall: Some("symlink".into()),
              ..Default::default()
            },
          )
        })?;
        return Ok(());
      }
      #[cfg(not(windows))]
      {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(resolved_src),
            dest: Some(dest.to_string()),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        ));
      }
    }
  };

  // Check subdirectory relationships.
  let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
  let src_stat = fs.stat_sync(&src_path.as_checked_path()).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  let src_is_dir = src_stat.is_directory;
  if src_is_dir && is_src_subdir(&resolved_src, &resolved_dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          resolved_src, resolved_dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  // Do not copy if src is a subdir of dest since unlinking
  // dest would remove src contents and create a broken symlink.
  if src_is_dir && is_src_subdir(&resolved_dest, &resolved_src) {
    return Err(
      CpError::SymlinkToSubdirectory {
        message: format!(
          "cannot overwrite {} with {}",
          resolved_dest, resolved_src
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  let dest_path = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(dest)),
      OpenAccessKind::Write,
      Some("node:fs.rm"),
    )?
  };

  fs.remove_sync(&dest_path, false).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(dest_path.to_string_lossy().to_string()),
        syscall: Some("unlink".into()),
        ..Default::default()
      },
    )
  })?;

  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<PermissionsContainer>()
      .check_write_all("node:fs.symlink")?;
    state
      .borrow_mut::<PermissionsContainer>()
      .check_read_all("node:fs.symlink")?;
  }

  let src_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
  let dest_path_buf = CheckedPathBuf::unsafe_new(dest_path.to_path_buf());
  let src_path = src_path_buf.as_checked_path();

  let file_type = cp_symlink_type(fs, &src_path_buf, &dest_path_buf);
  fs.symlink_sync(&src_path, &dest_path, file_type)
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src_path.to_string_lossy().to_string()),
          dest: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("symlink".into()),
          ..Default::default()
        },
      )
    })
}

#[cfg(test)]
mod tests {
  use super::is_src_subdir;

  #[test]
  fn test_is_src_subdir() {
    let base = std::env::temp_dir().join("deno_is_src_subdir_test");
    let src = base.join("src");
    let child = src.join("child");
    let sibling = base.join("sibling");

    let src = src.to_string_lossy().into_owned();
    let child = child.to_string_lossy().into_owned();
    let sibling = sibling.to_string_lossy().into_owned();

    assert!(is_src_subdir(&src, &child));
    assert!(is_src_subdir(&src, &src));
    assert!(!is_src_subdir(&src, &sibling));
    assert!(!is_src_subdir(&child, &src));
  }
}
