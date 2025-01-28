// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::Formatter;
use std::io;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::path::StripPrefixError;
use std::rc::Rc;

use boxed_error::Boxed;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::FastString;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_error::JsErrorBox;
use deno_io::fs::FileResource;
use deno_io::fs::FsError;
use deno_io::fs::FsStat;
use deno_permissions::PermissionCheckError;
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rand::Rng;
use serde::Serialize;

use crate::interface::AccessCheckFn;
use crate::interface::FileSystemRc;
use crate::interface::FsDirEntry;
use crate::interface::FsFileType;
use crate::FsPermissions;
use crate::OpenOptions;

#[derive(Debug, Boxed, deno_error::JsError)]
pub struct FsOpsError(pub Box<FsOpsErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FsOpsErrorKind {
  #[class(inherit)]
  #[error("{0}")]
  Io(#[source] std::io::Error),
  #[class(inherit)]
  #[error("{0}")]
  OperationError(#[source] OperationError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] ResourceError),
  #[class("InvalidData")]
  #[error("File name or path {0:?} is not valid UTF-8")]
  InvalidUtf8(std::ffi::OsString),
  #[class(generic)]
  #[error("{0}")]
  StripPrefix(#[from] StripPrefixError),
  #[class(inherit)]
  #[error("{0}")]
  Canceled(#[from] deno_core::Canceled),
  #[class(type)]
  #[error("Invalid seek mode: {0}")]
  InvalidSeekMode(i32),
  #[class(generic)]
  #[error("Invalid control character in prefix or suffix: {0:?}")]
  InvalidControlCharacter(String),
  #[class(generic)]
  #[error("Invalid character in prefix or suffix: {0:?}")]
  InvalidCharacter(String),
  #[cfg(windows)]
  #[class(generic)]
  #[error("Invalid trailing character in suffix")]
  InvalidTrailingCharacter,
  #[class("NotCapable")]
  #[error("Requires {err} access to {path}, {}", print_not_capable_info(*.standalone, .err))]
  NotCapableAccess {
    // NotCapable
    standalone: bool,
    err: &'static str,
    path: String,
  },
  #[class("NotCapable")]
  #[error("permission denied: {0}")]
  NotCapable(&'static str),
  #[class(inherit)]
  #[error(transparent)]
  Other(JsErrorBox),
}

impl From<FsError> for FsOpsError {
  fn from(err: FsError) -> Self {
    match err {
      FsError::Io(err) => FsOpsErrorKind::Io(err),
      FsError::FileBusy => FsOpsErrorKind::Resource(ResourceError::Unavailable),
      FsError::NotSupported => {
        FsOpsErrorKind::Other(JsErrorBox::not_supported())
      }
      FsError::NotCapable(err) => FsOpsErrorKind::NotCapable(err),
    }
    .into_box()
  }
}

fn print_not_capable_info(standalone: bool, err: &'static str) -> String {
  if standalone {
    format!("specify the required permissions during compilation using `deno compile --allow-{err}`")
  } else {
    format!("run again with the --allow-{err} flag")
  }
}

fn sync_permission_check<'a, P: FsPermissions + 'static>(
  permissions: &'a mut P,
  api_name: &'static str,
) -> impl AccessCheckFn + 'a {
  move |resolved, path, options| {
    permissions.check(resolved, options, path, api_name)
  }
}

fn async_permission_check<P: FsPermissions + 'static>(
  state: Rc<RefCell<OpState>>,
  api_name: &'static str,
) -> impl AccessCheckFn {
  move |resolved, path, options| {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check(resolved, options, path, api_name)
  }
}

fn map_permission_error(
  operation: &'static str,
  error: FsError,
  path: &Path,
) -> FsOpsError {
  match error {
    FsError::NotCapable(err) => {
      let path = format!("{path:?}");
      let (path, truncated) = if path.len() > 1024 {
        (&path[0..1024], "...(truncated)")
      } else {
        (path.as_str(), "")
      };

      FsOpsErrorKind::NotCapableAccess {
        standalone: deno_permissions::is_standalone(),
        err,
        path: format!("{path}{truncated}"),
      }
      .into_box()
    }
    err => Err::<(), _>(err)
      .context_path(operation, path)
      .err()
      .unwrap(),
  }
}

#[op2(stack_trace)]
#[string]
pub fn op_fs_cwd<P>(state: &mut OpState) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let fs = state.borrow::<FileSystemRc>();
  let path = fs.cwd()?;
  let path_str = path_into_string(path.into_os_string())?;
  Ok(path_str)
}

#[op2(fast, stack_trace)]
pub fn op_fs_chdir<P>(
  state: &mut OpState,
  #[string] directory: &str,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let d = state
    .borrow_mut::<P>()
    .check_read(directory, "Deno.chdir()")?;
  state
    .borrow::<FileSystemRc>()
    .chdir(&d)
    .context_path("chdir", &d)
}

#[op2]
pub fn op_fs_umask(
  state: &mut OpState,
  mask: Option<u32>,
) -> Result<u32, FsOpsError>
where
{
  state.borrow::<FileSystemRc>().umask(mask).context("umask")
}

#[op2(stack_trace)]
#[smi]
pub fn op_fs_open_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  #[serde] options: Option<OpenOptions>,
) -> Result<ResourceId, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = options.unwrap_or_else(OpenOptions::read);

  let fs = state.borrow::<FileSystemRc>().clone();
  let mut access_check =
    sync_permission_check::<P>(state.borrow_mut(), "Deno.openSync()");
  let file = fs
    .open_sync(&path, options, Some(&mut access_check))
    .map_err(|error| map_permission_error("open", error, &path))?;
  drop(access_check);
  let rid = state
    .resource_table
    .add(FileResource::new(file, "fsFile".to_string()));
  Ok(rid)
}

#[op2(async, stack_trace)]
#[smi]
pub async fn op_fs_open_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[serde] options: Option<OpenOptions>,
) -> Result<ResourceId, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = options.unwrap_or_else(OpenOptions::read);
  let mut access_check =
    async_permission_check::<P>(state.clone(), "Deno.open()");
  let fs = state.borrow().borrow::<FileSystemRc>().clone();
  let file = fs
    .open_async(path.clone(), options, Some(&mut access_check))
    .await
    .map_err(|error| map_permission_error("open", error, &path))?;

  let rid = state
    .borrow_mut()
    .resource_table
    .add(FileResource::new(file, "fsFile".to_string()));
  Ok(rid)
}

