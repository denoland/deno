// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::io;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use rand::rngs::ThreadRng;
use rand::thread_rng;
use rand::Rng;
use serde::Serialize;

use crate::check_unstable;
use crate::check_unstable2;
use crate::interface::FsDirEntry;
use crate::interface::FsError;
use crate::interface::FsFileType;
use crate::interface::FsStat;
use crate::File;
use crate::FileSystem;
use crate::FsPermissions;
use crate::OpenOptions;

#[op]
pub fn op_cwd<Fs, P>(state: &mut OpState) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let fs = state.borrow::<Fs>();
  let path = fs.cwd()?;
  state
    .borrow_mut::<P>()
    .check_read_blind(&path, "CWD", "Deno.cwd()")?;
  let path_str = path_into_string(path.into_os_string())?;
  Ok(path_str)
}

#[op]
fn op_chdir<Fs, P>(state: &mut OpState, directory: &str) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let d = PathBuf::from(&directory);
  state.borrow_mut::<P>().check_read(&d, "Deno.chdir()")?;
  state.borrow::<Fs>().chdir(&d).context_path("chdir", &d)
}

#[op]
fn op_umask<Fs>(state: &mut OpState, mask: Option<u32>) -> Result<u32, AnyError>
where
  Fs: FileSystem + 'static,
{
  check_unstable(state, "Deno.umask");
  state.borrow::<Fs>().umask(mask).context("umask")
}

#[op]
fn op_open_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  options: Option<OpenOptions>,
) -> Result<ResourceId, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = options.unwrap_or_else(OpenOptions::read);
  let permissions = state.borrow_mut::<P>();
  options.check(permissions, &path, "Deno.openSync()")?;

  let fs = state.borrow::<Fs>();
  let file = fs.open_sync(&path, options).context_path("open", &path)?;

  let rid = state.resource_table.add(file);
  Ok(rid)
}

#[op]
async fn op_open_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  options: Option<OpenOptions>,
) -> Result<ResourceId, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = options.unwrap_or_else(OpenOptions::read);
  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    options.check(permissions, &path, "Deno.open()")?;
    state.borrow::<Fs>().clone()
  };
  let file = fs
    .open_async(path.clone(), options)
    .await
    .context_path("open", &path)?;

  let rid = state.borrow_mut().resource_table.add(file);
  Ok(rid)
}

#[op]
fn op_mkdir_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  recursive: bool,
  mode: Option<u32>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let mode = mode.unwrap_or(0o777) & 0o777;

  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.mkdirSync()")?;

  let fs = state.borrow::<Fs>();
  fs.mkdir_sync(&path, recursive, mode)
    .context_path("mkdir", &path)?;

  Ok(())
}

#[op]
async fn op_mkdir_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  recursive: bool,
  mode: Option<u32>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let mode = mode.unwrap_or(0o777) & 0o777;

  let fs = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.mkdir()")?;
    state.borrow::<Fs>().clone()
  };

  fs.mkdir_async(path.clone(), recursive, mode)
    .await
    .context_path("mkdir", &path)?;

  Ok(())
}

#[op]
fn op_chmod_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  mode: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.chmodSync()")?;
  let fs = state.borrow::<Fs>();
  fs.chmod_sync(&path, mode).context_path("chmod", &path)?;
  Ok(())
}

#[op]
async fn op_chmod_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  mode: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  let fs = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.chmod()")?;
    state.borrow::<Fs>().clone()
  };
  fs.chmod_async(path.clone(), mode)
    .await
    .context_path("chmod", &path)?;
  Ok(())
}

#[op]
fn op_chown_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.chownSync()")?;
  let fs = state.borrow::<Fs>();
  fs.chown_sync(&path, uid, gid)
    .context_path("chown", &path)?;
  Ok(())
}

#[op]
async fn op_chown_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  uid: Option<u32>,
  gid: Option<u32>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  let fs = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.chown()")?;
    state.borrow::<Fs>().clone()
  };
  fs.chown_async(path.clone(), uid, gid)
    .await
    .context_path("chown", &path)?;
  Ok(())
}

