// Copyright 2018-2025 the Deno authors. MIT license.

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
use deno_permissions::CheckedPath;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use serde::Serialize;

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

#[op2(async, stack_trace)]
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
  let write = open_options.write || open_options.append;
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

#[op2(async, stack_trace)]
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

#[op2(async, stack_trace)]
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

#[op2(async, stack_trace)]
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

#[op2(async, stack_trace)]
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

#[op2(async, stack_trace)]
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

#[op2(async, stack_trace)]
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