#[op2(stack_trace)]
pub fn op_fs_mkdir_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  recursive: bool,
  mode: Option<u32>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let mode = mode.unwrap_or(0o777) & 0o777;

  let path = state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.mkdirSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.mkdir_sync(&path, recursive, Some(mode))
    .context_path("mkdir", &path)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_mkdir_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  recursive: bool,
  mode: Option<u32>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let mode = mode.unwrap_or(0o777) & 0o777;

  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_write(&path, "Deno.mkdir()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  fs.mkdir_async(path.clone(), recursive, Some(mode))
    .await
    .context_path("mkdir", &path)?;

  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_chmod_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  mode: u32,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.chmodSync()")?;
  let fs = state.borrow::<FileSystemRc>();
  fs.chmod_sync(&path, mode).context_path("chmod", &path)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_chmod_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  mode: u32,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_write(&path, "Deno.chmod()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.chmod_async(path.clone(), mode)
    .await
    .context_path("chmod", &path)?;
  Ok(())
}

#[op2(stack_trace)]
pub fn op_fs_chown_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.chownSync()")?;
  let fs = state.borrow::<FileSystemRc>();
  fs.chown_sync(&path, uid, gid)
    .context_path("chown", &path)?;
  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_chown_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_write(&path, "Deno.chown()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  fs.chown_async(path.clone(), uid, gid)
    .await
    .context_path("chown", &path)?;
  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_remove_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  recursive: bool,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_write(path, "Deno.removeSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.remove_sync(&path, recursive)
    .context_path("remove", &path)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_remove_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  recursive: bool,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = if recursive {
      state
        .borrow_mut::<P>()
        .check_write(&path, "Deno.remove()")?
    } else {
      state
        .borrow_mut::<P>()
        .check_write_partial(&path, "Deno.remove()")?
    };

    (state.borrow::<FileSystemRc>().clone(), path)
  };

  fs.remove_async(path.clone(), recursive)
    .await
    .context_path("remove", &path)?;

  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_copy_file_sync<P>(
  state: &mut OpState,
  #[string] from: &str,
  #[string] to: &str,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let permissions = state.borrow_mut::<P>();
  let from = permissions.check_read(from, "Deno.copyFileSync()")?;
  let to = permissions.check_write(to, "Deno.copyFileSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.copy_file_sync(&from, &to)
    .context_two_path("copy", &from, &to)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_copy_file_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] from: String,
  #[string] to: String,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, from, to) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    let from = permissions.check_read(&from, "Deno.copyFile()")?;
    let to = permissions.check_write(&to, "Deno.copyFile()")?;
    (state.borrow::<FileSystemRc>().clone(), from, to)
  };

  fs.copy_file_async(from.clone(), to.clone())
    .await
    .context_two_path("copy", &from, &to)?;

  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_stat_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  #[buffer] stat_out_buf: &mut [u32],
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.statSync()")?;
  let fs = state.borrow::<FileSystemRc>();
  let stat = fs.stat_sync(&path).context_path("stat", &path)?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_fs_stat_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<SerializableStat, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    let path = permissions.check_read(&path, "Deno.stat()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  let stat = fs
    .stat_async(path.clone())
    .await
    .context_path("stat", &path)?;
  Ok(SerializableStat::from(stat))
}

#[op2(fast, stack_trace)]
pub fn op_fs_lstat_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  #[buffer] stat_out_buf: &mut [u32],
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.lstatSync()")?;
  let fs = state.borrow::<FileSystemRc>();
  let stat = fs.lstat_sync(&path).context_path("lstat", &path)?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_fs_lstat_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<SerializableStat, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    let path = permissions.check_read(&path, "Deno.lstat()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };
  let stat = fs
    .lstat_async(path.clone())
    .await
    .context_path("lstat", &path)?;
  Ok(SerializableStat::from(stat))
}

#[op2(stack_trace)]
#[string]
pub fn op_fs_realpath_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let fs = state.borrow::<FileSystemRc>().clone();
  let permissions = state.borrow_mut::<P>();
  let path = permissions.check_read(&path, "Deno.realPathSync()")?;
  if path.is_relative() {
    permissions.check_read_blind(&fs.cwd()?, "CWD", "Deno.realPathSync()")?;
  }

  let resolved_path =
    fs.realpath_sync(&path).context_path("realpath", &path)?;

  let path_string = path_into_string(resolved_path.into_os_string())?;
  Ok(path_string)
}

#[op2(async, stack_trace)]
#[string]
pub async fn op_fs_realpath_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let fs = state.borrow::<FileSystemRc>().clone();
    let permissions = state.borrow_mut::<P>();
    let path = permissions.check_read(&path, "Deno.realPath()")?;
    if path.is_relative() {
      permissions.check_read_blind(&fs.cwd()?, "CWD", "Deno.realPath()")?;
    }
    (fs, path)
  };
  let resolved_path = fs
    .realpath_async(path.clone())
    .await
    .context_path("realpath", &path)?;

  let path_string = path_into_string(resolved_path.into_os_string())?;
  Ok(path_string)
}