#[op]
fn op_remove_sync<Fs, P>(
  state: &mut OpState,
  path: &str,
  recursive: bool,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.removeSync()")?;

  let fs = state.borrow::<Fs>();
  fs.remove_sync(&path, recursive)
    .context_path("remove", &path)?;

  Ok(())
}

#[op]
async fn op_remove_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  recursive: bool,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_write(&path, "Deno.remove()")?;
    state.borrow::<Fs>().clone()
  };

  fs.remove_async(path.clone(), recursive)
    .await
    .context_path("remove", &path)?;

  Ok(())
}

#[op]
fn op_copy_file_sync<Fs, P>(
  state: &mut OpState,
  from: &str,
  to: &str,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let from = PathBuf::from(from);
  let to = PathBuf::from(to);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&from, "Deno.copyFileSync()")?;
  permissions.check_write(&to, "Deno.copyFileSync()")?;

  let fs = state.borrow::<Fs>();
  fs.copy_file_sync(&from, &to)
    .context_two_path("copy", &from, &to)?;

  Ok(())
}

#[op]
async fn op_copy_file_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  from: String,
  to: String,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let from = PathBuf::from(from);
  let to = PathBuf::from(to);

  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&from, "Deno.copyFile()")?;
    permissions.check_write(&to, "Deno.copyFile()")?;
    state.borrow::<Fs>().clone()
  };

  fs.copy_file_async(from.clone(), to.clone())
    .await
    .context_two_path("copy", &from, &to)?;

  Ok(())
}

#[op]
fn op_stat_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  stat_out_buf: &mut [u32],
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.statSync()")?;
  let fs = state.borrow::<Fs>();
  let stat = fs.stat_sync(&path).context_path("stat", &path)?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op]
async fn op_stat_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<SerializableStat, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.stat()")?;
    state.borrow::<Fs>().clone()
  };
  let stat = fs
    .stat_async(path.clone())
    .await
    .context_path("stat", &path)?;
  Ok(SerializableStat::from(stat))
}

#[op]
fn op_lstat_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  stat_out_buf: &mut [u32],
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.lstatSync()")?;
  let fs = state.borrow::<Fs>();
  let stat = fs.lstat_sync(&path).context_path("lstat", &path)?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op]
async fn op_lstat_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<SerializableStat, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.lstat()")?;
    state.borrow::<Fs>().clone()
  };
  let stat = fs
    .lstat_async(path.clone())
    .await
    .context_path("lstat", &path)?;
  Ok(SerializableStat::from(stat))
}

#[op]
fn op_realpath_sync<Fs, P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = state.borrow::<Fs>().clone();
  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&path, "Deno.realPathSync()")?;
  if path.is_relative() {
    permissions.check_read_blind(&fs.cwd()?, "CWD", "Deno.realPathSync()")?;
  }

  let resolved_path =
    fs.realpath_sync(&path).context_path("realpath", &path)?;

  let path_string = path_into_string(resolved_path.into_os_string())?;
  Ok(path_string)
}

#[op]
async fn op_realpath_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs;
  {
    let mut state = state.borrow_mut();
    fs = state.borrow::<Fs>().clone();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.realPath()")?;
    if path.is_relative() {
      permissions.check_read_blind(&fs.cwd()?, "CWD", "Deno.realPath()")?;
    }
  }
  let resolved_path = fs
    .realpath_async(path.clone())
    .await
    .context_path("realpath", &path)?;

  let path_string = path_into_string(resolved_path.into_os_string())?;
  Ok(path_string)
}

#[op]
fn op_read_dir_sync<Fs, P>(
  state: &mut OpState,
  path: String,
) -> Result<Vec<FsDirEntry>, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readDirSync()")?;

  let fs = state.borrow::<Fs>();
  let entries = fs.read_dir_sync(&path).context_path("readdir", &path)?;

  Ok(entries)
}

#[op]
async fn op_read_dir_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<Vec<FsDirEntry>, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readDir()")?;
    state.borrow::<Fs>().clone()
  };

  let entries = fs
    .read_dir_async(path.clone())
    .await
    .context_path("readdir", &path)?;

  Ok(entries)
}

