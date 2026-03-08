// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_fs::FileSystemRc;
use deno_fs::OpenOptions;
use deno_io::fs::FileResource;
use deno_io::fs::FsResult;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use serde::Serialize;
use tokio::task::JoinError;

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
  CpError(
    #[from]
    #[inherit]
    CpErrorKind,
  ),
  #[class(inherit)]
  #[error(transparent)]
  UVCompat(#[from] NodeFsError),
}

impl From<JoinError> for FsError {
  fn from(err: JoinError) -> Self {
    if err.is_cancelled() {
      todo!("async tasks must not be cancelled")
    }
    if err.is_panic() {
      std::panic::resume_unwind(err.into_panic()); // resume the panic on the main thread
    }
    unreachable!()
  }
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
pub enum CpErrorKind {
  EInval { message: String, path: String },
  DirToNonDir { message: String, path: String },
  NonDirToDir { message: String, path: String },
  EExist { message: String, path: String },
  SymlinkToSubdirectory { message: String, path: String },
}

impl CpErrorKind {
  fn kind(&self) -> &'static str {
    match self {
      CpErrorKind::EInval { .. } => "EINVAL",
      CpErrorKind::DirToNonDir { .. } => "DIR_TO_NON_DIR",
      CpErrorKind::NonDirToDir { .. } => "NON_DIR_TO_DIR",
      CpErrorKind::EExist { .. } => "EEXIST",
      CpErrorKind::SymlinkToSubdirectory { .. } => "SYMLINK_TO_SUBDIRECTORY",
    }
  }

  fn message(&self) -> String {
    match self {
      CpErrorKind::EInval { message, .. } => message.clone(),
      CpErrorKind::DirToNonDir { message, .. } => message.clone(),
      CpErrorKind::NonDirToDir { message, .. } => message.clone(),
      CpErrorKind::EExist { message, .. } => message.clone(),
      CpErrorKind::SymlinkToSubdirectory { message, .. } => message.clone(),
    }
  }