#[op2(stack_trace)]
#[serde]
pub fn op_fs_read_dir_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<Vec<FsDirEntry>, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readDirSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  let entries = fs.read_dir_sync(&path).context_path("readdir", &path)?;

  Ok(entries)
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_fs_read_dir_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<Vec<FsDirEntry>, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readDir()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  let entries = fs
    .read_dir_async(path.clone())
    .await
    .context_path("readdir", &path)?;

  Ok(entries)
}

#[op2(fast, stack_trace)]
pub fn op_fs_rename_sync<P>(
  state: &mut OpState,
  #[string] oldpath: String,
  #[string] newpath: String,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let permissions = state.borrow_mut::<P>();
  let _ = permissions.check_read(&oldpath, "Deno.renameSync()")?;
  let oldpath = permissions.check_write(&oldpath, "Deno.renameSync()")?;
  let newpath = permissions.check_write(&newpath, "Deno.renameSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.rename_sync(&oldpath, &newpath)
    .context_two_path("rename", &oldpath, &newpath)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_rename_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] oldpath: String,
  #[string] newpath: String,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, oldpath, newpath) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    _ = permissions.check_read(&oldpath, "Deno.rename()")?;
    let oldpath = permissions.check_write(&oldpath, "Deno.rename()")?;
    let newpath = permissions.check_write(&newpath, "Deno.rename()")?;
    (state.borrow::<FileSystemRc>().clone(), oldpath, newpath)
  };

  fs.rename_async(oldpath.clone(), newpath.clone())
    .await
    .context_two_path("rename", &oldpath, &newpath)?;

  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_link_sync<P>(
  state: &mut OpState,
  #[string] oldpath: &str,
  #[string] newpath: &str,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let permissions = state.borrow_mut::<P>();
  _ = permissions.check_read(oldpath, "Deno.linkSync()")?;
  let oldpath = permissions.check_write(oldpath, "Deno.linkSync()")?;
  _ = permissions.check_read(newpath, "Deno.linkSync()")?;
  let newpath = permissions.check_write(newpath, "Deno.linkSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.link_sync(&oldpath, &newpath)
    .context_two_path("link", &oldpath, &newpath)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_link_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] oldpath: String,
  #[string] newpath: String,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, oldpath, newpath) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    _ = permissions.check_read(&oldpath, "Deno.link()")?;
    let oldpath = permissions.check_write(&oldpath, "Deno.link()")?;
    _ = permissions.check_read(&newpath, "Deno.link()")?;
    let newpath = permissions.check_write(&newpath, "Deno.link()")?;
    (state.borrow::<FileSystemRc>().clone(), oldpath, newpath)
  };

  fs.link_async(oldpath.clone(), newpath.clone())
    .await
    .context_two_path("link", &oldpath, &newpath)?;

  Ok(())
}