#[op]
fn op_rename_sync<Fs, P>(
  state: &mut OpState,
  oldpath: String,
  newpath: String,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&oldpath, "Deno.renameSync()")?;
  permissions.check_write(&oldpath, "Deno.renameSync()")?;
  permissions.check_write(&newpath, "Deno.renameSync()")?;

  let fs = state.borrow::<Fs>();
  fs.rename_sync(&oldpath, &newpath)
    .context_two_path("rename", &oldpath, &newpath)?;

  Ok(())
}

#[op]
async fn op_rename_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&oldpath, "Deno.rename()")?;
    permissions.check_write(&oldpath, "Deno.rename()")?;
    permissions.check_write(&newpath, "Deno.rename()")?;
    state.borrow::<Fs>().clone()
  };

  fs.rename_async(oldpath.clone(), newpath.clone())
    .await
    .context_two_path("rename", &oldpath, &newpath)?;

  Ok(())
}

#[op]
fn op_link_sync<Fs, P>(
  state: &mut OpState,
  oldpath: &str,
  newpath: &str,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&oldpath, "Deno.linkSync()")?;
  permissions.check_write(&oldpath, "Deno.linkSync()")?;
  permissions.check_read(&newpath, "Deno.linkSync()")?;
  permissions.check_write(&newpath, "Deno.linkSync()")?;

  let fs = state.borrow::<Fs>();
  fs.link_sync(&oldpath, &newpath)
    .context_two_path("link", &oldpath, &newpath)?;

  Ok(())
}

#[op]
async fn op_link_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);

  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&oldpath, "Deno.link()")?;
    permissions.check_write(&oldpath, "Deno.link()")?;
    permissions.check_read(&newpath, "Deno.link()")?;
    permissions.check_write(&newpath, "Deno.link()")?;
    state.borrow::<Fs>().clone()
  };

  fs.link_async(oldpath.clone(), newpath.clone())
    .await
    .context_two_path("link", &oldpath, &newpath)?;

  Ok(())
}

#[op]
fn op_symlink_sync<Fs, P>(
  state: &mut OpState,
  oldpath: &str,
  newpath: &str,
  file_type: Option<FsFileType>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_write_all("Deno.symlinkSync()")?;
  permissions.check_read_all("Deno.symlinkSync()")?;

  let fs = state.borrow::<Fs>();
  fs.symlink_sync(&oldpath, &newpath, file_type)
    .context_two_path("symlink", &oldpath, &newpath)?;

  Ok(())
}

#[op]
async fn op_symlink_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
  file_type: Option<FsFileType>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);

  let fs = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_write_all("Deno.symlink()")?;
    permissions.check_read_all("Deno.symlink()")?;
    state.borrow::<Fs>().clone()
  };

  fs.symlink_async(oldpath.clone(), newpath.clone(), file_type)
    .await
    .context_two_path("symlink", &oldpath, &newpath)?;

  Ok(())
}

#[op]
fn op_read_link_sync<Fs, P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readLink()")?;

  let fs = state.borrow::<Fs>();

  let target = fs.read_link_sync(&path).context_path("readlink", &path)?;
  let target_string = path_into_string(target.into_os_string())?;
  Ok(target_string)
}

#[op]
async fn op_read_link_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readLink()")?;
    state.borrow::<Fs>().clone()
  };

  let target = fs
    .read_link_async(path.clone())
    .await
    .context_path("readlink", &path)?;
  let target_string = path_into_string(target.into_os_string())?;
  Ok(target_string)
}

#[op]
fn op_truncate_sync<Fs, P>(
  state: &mut OpState,
  path: &str,
  len: u64,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.truncateSync()")?;

  let fs = state.borrow::<Fs>();
  fs.truncate_sync(&path, len)
    .context_path("truncate", &path)?;

  Ok(())
}