  fn path(&self) -> String {
    match self {
      CpErrorKind::EInval { path, .. } => path.clone(),
      CpErrorKind::DirToNonDir { path, .. } => path.clone(),
      CpErrorKind::NonDirToDir { path, .. } => path.clone(),
      CpErrorKind::EExist { path, .. } => path.clone(),
      CpErrorKind::SymlinkToSubdirectory { path, .. } => path.clone(),
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

  match spawn_blocking(move || statfs(path, bigint)).await {
    Ok(result) => result,
    Err(err) => Err(FsError::Io(err.into())),
  }
}

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

#[derive(Debug, Serialize)]
struct CpCheckPathsResult {
  src_dev: u64,
  src_ino: u64,
  dest_exists: bool,
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

/// Validates src and dest paths for a cp operation.
/// Checks identity, directory type conflicts, and subdirectory relationships.
async fn check_paths_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsResult, FsError> {
  let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
  let dest_path = check_cp_path(state, dest, OpenAccessKind::Read)?;

  let fs = fs.clone();
  let (src_stat_result, dest_result, syscall) =
    spawn_blocking(move || -> (FsResult<_>, FsResult<_>, String) {
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
    })
    .await?;

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
        CpErrorKind::EInval {
          message: "src and dest cannot be the same".to_string(),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if src_stat.is_directory && !dest_stat.is_directory {
      return Err(
        CpErrorKind::DirToNonDir {
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
        CpErrorKind::NonDirToDir {
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
      CpErrorKind::EInval {
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
    dest_exists: dest_stat.is_some(),
  })
}

/// Async op: validates src and dest paths for a cp operation.
/// Returns a result with src stat info, dest existence, and optional error.
#[op2(stack_trace)]
#[serde]
pub async fn op_node_cp_check_paths(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<CpCheckPathsResult, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  check_paths_impl(&state, &fs, &src, &dest, dereference).await
}

/// Async op: validates src and dest paths for recursive cp operations.
/// Returns a result with src stat info, dest existence, and optional error.
#[op2(stack_trace)]
pub async fn op_node_cp_check_paths_recursive(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<bool, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let result = check_paths_impl(&state, &fs, &src, &dest, dereference).await?;

  Ok(result.dest_exists)
}

/// Async op: validates src and dest paths, checks parent paths, and ensures
/// parent directory exists - all in a single operation for better performance.
/// Returns a result with src stat info, dest existence, and optional error.
#[op2(stack_trace)]
pub async fn op_node_cp_validate_and_prepare(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<bool, FsError> {
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

  Ok(check_result.dest_exists)
}

/// Recursively check if dest parent is a subdirectory of src.
/// It works for all file types including symlinks since it
/// checks the src and dest inodes. It starts from the deepest
/// parent and stops once it reaches the src parent or the root path.
#[allow(clippy::disallowed_methods)] // allow, implementation
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
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  let src_parent =
    deno_path_util::strip_unc_prefix(src_parent.canonicalize().unwrap_or_else(
      |_| std::path::absolute(&src_parent).unwrap_or(src_parent),
    ));

  let mut current = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  current = deno_path_util::strip_unc_prefix(
    current
      .canonicalize()
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

    let current_str = current.to_str().unwrap_or_default();
    let checked_path = check_cp_path(state, current_str, OpenAccessKind::Read)?;
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
            CpErrorKind::EInval {
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

/// Async op: checks that dest is not a subdirectory of src.
/// Returns null if OK, or a CpErrorKind object if the check fails.
#[op2(stack_trace)]
pub async fn op_node_cp_check_parent_paths(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  #[number] src_dev: u64,
  #[number] src_ino: u64,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  check_parent_paths_impl(&state, &fs, &src, src_dev, src_ino, &dest).await
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

  let parent_str = dest_parent.to_str().unwrap_or_default();
  let checked_parent = check_cp_path(state, parent_str, OpenAccessKind::Read)?;
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
      check_cp_path(state, parent_str, OpenAccessKind::Write)?;
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

/// Async op: ensures the parent directory of dest exists.
#[op2(stack_trace)]
pub async fn op_node_cp_ensure_parent_dir(
  state: Rc<RefCell<OpState>>,
  #[string] dest: String,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  ensure_parent_dir_impl(&state, &fs, &dest).await
}

fn handle_timestamps_and_mode_sync(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
  mut src_mode: u32,
) -> Result<(), FsError> {
  // Make sure the file is writable before setting the timestamp
  // otherwise open fails with EPERM when invoked with 'r+'
  if file_is_not_writable(src_mode) {
    src_mode |= 0o200;
  }

  // Set timestamps from a fresh stat of src (atime is modified by read).
  set_dest_timestamps_sync(fs, src_path, dest_path)?;
  set_dest_mode_sync(fs, dest_path, src_mode)?;
  Ok(())
}

fn file_is_not_writable(mode: u32) -> bool {
  (mode & 0o200) == 0
}

fn set_dest_mode_sync(
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

fn set_dest_timestamps_sync(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
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
        CpErrorKind::EExist {
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
  let (src_path, dest_path) = {
    let mut state = state.borrow_mut();
    (
      state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Owned(PathBuf::from(src)),
        OpenAccessKind::Read,
        Some("node:fs.cp"),
      )?,
      state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Owned(PathBuf::from(dest)),
        OpenAccessKind::Write,
        Some("node:fs.cp"),
      )?,
    )
  };

  spawn_blocking(move || -> Result<(), FsError> {
    fs.copy_file_sync(&src_path, &dest_path).map_err(|err| {
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
      handle_timestamps_and_mode_sync(&fs, &src_path, &dest_path, src_mode)?;
    } else {
      set_dest_mode_sync(&fs, &dest_path, src_mode)?;
    }
    Ok(())
  })
  .await?
}

/// Async op: handles copying a symlink (onLink + copyLink).
/// Returns null on success, or a CpErrorKind on error.
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
  let mut resolved_src =
    resolved_src_buf.to_str().unwrap_or_default().to_string();

  // Resolve relative symlink targets
  if !verbatim_symlinks
    && !Path::new(&resolved_src).is_absolute()
    && let Some(parent) = Path::new(&src).parent()
  {
    resolved_src = parent
      .join(&resolved_src)
      .to_str()
      .unwrap_or_default()
      .to_string();
  }

  if !dest_exists {
    {
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<PermissionsContainer>();
      permissions.check_write_all("node:fs.symlink")?;
      permissions.check_read_all("node:fs.symlink")?;
    }
    // PERMISSIONS: ok because we verified --allow-write and --allow-read above
    let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
    let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(&dest));
    fs.symlink_async(oldpath, newpath, None)
      .await
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(resolved_src),
            dest: Some(dest),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        )
      })?;

    return Ok(());
  }

  // Dest exists — try to read it as a symlink
  let dest_path = check_cp_path(&state, &dest, OpenAccessKind::ReadNoFollow)?;
  let resolved_dest_result = fs.read_link_async(dest_path).await;
  let resolved_dest = match resolved_dest_result {
    Ok(p) => {
      let s = p.to_str().unwrap_or_default().to_string();
      // If relative, resolve against dirname(dest)
      if !Path::new(&s).is_absolute() {
        if let Some(parent) = Path::new(&dest).parent() {
          parent.join(&s).to_str().unwrap_or_default().to_string()
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
        {
          let mut state = state.borrow_mut();
          let permissions = state.borrow_mut::<PermissionsContainer>();
          permissions.check_write_all("node:fs.symlink")?;
          permissions.check_read_all("node:fs.symlink")?;
        }
        // PERMISSIONS: ok because we verified --allow-write and --allow-read above
        let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
        let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(&dest));
        fs.symlink_async(oldpath, newpath, None).await?;
        return Ok(());
      }

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
      CpErrorKind::EInval {
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
      CpErrorKind::SymlinkToSubdirectory {
        message: format!(
          "cannot overwrite {} with {}",
          resolved_dest, resolved_src
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  let (src_path, dest_path) = {
    let mut state = state.borrow_mut();
    (
      state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Owned(PathBuf::from(resolved_src)),
        OpenAccessKind::Read,
        Some("node:fs.cp"),
      )?,
      state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Owned(PathBuf::from(dest)),
        OpenAccessKind::Write,
        Some("node:fs.cp"),
      )?,
    )
  };

  // Unlink dest and create new symlink
  spawn_blocking(move || -> Result<(), FsError> {
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

    fs.symlink_sync(&src_path, &dest_path, None)
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
      })?;
    Ok(())
  })
  .await?
}