#[op2(stack_trace)]
pub fn op_fs_symlink_sync<P>(
  state: &mut OpState,
  #[string] oldpath: &str,
  #[string] newpath: &str,
  #[serde] file_type: Option<FsFileType>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_write_all("Deno.symlinkSync()")?;
  permissions.check_read_all("Deno.symlinkSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.symlink_sync(&oldpath, &newpath, file_type)
    .context_two_path("symlink", &oldpath, &newpath)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_symlink_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] oldpath: String,
  #[string] newpath: String,
  #[serde] file_type: Option<FsFileType>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);

  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_write_all("Deno.symlink()")?;
    permissions.check_read_all("Deno.symlink()")?;
    state.borrow::<FileSystemRc>().clone()
  };

  fs.symlink_async(oldpath.clone(), newpath.clone(), file_type)
    .await
    .context_two_path("symlink", &oldpath, &newpath)?;

  Ok(())
}

#[op2(stack_trace)]
#[string]
pub fn op_fs_read_link_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readLink()")?;

  let fs = state.borrow::<FileSystemRc>();

  let target = fs.read_link_sync(&path).context_path("readlink", &path)?;
  let target_string = path_into_string(target.into_os_string())?;
  Ok(target_string)
}

#[op2(async, stack_trace)]
#[string]
pub async fn op_fs_read_link_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readLink()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  let target = fs
    .read_link_async(path.clone())
    .await
    .context_path("readlink", &path)?;
  let target_string = path_into_string(target.into_os_string())?;
  Ok(target_string)
}

#[op2(fast, stack_trace)]
pub fn op_fs_truncate_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[number] len: u64,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state
    .borrow_mut::<P>()
    .check_write(path, "Deno.truncateSync()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.truncate_sync(&path, len)
    .context_path("truncate", &path)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_truncate_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[number] len: u64,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state
      .borrow_mut::<P>()
      .check_write(&path, "Deno.truncate()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  fs.truncate_async(path.clone(), len)
    .await
    .context_path("truncate", &path)?;

  Ok(())
}

#[op2(fast, stack_trace)]
pub fn op_fs_utime_sync<P>(
  state: &mut OpState,
  #[string] path: &str,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = state.borrow_mut::<P>().check_write(path, "Deno.utime()")?;

  let fs = state.borrow::<FileSystemRc>();
  fs.utime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .context_path("utime", &path)?;

  Ok(())
}

#[op2(async, stack_trace)]
pub async fn op_fs_utime_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (fs, path) = {
    let mut state = state.borrow_mut();
    let path = state.borrow_mut::<P>().check_write(&path, "Deno.utime()")?;
    (state.borrow::<FileSystemRc>().clone(), path)
  };

  fs.utime_async(
    path.clone(),
    atime_secs,
    atime_nanos,
    mtime_secs,
    mtime_nanos,
  )
  .await
  .context_path("utime", &path)?;

  Ok(())
}

#[op2(stack_trace)]
#[string]
pub fn op_fs_make_temp_dir_sync<P>(
  state: &mut OpState,
  #[string] dir_arg: Option<String>,
  #[string] prefix: Option<String>,
  #[string] suffix: Option<String>,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_sync::<P>(
    state,
    dir_arg.as_deref(),
    "Deno.makeTempDirSync()",
  )?;

  let mut rng = thread_rng();

  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs.mkdir_sync(&path, false, Some(0o700)) {
      Ok(_) => {
        // PERMISSIONS: ensure the absolute path is not leaked
        let path = strip_dir_prefix(&dir, dir_arg.as_deref(), path)?;
        return path_into_string(path.into_os_string());
      }
      Err(FsError::Io(ref e)) if e.kind() == io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(e) => return Err(e).context("tmpdir"),
    }
  }

  Err(FsError::Io(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "too many temp dirs exist",
  )))
  .context("tmpdir")
}

#[op2(async, stack_trace)]
#[string]
pub async fn op_fs_make_temp_dir_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] dir_arg: Option<String>,
  #[string] prefix: Option<String>,
  #[string] suffix: Option<String>,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_async::<P>(
    state,
    dir_arg.as_deref(),
    "Deno.makeTempDir()",
  )?;

  let mut rng = thread_rng();

  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs
      .clone()
      .mkdir_async(path.clone(), false, Some(0o700))
      .await
    {
      Ok(_) => {
        // PERMISSIONS: ensure the absolute path is not leaked
        let path = strip_dir_prefix(&dir, dir_arg.as_deref(), path)?;
        return path_into_string(path.into_os_string());
      }
      Err(FsError::Io(ref e)) if e.kind() == io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(e) => return Err(e).context("tmpdir"),
    }
  }

  Err(FsError::Io(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "too many temp dirs exist",
  )))
  .context("tmpdir")
}