#[op]
async fn op_truncate_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  len: u64,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_write(&path, "Deno.truncate()")?;
    state.borrow::<Fs>().clone()
  };

  fs.truncate_async(path.clone(), len)
    .await
    .context_path("truncate", &path)?;

  Ok(())
}

#[op]
fn op_utime_sync<Fs, P>(
  state: &mut OpState,
  path: &str,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state.borrow_mut::<P>().check_write(&path, "Deno.utime()")?;

  let fs = state.borrow::<Fs>();
  fs.utime_sync(&path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .context_path("utime", &path)?;

  Ok(())
}

#[op]
async fn op_utime_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let fs = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.utime()")?;
    state.borrow::<Fs>().clone()
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

#[op]
fn op_make_temp_dir_sync<Fs, P>(
  state: &mut OpState,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_sync::<Fs, P>(state, dir)?;

  let mut rng = thread_rng();

  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs.mkdir_sync(&path, false, 0o700) {
      Ok(_) => return path_into_string(path.into_os_string()),
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

#[op]
async fn op_make_temp_dir_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_async::<Fs, P>(state, dir)?;

  let mut rng = thread_rng();

  const MAX_TRIES: u32 = 10;
  for _ in 0..MAX_TRIES {
    let path = tmp_name(&mut rng, &dir, prefix.as_deref(), suffix.as_deref())?;
    match fs.clone().mkdir_async(path.clone(), false, 0o700).await {
      Ok(_) => return path_into_string(path.into_os_string()),
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

#[op]
fn op_make_temp_file_sync<Fs, P>(
  state: &mut OpState,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_sync::<Fs, P>(state, dir)?;

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
    match fs.open_sync(&path, open_opts) {
      Ok(_) => return path_into_string(path.into_os_string()),
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

#[op]
async fn op_make_temp_file_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let (dir, fs) = make_temp_check_async::<Fs, P>(state, dir)?;

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
    match fs.clone().open_async(path.clone(), open_opts).await {
      Ok(_) => return path_into_string(path.into_os_string()),
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

fn make_temp_check_sync<Fs, P>(
  state: &mut OpState,
  dir: Option<String>,
) -> Result<(PathBuf, Fs), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let fs = state.borrow::<Fs>().clone();
  let dir = match dir {
    Some(dir) => {
      let dir = PathBuf::from(dir);
      state
        .borrow_mut::<P>()
        .check_write(&dir, "Deno.makeTempFile()")?;
      dir
    }
    None => {
      let dir = fs.tmp_dir().context("tmpdir")?;
      state.borrow_mut::<P>().check_write_blind(
        &dir,
        "TMP",
        "Deno.makeTempFile()",
      )?;
      dir
    }
  };
  Ok((dir, fs))
}

fn make_temp_check_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  dir: Option<String>,
) -> Result<(PathBuf, Fs), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let mut state = state.borrow_mut();
  let fs = state.borrow::<Fs>().clone();
  let dir = match dir {
    Some(dir) => {
      let dir = PathBuf::from(dir);
      state
        .borrow_mut::<P>()
        .check_write(&dir, "Deno.makeTempFile()")?;
      dir
    }
    None => {
      let dir = fs.tmp_dir().context("tmpdir")?;
      state.borrow_mut::<P>().check_write_blind(
        &dir,
        "TMP",
        "Deno.makeTempFile()",
      )?;
      dir
    }
  };
  Ok((dir, fs))
}

fn tmp_name(
  rng: &mut ThreadRng,
  dir: &Path,
  prefix: Option<&str>,
  suffix: Option<&str>,
) -> Result<PathBuf, AnyError> {
  let prefix = prefix.unwrap_or("");
  let suffix = suffix.unwrap_or("");

  let mut path = dir.join("_");

  let unique = rng.gen::<u32>();
  path.set_file_name(format!("{prefix}{unique:08x}{suffix}"));

  Ok(path)
}

#[op]
fn op_write_file_sync<Fs, P>(
  state: &mut OpState,
  path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  create_new: bool,
  data: ZeroCopyBuf,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let permissions = state.borrow_mut::<P>();
  let options = OpenOptions::write(create, append, create_new, mode);
  options.check(permissions, &path, "Deno.writeFileSync()")?;

  let fs = state.borrow::<Fs>();

  fs.write_file_sync(&path, options, &data)
    .context_path("writefile", &path)?;

  Ok(())
}

#[op]
async fn op_write_file_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  create_new: bool,
  data: ZeroCopyBuf,
  cancel_rid: Option<ResourceId>,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let options = OpenOptions::write(create, append, create_new, mode);

  let (fs, cancel_handle) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    options.check(permissions, &path, "Deno.writeFile()")?;
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<Fs>().clone(), cancel_handle)
  };

  let fut = fs.write_file_async(path.clone(), options, data.to_vec());

  if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    res?.context_path("writefile", &path)?;
  } else {
    fut.await.context_path("writefile", &path)?;
  }

  Ok(())
}

#[op]
fn op_read_file_sync<Fs, P>(
  state: &mut OpState,
  path: String,
) -> Result<ZeroCopyBuf, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&path, "Deno.readFileSync()")?;

  let fs = state.borrow::<Fs>();
  let buf = fs.read_file_sync(path).context("readfile")?;

  Ok(buf.into())
}