#[op2(stack_trace)]
#[string]
pub fn op_fs_make_temp_file_sync<P>(
  state: &mut OpState,
  #[string] dir_arg: Option<String>,
  #[string] prefix: Option<String>,
  #[string] suffix: Option<String>,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_sync::<P>(
    state,
    dir_arg.as_deref(),
    "Deno.makeTempFileSync()",
  )?;

  let open_opts = OpenOptions {
    write: true,
    create_new: true,
    mode: Some(0o600),
    ..Default::default()
  };

  let mut rng = thread_rng();
  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs.open_sync(&path, open_opts, None) {
      Ok(_) => {
        // PERMISSIONS: ensure the absolute path is not leaked
        let path = strip_dir_prefix(&dir, dir_arg.as_deref(), path)?;
        return path_into_string(path.into_os_string());
      }
      Err(FsError::Io(ref e)) if e.kind() == io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(e) => return Err(e).context("tmpfile"),
    }
  }

  Err(FsError::Io(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "too many temp files exist",
  )))
  .context("tmpfile")
}

#[op2(async, stack_trace)]
#[string]
pub async fn op_fs_make_temp_file_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] dir_arg: Option<String>,
  #[string] prefix: Option<String>,
  #[string] suffix: Option<String>,
) -> Result<String, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_async::<P>(
    state,
    dir_arg.as_deref(),
    "Deno.makeTempFile()",
  )?;

  let open_opts = OpenOptions {
    write: true,
    create_new: true,
    mode: Some(0o600),
    ..Default::default()
  };

  let mut rng = thread_rng();

  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs.clone().open_async(path.clone(), open_opts, None).await {
      Ok(_) => {
        // PERMISSIONS: ensure the absolute path is not leaked
        let path = strip_dir_prefix(&dir, dir_arg.as_deref(), path)?;
        return path_into_string(path.into_os_string());
      }
      Err(FsError::Io(ref e)) if e.kind() == io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(e) => return Err(e).context("tmpfile"),
    }
  }
  Err(FsError::Io(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "too many temp files exist",
  )))
  .context("tmpfile")
}

fn strip_dir_prefix(
  resolved_dir: &Path,
  dir_arg: Option<&str>,
  result_path: PathBuf,
) -> Result<PathBuf, StripPrefixError> {
  if resolved_dir.is_absolute() {
    match &dir_arg {
      Some(dir_arg) => {
        Ok(Path::new(dir_arg).join(result_path.strip_prefix(resolved_dir)?))
      }
      None => Ok(result_path),
    }
  } else {
    Ok(result_path)
  }
}

fn make_temp_check_sync<P>(
  state: &mut OpState,
  dir: Option<&str>,
  api_name: &str,
) -> Result<(PathBuf, FileSystemRc), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let fs = state.borrow::<FileSystemRc>().clone();
  let dir = match dir {
    Some(dir) => state.borrow_mut::<P>().check_write(dir, api_name)?,
    None => {
      let dir = fs.tmp_dir().context("tmpdir")?;
      state
        .borrow_mut::<P>()
        .check_write_blind(&dir, "TMP", api_name)?;
      dir
    }
  };
  Ok((dir, fs))
}

fn make_temp_check_async<P>(
  state: Rc<RefCell<OpState>>,
  dir: Option<&str>,
  api_name: &str,
) -> Result<(PathBuf, FileSystemRc), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let mut state = state.borrow_mut();
  let fs = state.borrow::<FileSystemRc>().clone();
  let dir = match dir {
    Some(dir) => state.borrow_mut::<P>().check_write(dir, api_name)?,
    None => {
      let dir = fs.tmp_dir().context("tmpdir")?;
      state
        .borrow_mut::<P>()
        .check_write_blind(&dir, "TMP", api_name)?;
      dir
    }
  };
  Ok((dir, fs))
}

/// Identify illegal filename characters before attempting to use them in a filesystem
/// operation. We're a bit stricter with tempfile and tempdir names than with regular
/// files.
fn validate_temporary_filename_component(
  component: &str,
  #[allow(unused_variables)] suffix: bool,
) -> Result<(), FsOpsError> {
  // Ban ASCII and Unicode control characters: these will often fail
  if let Some(c) = component.matches(|c: char| c.is_control()).next() {
    return Err(
      FsOpsErrorKind::InvalidControlCharacter(c.to_string()).into_box(),
    );
  }
  // Windows has the most restrictive filenames. As temp files aren't normal files, we just
  // use this set of banned characters for all platforms because wildcard-like files can also
  // be problematic in unix-like shells.

  // The list of banned characters in Windows:
  // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#naming-conventions

  // You might ask why <, >, and " are included in the Windows list? It's because they are
  // special wildcard implemented in the filesystem driver!
  // https://learn.microsoft.com/en-ca/archive/blogs/jeremykuhne/wildcards-in-windows
  if let Some(c) = component
    .matches(|c: char| "<>:\"/\\|?*".contains(c))
    .next()
  {
    return Err(FsOpsErrorKind::InvalidCharacter(c.to_string()).into_box());
  }

  // This check is only for Windows
  #[cfg(windows)]
  if suffix && component.ends_with(|c: char| ". ".contains(c)) {
    return Err(FsOpsErrorKind::InvalidTrailingCharacter.into_box());
  }

  Ok(())
}

fn tmp_name(
  rng: &mut ThreadRng,
  dir: &Path,
  prefix: Option<&str>,
  suffix: Option<&str>,
) -> Result<PathBuf, FsOpsError> {
  let prefix = prefix.unwrap_or("");
  validate_temporary_filename_component(prefix, false)?;
  let suffix = suffix.unwrap_or("");
  validate_temporary_filename_component(suffix, true)?;

  // If we use a 32-bit number, we only need ~70k temp files before we have a 50%
  // chance of collision. By bumping this up to 64-bits, we require ~5 billion
  // before hitting a 50% chance. We also base32-encode this value so the entire
  // thing is 1) case insensitive and 2) slightly shorter than the equivalent hex
  // value.
  let unique = rng.gen::<u64>();
  base32::encode(base32::Alphabet::Crockford, &unique.to_le_bytes());
  let path = dir.join(format!("{prefix}{unique:08x}{suffix}"));

  Ok(path)
}

#[op2(stack_trace)]
pub fn op_fs_write_file_sync<P>(
  state: &mut OpState,
  #[string] path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  create_new: bool,
  #[buffer] data: JsBuffer,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = OpenOptions::write(create, append, create_new, mode);
  let fs = state.borrow::<FileSystemRc>().clone();
  let mut access_check =
    sync_permission_check::<P>(state.borrow_mut(), "Deno.writeFileSync()");

  fs.write_file_sync(&path, options, Some(&mut access_check), &data)
    .map_err(|error| map_permission_error("writefile", error, &path))?;

  Ok(())
}

#[op2(async, stack_trace)]
#[allow(clippy::too_many_arguments)]
pub async fn op_fs_write_file_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] mode: Option<u32>,
  append: bool,
  create: bool,
  create_new: bool,
  #[buffer] data: JsBuffer,
  #[smi] cancel_rid: Option<ResourceId>,
) -> Result<(), FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = OpenOptions::write(create, append, create_new, mode);

  let mut access_check =
    async_permission_check::<P>(state.clone(), "Deno.writeFile()");
  let (fs, cancel_handle) = {
    let state = state.borrow_mut();
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<FileSystemRc>().clone(), cancel_handle)
  };

  let fut = fs.write_file_async(
    path.clone(),
    options,
    Some(&mut access_check),
    data.to_vec(),
  );

  if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      if let Ok(res) = state.borrow_mut().resource_table.take_any(cancel_rid) {
        res.close();
      }
    };

    res?.map_err(|error| map_permission_error("writefile", error, &path))?;
  } else {
    fut
      .await
      .map_err(|error| map_permission_error("writefile", error, &path))?;
  }

  Ok(())
}

#[op2(stack_trace)]
#[serde]
pub fn op_fs_read_file_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<ToJsBuffer, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = state.borrow::<FileSystemRc>().clone();
  let mut access_check =
    sync_permission_check::<P>(state.borrow_mut(), "Deno.readFileSync()");
  let buf = fs
    .read_file_sync(&path, Some(&mut access_check))
    .map_err(|error| map_permission_error("readfile", error, &path))?;

  // todo(https://github.com/denoland/deno/issues/27107): do not clone here
  Ok(buf.into_owned().into_boxed_slice().into())
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_fs_read_file_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] cancel_rid: Option<ResourceId>,
) -> Result<ToJsBuffer, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let mut access_check =
    async_permission_check::<P>(state.clone(), "Deno.readFile()");
  let (fs, cancel_handle) = {
    let state = state.borrow();
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<FileSystemRc>().clone(), cancel_handle)
  };

  let fut = fs.read_file_async(path.clone(), Some(&mut access_check));

  let buf = if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      if let Ok(res) = state.borrow_mut().resource_table.take_any(cancel_rid) {
        res.close();
      }
    };

    res?.map_err(|error| map_permission_error("readfile", error, &path))?
  } else {
    fut
      .await
      .map_err(|error| map_permission_error("readfile", error, &path))?
  };

  // todo(https://github.com/denoland/deno/issues/27107): do not clone here
  Ok(buf.into_owned().into_boxed_slice().into())
}