#[op]
async fn op_read_file_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  cancel_rid: Option<ResourceId>,
) -> Result<ZeroCopyBuf, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let (fs, cancel_handle) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.readFile()")?;
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<Fs>().clone(), cancel_handle)
  };

  let fut = fs.read_file_async(path.clone());

  let buf = if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    res?.context_path("readfile", &path)?
  } else {
    fut.await.context_path("readfile", &path)?
  };

  Ok(buf.into())
}

#[op]
fn op_read_file_text_sync<Fs, P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&path, "Deno.readFileSync()")?;

  let fs = state.borrow::<Fs>();
  let buf = fs.read_file_sync(path).context("readfile")?;

  Ok(string_from_utf8_lossy(buf))
}

#[op]
async fn op_read_file_text_async<Fs, P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  cancel_rid: Option<ResourceId>,
) -> Result<String, AnyError>
where
  Fs: FileSystem + 'static,
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  let (fs, cancel_handle) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.readFile()")?;
    let cancel_handle = cancel_rid
      .and_then(|rid| state.resource_table.get::<CancelHandle>(rid).ok());
    (state.borrow::<Fs>().clone(), cancel_handle)
  };

  let fut = fs.read_file_async(path.clone());

  let buf = if let Some(cancel_handle) = cancel_handle {
    let res = fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    res?.context_path("readfile", &path)?
  } else {
    fut.await.context_path("readfile", &path)?
  };

  Ok(string_from_utf8_lossy(buf))
}

// Like String::from_utf8_lossy but operates on owned values
fn string_from_utf8_lossy(buf: Vec<u8>) -> String {
  match String::from_utf8_lossy(&buf) {
    // buf contained non-utf8 chars than have been patched
    Cow::Owned(s) => s,
    // SAFETY: if Borrowed then the buf only contains utf8 chars,
    // we do this instead of .into_owned() to avoid copying the input buf
    Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(buf) },
  }
}

fn to_seek_from(offset: i64, whence: i32) -> Result<SeekFrom, AnyError> {
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(offset),
    2 => SeekFrom::End(offset),
    _ => {
      return Err(type_error(format!("Invalid seek mode: {whence}")));
    }
  };
  Ok(seek_from)
}

#[op]
fn op_seek_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
  offset: i64,
  whence: i32,
) -> Result<u64, AnyError>
where
  Fs: FileSystem + 'static,
{
  let pos = to_seek_from(offset, whence)?;
  let file = state.resource_table.get::<Fs::File>(rid)?;
  let cursor = file.seek_sync(pos)?;
  Ok(cursor)
}

#[op]
async fn op_seek_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  offset: i64,
  whence: i32,
) -> Result<u64, AnyError>
where
  Fs: FileSystem + 'static,
{
  let pos = to_seek_from(offset, whence)?;
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  let cursor = file.seek_async(pos).await?;
  Ok(cursor)
}