#[op2(stack_trace)]
#[to_v8]
pub fn op_fs_read_file_text_sync<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<FastString, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = state.borrow::<FileSystemRc>().clone();
  let mut access_check =
    sync_permission_check::<P>(state.borrow_mut(), "Deno.readFileSync()");
  let str = fs
    .read_text_file_lossy_sync(&path, Some(&mut access_check))
    .map_err(|error| map_permission_error("readfile", error, &path))?;
  Ok(match str {
    Cow::Borrowed(text) => FastString::from_static(text),
    Cow::Owned(value) => value.into(),
  })
}

#[op2(async, stack_trace)]
#[to_v8]
pub async fn op_fs_read_file_text_async<P>(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  #[smi] cancel_rid: Option<ResourceId>,
) -> Result<FastString, FsOpsError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let mut access_check =
    async_permission_check::<P>(state.clone(), "Deno.readFile()");
  let (fs, cancel_handle) = {
    let state = state.borrow_mut();
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<FileSystemRc>().clone(), cancel_handle)
  };

  let fut =
    fs.read_text_file_lossy_async(path.clone(), Some(&mut access_check));

  let str = if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      if let Ok(res) = state.borrow_mut().resource_table.take_any(cancel_rid) {
        res.close();
      }
    };

    res?.map_err(|error| map_permission_error("readfile", error, &path))?
  } else {
    fut
      .await
      .map_err(|error| map_permission_error("readfile", error, &path))?
  };

  Ok(match str {
    Cow::Borrowed(text) => FastString::from_static(text),
    Cow::Owned(value) => value.into(),
  })
}

fn to_seek_from(offset: i64, whence: i32) -> Result<SeekFrom, FsOpsError> {
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(offset),
    2 => SeekFrom::End(offset),
    _ => {
      return Err(FsOpsErrorKind::InvalidSeekMode(whence).into_box());
    }
  };
  Ok(seek_from)
}

#[op2(fast)]
#[number]
pub fn op_fs_seek_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[number] offset: i64,
  #[smi] whence: i32,
) -> Result<u64, FsOpsError> {
  let pos = to_seek_from(offset, whence)?;
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  let cursor = file.seek_sync(pos)?;
  Ok(cursor)
}

#[op2(async)]
#[number]
pub async fn op_fs_seek_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[number] offset: i64,
  #[smi] whence: i32,
) -> Result<u64, FsOpsError> {
  let pos = to_seek_from(offset, whence)?;
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  let cursor = file.seek_async(pos).await?;
  Ok(cursor)
}

#[op2(fast)]
pub fn op_fs_file_sync_data_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.datasync_sync()?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_file_sync_data_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file.datasync_async().await?;
  Ok(())
}

#[op2(fast)]
pub fn op_fs_file_sync_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.sync_sync()?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_file_sync_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file.sync_async().await?;
  Ok(())
}

#[op2(fast)]
pub fn op_fs_file_stat_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[buffer] stat_out_buf: &mut [u32],
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  let stat = file.stat_sync()?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op2(async)]
#[serde]
pub async fn op_fs_file_stat_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<SerializableStat, FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  let stat = file.stat_async().await?;
  Ok(stat.into())
}

#[op2(fast)]
pub fn op_fs_flock_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  exclusive: bool,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.lock_sync(exclusive)?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_flock_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  exclusive: bool,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file.lock_async(exclusive).await?;
  Ok(())
}

#[op2(fast)]
pub fn op_fs_funlock_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.unlock_sync()?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_funlock_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file.unlock_async().await?;
  Ok(())
}

#[op2(fast)]
pub fn op_fs_ftruncate_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[number] len: u64,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.truncate_sync(len)?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_file_truncate_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[number] len: u64,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file.truncate_async(len).await?;
  Ok(())
}

#[op2(fast)]
pub fn op_fs_futime_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsOpsError> {
  let file =
    FileResource::get_file(state, rid).map_err(FsOpsErrorKind::Resource)?;
  file.utime_sync(atime_secs, atime_nanos, mtime_secs, mtime_nanos)?;
  Ok(())
}

#[op2(async)]
pub async fn op_fs_futime_async(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[number] atime_secs: i64,
  #[smi] atime_nanos: u32,
  #[number] mtime_secs: i64,
  #[smi] mtime_nanos: u32,
) -> Result<(), FsOpsError> {
  let file = FileResource::get_file(&state.borrow(), rid)
    .map_err(FsOpsErrorKind::Resource)?;
  file
    .utime_async(atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .await?;
  Ok(())
}

#[derive(Debug, deno_error::JsError)]
#[class(inherit)]
pub struct OperationError {
  operation: &'static str,
  kind: OperationErrorKind,
  #[inherit]
  pub err: FsError,
}

impl std::fmt::Display for OperationError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    if let FsError::Io(e) = &self.err {
      std::fmt::Display::fmt(&e, f)?;
      f.write_str(": ")?;
    }

    f.write_str(self.operation)?;

    match &self.kind {
      OperationErrorKind::Bare => Ok(()),
      OperationErrorKind::WithPath(path) => write!(f, " '{}'", path.display()),
      OperationErrorKind::WithTwoPaths(from, to) => {
        write!(f, " '{}' -> '{}'", from.display(), to.display())
      }
    }
  }
}

impl std::error::Error for OperationError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    if let FsError::Io(err) = &self.err {
      Some(err)
    } else {
      None
    }
  }
}

#[derive(Debug)]
pub enum OperationErrorKind {
  Bare,
  WithPath(PathBuf),
  WithTwoPaths(PathBuf, PathBuf),
}

trait MapErrContext {
  type R;

  fn context_fn<F>(self, f: F) -> Self::R
  where
    F: FnOnce(FsError) -> OperationError;

  fn context(self, desc: &'static str) -> Self::R;

  fn context_path(self, operation: &'static str, path: &Path) -> Self::R;

  fn context_two_path(
    self,
    operation: &'static str,
    from: &Path,
    to: &Path,
  ) -> Self::R;
}

impl<T> MapErrContext for Result<T, FsError> {
  type R = Result<T, FsOpsError>;

  fn context_fn<F>(self, f: F) -> Self::R
  where
    F: FnOnce(FsError) -> OperationError,
  {
    self.map_err(|err| FsOpsErrorKind::OperationError(f(err)).into_box())
  }

  fn context(self, operation: &'static str) -> Self::R {
    self.context_fn(move |err| OperationError {
      operation,
      kind: OperationErrorKind::Bare,
      err,
    })
  }

  fn context_path(self, operation: &'static str, path: &Path) -> Self::R {
    self.context_fn(|err| OperationError {
      operation,
      kind: OperationErrorKind::WithPath(path.to_path_buf()),
      err,
    })
  }

  fn context_two_path(
    self,
    operation: &'static str,
    oldpath: &Path,
    newpath: &Path,
  ) -> Self::R {
    self.context_fn(|err| OperationError {
      operation,
      kind: OperationErrorKind::WithTwoPaths(
        oldpath.to_path_buf(),
        newpath.to_path_buf(),
      ),
      err,
    })
  }
}

fn path_into_string(s: std::ffi::OsString) -> Result<String, FsOpsError> {
  s.into_string()
    .map_err(|e| FsOpsErrorKind::InvalidUtf8(e).into_box())
}

macro_rules! create_struct_writer {
  (pub struct $name:ident { $($field:ident: $type:ty),* $(,)? }) => {
    impl $name {
      fn write(self, buf: &mut [u32]) {
        let mut offset = 0;
        $(
          let value = self.$field as u64;
          buf[offset] = value as u32;
          buf[offset + 1] = (value >> 32) as u32;
          #[allow(unused_assignments)]
          {
            offset += 2;
          }
        )*
      }
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct $name {
      $($field: $type),*
    }
  };
}

create_struct_writer! {
  pub struct SerializableStat {
    is_file: bool,
    is_directory: bool,
    is_symlink: bool,
    size: u64,
    // In milliseconds, like JavaScript. Available on both Unix or Windows.
    mtime_set: bool,
    mtime: u64,
    atime_set: bool,
    atime: u64,
    birthtime_set: bool,
    birthtime: u64,
    ctime_set: bool,
    ctime: u64,
    // Following are only valid under Unix.
    dev: u64,
    ino: u64,
    mode: u32,
    nlink: u64,
    uid: u32,
    gid: u32,
    rdev: u64,
    blksize: u64,
    blocks: u64,
    is_block_device: bool,
    is_char_device: bool,
    is_fifo: bool,
    is_socket: bool,
  }
}

impl From<FsStat> for SerializableStat {
  fn from(stat: FsStat) -> Self {
    SerializableStat {
      is_file: stat.is_file,
      is_directory: stat.is_directory,
      is_symlink: stat.is_symlink,
      size: stat.size,

      mtime_set: stat.mtime.is_some(),
      mtime: stat.mtime.unwrap_or(0),
      atime_set: stat.atime.is_some(),
      atime: stat.atime.unwrap_or(0),
      birthtime_set: stat.birthtime.is_some(),
      birthtime: stat.birthtime.unwrap_or(0),
      ctime_set: stat.ctime.is_some(),
      ctime: stat.ctime.unwrap_or(0),

      dev: stat.dev,
      ino: stat.ino,
      mode: stat.mode,
      nlink: stat.nlink,
      uid: stat.uid,
      gid: stat.gid,
      rdev: stat.rdev,
      blksize: stat.blksize,
      blocks: stat.blocks,
      is_block_device: stat.is_block_device,
      is_char_device: stat.is_char_device,
      is_fifo: stat.is_fifo,
      is_socket: stat.is_socket,
    }
  }
}