#[op]
fn op_fdatasync_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.datasync_sync()?;
  Ok(())
}

#[op]
async fn op_fdatasync_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file.datasync_async().await?;
  Ok(())
}

#[op]
fn op_fsync_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.sync_sync()?;
  Ok(())
}

#[op]
async fn op_fsync_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file.sync_async().await?;
  Ok(())
}

#[op]
fn op_fstat_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
  stat_out_buf: &mut [u32],
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.resource_table.get::<Fs::File>(rid)?;
  let stat = file.stat_sync()?;
  let serializable_stat = SerializableStat::from(stat);
  serializable_stat.write(stat_out_buf);
  Ok(())
}

#[op]
async fn op_fstat_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<SerializableStat, AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  let stat = file.stat_async().await?;
  Ok(stat.into())
}

#[op]
fn op_flock_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  check_unstable(state, "Deno.flockSync");
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.lock_sync(exclusive)?;
  Ok(())
}

#[op]
async fn op_flock_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  check_unstable2(&state, "Deno.flock");
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file.lock_async(exclusive).await?;
  Ok(())
}

#[op]
fn op_funlock_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  check_unstable(state, "Deno.funlockSync");
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.unlock_sync()?;
  Ok(())
}

#[op]
async fn op_funlock_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  check_unstable2(&state, "Deno.funlock");
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file.unlock_async().await?;
  Ok(())
}

#[op]
fn op_ftruncate_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
  len: u64,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.truncate_sync(len)?;
  Ok(())
}

#[op]
async fn op_ftruncate_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  len: u64,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file.truncate_async(len).await?;
  Ok(())
}

#[op]
fn op_futime_sync<Fs>(
  state: &mut OpState,
  rid: ResourceId,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.resource_table.get::<Fs::File>(rid)?;
  file.utime_sync(atime_secs, atime_nanos, mtime_secs, mtime_nanos)?;
  Ok(())
}

#[op]
async fn op_futime_async<Fs>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  Fs: FileSystem + 'static,
{
  let file = state.borrow().resource_table.get::<Fs::File>(rid)?;
  file
    .utime_async(atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .await?;
  Ok(())
}

trait WithContext {
  fn context<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
    self,
    desc: E,
  ) -> AnyError;
}

impl WithContext for FsError {
  fn context<E: Into<Box<dyn std::error::Error + Send + Sync>>>(
    self,
    desc: E,
  ) -> AnyError {
    match self {
      FsError::Io(io) => {
        AnyError::new(io::Error::new(io.kind(), desc)).context(io)
      }
      _ => self.into(),
    }
  }
}

trait MapErrContext {
  type R;

  fn context_fn<F, E>(self, f: F) -> Self::R
  where
    F: FnOnce() -> E,
    E: Into<Box<dyn std::error::Error + Send + Sync>>;

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
  type R = Result<T, AnyError>;

  fn context_fn<F, E>(self, f: F) -> Self::R
  where
    F: FnOnce() -> E,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
  {
    self.map_err(|err| {
      let message = f();
      err.context(message)
    })
  }

  fn context(self, desc: &'static str) -> Self::R {
    self.context_fn(move || desc)
  }

  fn context_path(self, operation: &'static str, path: &Path) -> Self::R {
    self.context_fn(|| format!("{operation} '{}'", path.display()))
  }

  fn context_two_path(
    self,
    operation: &'static str,
    oldpath: &Path,
    newpath: &Path,
  ) -> Self::R {
    self.context_fn(|| {
      format!(
        "{operation} '{}' -> '{}'",
        oldpath.display(),
        newpath.display()
      )
    })
  }
}

fn path_into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {s:?} is not valid UTF-8");
    custom_error("InvalidData", message)
  })
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
    struct $name {
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

      dev: stat.dev,
      ino: stat.ino,
      mode: stat.mode,
      nlink: stat.nlink,
      uid: stat.uid,
      gid: stat.gid,
      rdev: stat.rdev,
      blksize: stat.blksize,
      blocks: stat.blocks,
    }
  }
}
