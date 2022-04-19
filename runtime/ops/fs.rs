// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::io::StdFileResource;
use super::utils::into_string;
use crate::fs_util::canonicalize_path;
use crate::permissions::Permissions;
use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::ZeroCopyBuf;

use deno_core::Extension;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_crypto::rand::thread_rng;
use deno_crypto::rand::Rng;
use log::debug;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::convert::From;
use std::env::{current_dir, set_current_dir, temp_dir};
use std::io;
use std::io::Write;
use std::io::{Error, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[cfg(not(unix))]
use deno_core::error::generic_error;
#[cfg(not(unix))]
use deno_core::error::not_supported;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_open_sync::decl(),
      op_open_async::decl(),
      op_write_file_sync::decl(),
      op_write_file_async::decl(),
      op_seek_sync::decl(),
      op_seek_async::decl(),
      op_fdatasync_sync::decl(),
      op_fdatasync_async::decl(),
      op_fsync_sync::decl(),
      op_fsync_async::decl(),
      op_fstat_sync::decl(),
      op_fstat_async::decl(),
      op_flock_sync::decl(),
      op_flock_async::decl(),
      op_funlock_sync::decl(),
      op_funlock_async::decl(),
      op_umask::decl(),
      op_chdir::decl(),
      op_mkdir_sync::decl(),
      op_mkdir_async::decl(),
      op_chmod_sync::decl(),
      op_chmod_async::decl(),
      op_chown_sync::decl(),
      op_chown_async::decl(),
      op_remove_sync::decl(),
      op_remove_async::decl(),
      op_copy_file_sync::decl(),
      op_copy_file_async::decl(),
      op_stat_sync::decl(),
      op_stat_async::decl(),
      op_realpath_sync::decl(),
      op_realpath_async::decl(),
      op_read_dir_sync::decl(),
      op_read_dir_async::decl(),
      op_rename_sync::decl(),
      op_rename_async::decl(),
      op_link_sync::decl(),
      op_link_async::decl(),
      op_symlink_sync::decl(),
      op_symlink_async::decl(),
      op_read_link_sync::decl(),
      op_read_link_async::decl(),
      op_ftruncate_sync::decl(),
      op_ftruncate_async::decl(),
      op_truncate_sync::decl(),
      op_truncate_async::decl(),
      op_make_temp_dir_sync::decl(),
      op_make_temp_dir_async::decl(),
      op_make_temp_file_sync::decl(),
      op_make_temp_file_async::decl(),
      op_cwd::decl(),
      op_futime_sync::decl(),
      op_futime_async::decl(),
      op_utime_sync::decl(),
      op_utime_async::decl(),
    ])
    .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
  path: String,
  mode: Option<u32>,
  options: OpenOptions,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct OpenOptions {
  read: bool,
  write: bool,
  create: bool,
  truncate: bool,
  append: bool,
  create_new: bool,
}

fn open_helper(
  state: &mut OpState,
  args: &OpenArgs,
) -> Result<(PathBuf, std::fs::OpenOptions), AnyError> {
  let path = Path::new(&args.path).to_path_buf();

  let mut open_options = std::fs::OpenOptions::new();

  if let Some(mode) = args.mode {
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
  let options = &args.options;

  if options.read {
    permissions.read.check(&path)?;
  }

  if options.write || options.append {
    permissions.write.check(&path)?;
  }

  open_options
    .read(options.read)
    .create(options.create)
    .write(options.write)
    .truncate(options.truncate)
    .append(options.append)
    .create_new(options.create_new);

  Ok((path, open_options))
}

#[op]
fn op_open_sync(
  state: &mut OpState,
  args: OpenArgs,
) -> Result<ResourceId, AnyError> {
  let (path, open_options) = open_helper(state, &args)?;
  let std_file = open_options.open(&path).map_err(|err| {
    Error::new(err.kind(), format!("{}, open '{}'", err, path.display()))
  })?;
  let resource = StdFileResource::fs_file(std_file);
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[op]
async fn op_open_async(
  state: Rc<RefCell<OpState>>,
  args: OpenArgs,
) -> Result<ResourceId, AnyError> {
  let (path, open_options) = open_helper(&mut state.borrow_mut(), &args)?;
  let std_file = tokio::task::spawn_blocking(move || {
    open_options.open(path.clone()).map_err(|err| {
      Error::new(err.kind(), format!("{}, open '{}'", err, path.display()))
    })
  })
  .await?;
  let resource = StdFileResource::fs_file(std_file?);
  let rid = state.borrow_mut().resource_table.add(resource);
  Ok(rid)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileArgs {
  path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  data: ZeroCopyBuf,
  cancel_rid: Option<ResourceId>,
}

impl WriteFileArgs {
  fn into_open_args_and_data(self) -> (OpenArgs, ZeroCopyBuf) {
    (
      OpenArgs {
        path: self.path,
        mode: self.mode,
        options: OpenOptions {
          read: false,
          write: true,
          create: self.create,
          truncate: !self.append,
          append: self.append,
          create_new: false,
        },
      },
      self.data,
    )
  }
}

#[op]
fn op_write_file_sync(
  state: &mut OpState,
  args: WriteFileArgs,
) -> Result<(), AnyError> {
  let (open_args, data) = args.into_open_args_and_data();
  let (path, open_options) = open_helper(state, &open_args)?;
  write_file(&path, open_options, &open_args, data)
}

#[op]
async fn op_write_file_async(
  state: Rc<RefCell<OpState>>,
  args: WriteFileArgs,
) -> Result<(), AnyError> {
  let cancel_handle = match args.cancel_rid {
    Some(cancel_rid) => state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(cancel_rid)
      .ok(),
    None => None,
  };
  let (open_args, data) = args.into_open_args_and_data();
  let (path, open_options) = open_helper(&mut *state.borrow_mut(), &open_args)?;
  let write_future = tokio::task::spawn_blocking(move || {
    write_file(&path, open_options, &open_args, data)
  });
  if let Some(cancel_handle) = cancel_handle {
    write_future.or_cancel(cancel_handle).await???;
  } else {
    write_future.await??;
  }
  Ok(())
}

fn write_file(
  path: &Path,
  open_options: std::fs::OpenOptions,
  _open_args: &OpenArgs,
  data: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let mut std_file = open_options.open(path).map_err(|err| {
    Error::new(err.kind(), format!("{}, open '{}'", err, path.display()))
  })?;

  // need to chmod the file if it already exists and a mode is specified
  #[cfg(unix)]
  if let Some(mode) = &_open_args.mode {
    use std::os::unix::fs::PermissionsExt;
    let permissions = PermissionsExt::from_mode(mode & 0o777);
    std_file
      .set_permissions(permissions)
      .map_err(|err: Error| {
        Error::new(err.kind(), format!("{}, chmod '{}'", err, path.display()))
      })?;
  }

  std_file.write_all(&data)?;
  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeekArgs {
  rid: ResourceId,
  offset: i64,
  whence: i32,
}

fn seek_helper(args: SeekArgs) -> Result<(u32, SeekFrom), AnyError> {
  let rid = args.rid;
  let offset = args.offset;
  let whence = args.whence as u32;
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(offset),
    2 => SeekFrom::End(offset),
    _ => {
      return Err(type_error(format!("Invalid seek mode: {}", whence)));
    }
  };

  Ok((rid, seek_from))
}

#[op]
fn op_seek_sync(state: &mut OpState, args: SeekArgs) -> Result<u64, AnyError> {
  let (rid, seek_from) = seek_helper(args)?;
  let pos = StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => std_file.seek(seek_from).map_err(AnyError::from),
    Err(_) => Err(type_error(
      "cannot seek on this type of resource".to_string(),
    )),
  })?;
  Ok(pos)
}

#[op]
async fn op_seek_async(
  state: Rc<RefCell<OpState>>,
  args: SeekArgs,
) -> Result<u64, AnyError> {
  let (rid, seek_from) = seek_helper(args)?;

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || {
    let mut std_file = std_file.lock().unwrap();
    std_file.seek(seek_from)
  })
  .await?
  .map_err(AnyError::from)
}

#[op]
fn op_fdatasync_sync(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => std_file.sync_data().map_err(AnyError::from),
    Err(_) => Err(type_error("cannot sync this type of resource".to_string())),
  })?;
  Ok(())
}

#[op]
async fn op_fdatasync_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || {
    let std_file = std_file.lock().unwrap();
    std_file.sync_data()
  })
  .await?
  .map_err(AnyError::from)
}

#[op]
fn op_fsync_sync(state: &mut OpState, rid: ResourceId) -> Result<(), AnyError> {
  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => std_file.sync_all().map_err(AnyError::from),
    Err(_) => Err(type_error("cannot sync this type of resource".to_string())),
  })?;
  Ok(())
}

#[op]
async fn op_fsync_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || {
    let std_file = std_file.lock().unwrap();
    std_file.sync_all()
  })
  .await?
  .map_err(AnyError::from)
}

#[op]
fn op_fstat_sync(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<FsStat, AnyError> {
  let metadata = StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => std_file.metadata().map_err(AnyError::from),
    Err(_) => Err(type_error("cannot stat this type of resource".to_string())),
  })?;
  Ok(get_stat(metadata))
}

#[op]
async fn op_fstat_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<FsStat, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  let metadata = tokio::task::spawn_blocking(move || {
    let std_file = std_file.lock().unwrap();
    std_file.metadata()
  })
  .await?
  .map_err(AnyError::from)?;
  Ok(get_stat(metadata))
}

#[op]
fn op_flock_sync(
  state: &mut OpState,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  super::check_unstable(state, "Deno.flockSync");

  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => {
      if exclusive {
        std_file.lock_exclusive()?;
      } else {
        std_file.lock_shared()?;
      }
      Ok(())
    }
    Err(_) => Err(type_error("cannot lock this type of resource".to_string())),
  })
}

#[op]
async fn op_flock_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  super::check_unstable2(&state, "Deno.flock");

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || -> Result<(), AnyError> {
    let std_file = std_file.lock().unwrap();
    if exclusive {
      std_file.lock_exclusive()?;
    } else {
      std_file.lock_shared()?;
    }
    Ok(())
  })
  .await?
}

#[op]
fn op_funlock_sync(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  super::check_unstable(state, "Deno.funlockSync");

  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => {
      std_file.unlock()?;
      Ok(())
    }
    Err(_) => Err(type_error("cannot lock this type of resource".to_string())),
  })
}

#[op]
async fn op_funlock_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  super::check_unstable2(&state, "Deno.funlock");

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || -> Result<(), AnyError> {
    let std_file = std_file.lock().unwrap();
    std_file.unlock()?;
    Ok(())
  })
  .await?
}

#[op]
fn op_umask(state: &mut OpState, mask: Option<u32>) -> Result<u32, AnyError> {
  super::check_unstable(state, "Deno.umask");
  // TODO implement umask for Windows
  // see https://github.com/nodejs/node/blob/master/src/node_process_methods.cc
  // and https://docs.microsoft.com/fr-fr/cpp/c-runtime-library/reference/umask?view=vs-2019
  #[cfg(not(unix))]
  {
    let _ = mask; // avoid unused warning.
    Err(not_supported())
  }
  #[cfg(unix)]
  {
    use nix::sys::stat::mode_t;
    use nix::sys::stat::umask;
    use nix::sys::stat::Mode;
    let r = if let Some(mask) = mask {
      // If mask provided, return previous.
      umask(Mode::from_bits_truncate(mask as mode_t))
    } else {
      // If no mask provided, we query the current. Requires two syscalls.
      let prev = umask(Mode::from_bits_truncate(0o777));
      let _ = umask(prev);
      prev
    };
    Ok(r.bits() as u32)
  }
}

#[op]
fn op_chdir(state: &mut OpState, directory: String) -> Result<(), AnyError> {
  let d = PathBuf::from(&directory);
  state.borrow_mut::<Permissions>().read.check(&d)?;
  set_current_dir(&d).map_err(|err| {
    Error::new(err.kind(), format!("{}, chdir '{}'", err, directory))
  })?;
  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MkdirArgs {
  path: String,
  recursive: bool,
  mode: Option<u32>,
}

#[op]
fn op_mkdir_sync(state: &mut OpState, args: MkdirArgs) -> Result<(), AnyError> {
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode.unwrap_or(0o777) & 0o777;
  state.borrow_mut::<Permissions>().write.check(&path)?;
  debug!("op_mkdir {} {:o} {}", path.display(), mode, args.recursive);
  let mut builder = std::fs::DirBuilder::new();
  builder.recursive(args.recursive);
  #[cfg(unix)]
  {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(mode);
  }
  builder.create(&path).map_err(|err| {
    Error::new(err.kind(), format!("{}, mkdir '{}'", err, path.display()))
  })?;
  Ok(())
}

#[op]
async fn op_mkdir_async(
  state: Rc<RefCell<OpState>>,
  args: MkdirArgs,
) -> Result<(), AnyError> {
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode.unwrap_or(0o777) & 0o777;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check(&path)?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_mkdir {} {:o} {}", path.display(), mode, args.recursive);
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(args.recursive);
    #[cfg(unix)]
    {
      use std::os::unix::fs::DirBuilderExt;
      builder.mode(mode);
    }
    builder.create(&path).map_err(|err| {
      Error::new(err.kind(), format!("{}, mkdir '{}'", err, path.display()))
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChmodArgs {
  path: String,
  mode: u32,
}

#[op]
fn op_chmod_sync(state: &mut OpState, args: ChmodArgs) -> Result<(), AnyError> {
  let path = Path::new(&args.path);
  let mode = args.mode & 0o777;

  state.borrow_mut::<Permissions>().write.check(path)?;
  debug!("op_chmod_sync {} {:o}", path.display(), mode);
  raw_chmod(path, mode)
}

#[op]
async fn op_chmod_async(
  state: Rc<RefCell<OpState>>,
  args: ChmodArgs,
) -> Result<(), AnyError> {
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode & 0o777;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check(&path)?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_chmod_async {} {:o}", path.display(), mode);
    raw_chmod(&path, mode)
  })
  .await
  .unwrap()
}

fn raw_chmod(path: &Path, _raw_mode: u32) -> Result<(), AnyError> {
  let err_mapper = |err: Error| {
    Error::new(err.kind(), format!("{}, chmod '{}'", err, path.display()))
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let permissions = PermissionsExt::from_mode(_raw_mode);
    std::fs::set_permissions(&path, permissions).map_err(err_mapper)?;
    Ok(())
  }
  // TODO Implement chmod for Windows (#4357)
  #[cfg(not(unix))]
  {
    // Still check file/dir exists on Windows
    let _metadata = std::fs::metadata(&path).map_err(err_mapper)?;
    Err(not_supported())
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChownArgs {
  path: String,
  uid: Option<u32>,
  gid: Option<u32>,
}

#[op]
fn op_chown_sync(state: &mut OpState, args: ChownArgs) -> Result<(), AnyError> {
  let path = Path::new(&args.path).to_path_buf();
  state.borrow_mut::<Permissions>().write.check(&path)?;
  debug!(
    "op_chown_sync {} {:?} {:?}",
    path.display(),
    args.uid,
    args.gid,
  );
  #[cfg(unix)]
  {
    use crate::errors::get_nix_error_class;
    use nix::unistd::{chown, Gid, Uid};
    let nix_uid = args.uid.map(Uid::from_raw);
    let nix_gid = args.gid.map(Gid::from_raw);
    chown(&path, nix_uid, nix_gid).map_err(|err| {
      custom_error(
        get_nix_error_class(&err),
        format!("{}, chown '{}'", err.desc(), path.display()),
      )
    })?;
    Ok(())
  }
  // TODO Implement chown for Windows
  #[cfg(not(unix))]
  {
    Err(generic_error("Not implemented"))
  }
}

#[op]
async fn op_chown_async(
  state: Rc<RefCell<OpState>>,
  args: ChownArgs,
) -> Result<(), AnyError> {
  let path = Path::new(&args.path).to_path_buf();

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check(&path)?;
  }

  tokio::task::spawn_blocking(move || {
    debug!(
      "op_chown_async {} {:?} {:?}",
      path.display(),
      args.uid,
      args.gid,
    );
    #[cfg(unix)]
    {
      use crate::errors::get_nix_error_class;
      use nix::unistd::{chown, Gid, Uid};
      let nix_uid = args.uid.map(Uid::from_raw);
      let nix_gid = args.gid.map(Gid::from_raw);
      chown(&path, nix_uid, nix_gid).map_err(|err| {
        custom_error(
          get_nix_error_class(&err),
          format!("{}, chown '{}'", err.desc(), path.display()),
        )
      })?;
      Ok(())
    }
    // TODO Implement chown for Windows
    #[cfg(not(unix))]
    Err(not_supported())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveArgs {
  path: String,
  recursive: bool,
}

#[op]
fn op_remove_sync(
  state: &mut OpState,
  args: RemoveArgs,
) -> Result<(), AnyError> {
  let path = PathBuf::from(&args.path);
  let recursive = args.recursive;

  state.borrow_mut::<Permissions>().write.check(&path)?;

  #[cfg(not(unix))]
  use std::os::windows::prelude::MetadataExt;

  let err_mapper = |err: Error| {
    Error::new(err.kind(), format!("{}, remove '{}'", err, path.display()))
  };
  let metadata = std::fs::symlink_metadata(&path).map_err(err_mapper)?;

  debug!("op_remove_sync {} {}", path.display(), recursive);
  let file_type = metadata.file_type();
  if file_type.is_file() {
    std::fs::remove_file(&path).map_err(err_mapper)?;
  } else if recursive {
    std::fs::remove_dir_all(&path).map_err(err_mapper)?;
  } else if file_type.is_symlink() {
    #[cfg(unix)]
    std::fs::remove_file(&path).map_err(err_mapper)?;
    #[cfg(not(unix))]
    {
      use winapi::um::winnt::FILE_ATTRIBUTE_DIRECTORY;
      if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
        std::fs::remove_dir(&path).map_err(err_mapper)?;
      } else {
        std::fs::remove_file(&path).map_err(err_mapper)?;
      }
    }
  } else if file_type.is_dir() {
    std::fs::remove_dir(&path).map_err(err_mapper)?;
  } else {
    // pipes, sockets, etc...
    std::fs::remove_file(&path).map_err(err_mapper)?;
  }
  Ok(())
}

#[op]
async fn op_remove_async(
  state: Rc<RefCell<OpState>>,
  args: RemoveArgs,
) -> Result<(), AnyError> {
  let path = PathBuf::from(&args.path);
  let recursive = args.recursive;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check(&path)?;
  }

  tokio::task::spawn_blocking(move || {
    #[cfg(not(unix))]
    use std::os::windows::prelude::MetadataExt;
    let err_mapper = |err: Error| {
      Error::new(err.kind(), format!("{}, remove '{}'", err, path.display()))
    };
    let metadata = std::fs::symlink_metadata(&path).map_err(err_mapper)?;

    debug!("op_remove_async {} {}", path.display(), recursive);
    let file_type = metadata.file_type();
    if file_type.is_file() {
      std::fs::remove_file(&path).map_err(err_mapper)?;
    } else if recursive {
      std::fs::remove_dir_all(&path).map_err(err_mapper)?;
    } else if file_type.is_symlink() {
      #[cfg(unix)]
      std::fs::remove_file(&path).map_err(err_mapper)?;
      #[cfg(not(unix))]
      {
        use winapi::um::winnt::FILE_ATTRIBUTE_DIRECTORY;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
          std::fs::remove_dir(&path).map_err(err_mapper)?;
        } else {
          std::fs::remove_file(&path).map_err(err_mapper)?;
        }
      }
    } else if file_type.is_dir() {
      std::fs::remove_dir(&path).map_err(err_mapper)?;
    } else {
      // pipes, sockets, etc...
      std::fs::remove_file(&path).map_err(err_mapper)?;
    }
    Ok(())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyFileArgs {
  from: String,
  to: String,
}

#[op]
fn op_copy_file_sync(
  state: &mut OpState,
  args: CopyFileArgs,
) -> Result<(), AnyError> {
  let from = PathBuf::from(&args.from);
  let to = PathBuf::from(&args.to);

  let permissions = state.borrow_mut::<Permissions>();
  permissions.read.check(&from)?;
  permissions.write.check(&to)?;

  debug!("op_copy_file_sync {} {}", from.display(), to.display());
  // On *nix, Rust reports non-existent `from` as ErrorKind::InvalidInput
  // See https://github.com/rust-lang/rust/issues/54800
  // Once the issue is resolved, we should remove this workaround.
  if cfg!(unix) && !from.is_file() {
    return Err(custom_error(
      "NotFound",
      format!(
        "File not found, copy '{}' -> '{}'",
        from.display(),
        to.display()
      ),
    ));
  }

  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!("{}, copy '{}' -> '{}'", err, from.display(), to.display()),
    )
  };
  // returns size of from as u64 (we ignore)
  std::fs::copy(&from, &to).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_copy_file_async(
  state: Rc<RefCell<OpState>>,
  args: CopyFileArgs,
) -> Result<(), AnyError> {
  let from = PathBuf::from(&args.from);
  let to = PathBuf::from(&args.to);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<Permissions>();
    permissions.read.check(&from)?;
    permissions.write.check(&to)?;
  }

  debug!("op_copy_file_async {} {}", from.display(), to.display());
  tokio::task::spawn_blocking(move || {
    // On *nix, Rust reports non-existent `from` as ErrorKind::InvalidInput
    // See https://github.com/rust-lang/rust/issues/54800
    // Once the issue is resolved, we should remove this workaround.
    if cfg!(unix) && !from.is_file() {
      return Err(custom_error(
        "NotFound",
        format!(
          "File not found, copy '{}' -> '{}'",
          from.display(),
          to.display()
        ),
      ));
    }

    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!("{}, copy '{}' -> '{}'", err, from.display(), to.display()),
      )
    };
    // returns size of from as u64 (we ignore)
    std::fs::copy(&from, &to).map_err(err_mapper)?;
    Ok(())
  })
  .await
  .unwrap()
}

fn to_msec(maybe_time: Result<SystemTime, io::Error>) -> Option<u64> {
  match maybe_time {
    Ok(time) => {
      let msec = time
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as u64)
        .unwrap_or_else(|err| err.duration().as_millis() as u64);
      Some(msec)
    }
    Err(_) => None,
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsStat {
  is_file: bool,
  is_directory: bool,
  is_symlink: bool,
  size: u64,
  // In milliseconds, like JavaScript. Available on both Unix or Windows.
  mtime: Option<u64>,
  atime: Option<u64>,
  birthtime: Option<u64>,
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

#[inline(always)]
fn get_stat(metadata: std::fs::Metadata) -> FsStat {
  // Unix stat member (number types only). 0 if not on unix.
  macro_rules! usm {
    ($member:ident) => {{
      #[cfg(unix)]
      {
        metadata.$member()
      }
      #[cfg(not(unix))]
      {
        0
      }
    }};
  }

  #[cfg(unix)]
  use std::os::unix::fs::MetadataExt;
  FsStat {
    is_file: metadata.is_file(),
    is_directory: metadata.is_dir(),
    is_symlink: metadata.file_type().is_symlink(),
    size: metadata.len(),
    // In milliseconds, like JavaScript. Available on both Unix or Windows.
    mtime: to_msec(metadata.modified()),
    atime: to_msec(metadata.accessed()),
    birthtime: to_msec(metadata.created()),
    // Following are only valid under Unix.
    dev: usm!(dev),
    ino: usm!(ino),
    mode: usm!(mode),
    nlink: usm!(nlink),
    uid: usm!(uid),
    gid: usm!(gid),
    rdev: usm!(rdev),
    blksize: usm!(blksize),
    blocks: usm!(blocks),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatArgs {
  path: String,
  lstat: bool,
}

#[op]
fn op_stat_sync(
  state: &mut OpState,
  args: StatArgs,
) -> Result<FsStat, AnyError> {
  let path = PathBuf::from(&args.path);
  let lstat = args.lstat;
  state.borrow_mut::<Permissions>().read.check(&path)?;
  debug!("op_stat_sync {} {}", path.display(), lstat);
  let err_mapper = |err: Error| {
    Error::new(err.kind(), format!("{}, stat '{}'", err, path.display()))
  };
  let metadata = if lstat {
    std::fs::symlink_metadata(&path).map_err(err_mapper)?
  } else {
    std::fs::metadata(&path).map_err(err_mapper)?
  };
  Ok(get_stat(metadata))
}

#[op]
async fn op_stat_async(
  state: Rc<RefCell<OpState>>,
  args: StatArgs,
) -> Result<FsStat, AnyError> {
  let path = PathBuf::from(&args.path);
  let lstat = args.lstat;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().read.check(&path)?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_stat_async {} {}", path.display(), lstat);
    let err_mapper = |err: Error| {
      Error::new(err.kind(), format!("{}, stat '{}'", err, path.display()))
    };
    let metadata = if lstat {
      std::fs::symlink_metadata(&path).map_err(err_mapper)?
    } else {
      std::fs::metadata(&path).map_err(err_mapper)?
    };
    Ok(get_stat(metadata))
  })
  .await
  .unwrap()
}

#[op]
fn op_realpath_sync(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError> {
  let path = PathBuf::from(&path);

  let permissions = state.borrow_mut::<Permissions>();
  permissions.read.check(&path)?;
  if path.is_relative() {
    permissions.read.check_blind(&current_dir()?, "CWD")?;
  }

  debug!("op_realpath_sync {}", path.display());
  // corresponds to the realpath on Unix and
  // CreateFile and GetFinalPathNameByHandle on Windows
  let realpath = canonicalize_path(&path)?;
  let realpath_str = into_string(realpath.into_os_string())?;
  Ok(realpath_str)
}

#[op]
async fn op_realpath_async(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError> {
  let path = PathBuf::from(&path);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<Permissions>();
    permissions.read.check(&path)?;
    if path.is_relative() {
      permissions.read.check_blind(&current_dir()?, "CWD")?;
    }
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_realpath_async {}", path.display());
    // corresponds to the realpath on Unix and
    // CreateFile and GetFinalPathNameByHandle on Windows
    let realpath = canonicalize_path(&path)?;
    let realpath_str = into_string(realpath.into_os_string())?;
    Ok(realpath_str)
  })
  .await
  .unwrap()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
  name: String,
  is_file: bool,
  is_directory: bool,
  is_symlink: bool,
}

#[op]
fn op_read_dir_sync(
  state: &mut OpState,
  path: String,
) -> Result<Vec<DirEntry>, AnyError> {
  let path = PathBuf::from(&path);

  state.borrow_mut::<Permissions>().read.check(&path)?;

  debug!("op_read_dir_sync {}", path.display());
  let err_mapper = |err: Error| {
    Error::new(err.kind(), format!("{}, readdir '{}'", err, path.display()))
  };
  let entries: Vec<_> = std::fs::read_dir(&path)
    .map_err(err_mapper)?
    .filter_map(|entry| {
      let entry = entry.unwrap();
      // Not all filenames can be encoded as UTF-8. Skip those for now.
      if let Ok(name) = into_string(entry.file_name()) {
        Some(DirEntry {
          name,
          is_file: entry
            .file_type()
            .map_or(false, |file_type| file_type.is_file()),
          is_directory: entry
            .file_type()
            .map_or(false, |file_type| file_type.is_dir()),
          is_symlink: entry
            .file_type()
            .map_or(false, |file_type| file_type.is_symlink()),
        })
      } else {
        None
      }
    })
    .collect();

  Ok(entries)
}

#[op]
async fn op_read_dir_async(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<Vec<DirEntry>, AnyError> {
  let path = PathBuf::from(&path);
  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().read.check(&path)?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_read_dir_async {}", path.display());
    let err_mapper = |err: Error| {
      Error::new(err.kind(), format!("{}, readdir '{}'", err, path.display()))
    };
    let entries: Vec<_> = std::fs::read_dir(&path)
      .map_err(err_mapper)?
      .filter_map(|entry| {
        let entry = entry.unwrap();
        // Not all filenames can be encoded as UTF-8. Skip those for now.
        if let Ok(name) = into_string(entry.file_name()) {
          Some(DirEntry {
            name,
            is_file: entry
              .file_type()
              .map_or(false, |file_type| file_type.is_file()),
            is_directory: entry
              .file_type()
              .map_or(false, |file_type| file_type.is_dir()),
            is_symlink: entry
              .file_type()
              .map_or(false, |file_type| file_type.is_symlink()),
          })
        } else {
          None
        }
      })
      .collect();

    Ok(entries)
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameArgs {
  oldpath: String,
  newpath: String,
}

#[op]
fn op_rename_sync(
  state: &mut OpState,
  args: RenameArgs,
) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  let permissions = state.borrow_mut::<Permissions>();
  permissions.read.check(&oldpath)?;
  permissions.write.check(&oldpath)?;
  permissions.write.check(&newpath)?;
  debug!("op_rename_sync {} {}", oldpath.display(), newpath.display());
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, rename '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  std::fs::rename(&oldpath, &newpath).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_rename_async(
  state: Rc<RefCell<OpState>>,
  args: RenameArgs,
) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<Permissions>();
    permissions.read.check(&oldpath)?;
    permissions.write.check(&oldpath)?;
    permissions.write.check(&newpath)?;
  }
  tokio::task::spawn_blocking(move || {
    debug!(
      "op_rename_async {} {}",
      oldpath.display(),
      newpath.display()
    );
    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!(
          "{}, rename '{}' -> '{}'",
          err,
          oldpath.display(),
          newpath.display()
        ),
      )
    };
    std::fs::rename(&oldpath, &newpath).map_err(err_mapper)?;
    Ok(())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkArgs {
  oldpath: String,
  newpath: String,
}

#[op]
fn op_link_sync(state: &mut OpState, args: LinkArgs) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  let permissions = state.borrow_mut::<Permissions>();
  permissions.read.check(&oldpath)?;
  permissions.write.check(&oldpath)?;
  permissions.read.check(&newpath)?;
  permissions.write.check(&newpath)?;

  debug!("op_link_sync {} {}", oldpath.display(), newpath.display());
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, link '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  std::fs::hard_link(&oldpath, &newpath).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_link_async(
  state: Rc<RefCell<OpState>>,
  args: LinkArgs,
) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<Permissions>();
    permissions.read.check(&oldpath)?;
    permissions.write.check(&oldpath)?;
    permissions.read.check(&newpath)?;
    permissions.write.check(&newpath)?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_link_async {} {}", oldpath.display(), newpath.display());
    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!(
          "{}, link '{}' -> '{}'",
          err,
          oldpath.display(),
          newpath.display()
        ),
      )
    };
    std::fs::hard_link(&oldpath, &newpath).map_err(err_mapper)?;
    Ok(())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymlinkArgs {
  oldpath: String,
  newpath: String,
  #[cfg(not(unix))]
  options: Option<SymlinkOptions>,
}

#[cfg(not(unix))]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymlinkOptions {
  _type: String,
}

#[op]
fn op_symlink_sync(
  state: &mut OpState,
  args: SymlinkArgs,
) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  state.borrow_mut::<Permissions>().write.check_all()?;
  state.borrow_mut::<Permissions>().read.check_all()?;

  debug!(
    "op_symlink_sync {} {}",
    oldpath.display(),
    newpath.display()
  );
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::symlink;
    symlink(&oldpath, &newpath).map_err(err_mapper)?;
    Ok(())
  }
  #[cfg(not(unix))]
  {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    match args.options {
      Some(options) => match options._type.as_ref() {
        "file" => symlink_file(&oldpath, &newpath).map_err(err_mapper)?,
        "dir" => symlink_dir(&oldpath, &newpath).map_err(err_mapper)?,
        _ => return Err(type_error("unsupported type")),
      },
      None => {
        let old_meta = std::fs::metadata(&oldpath);
        match old_meta {
          Ok(metadata) => {
            if metadata.is_file() {
              symlink_file(&oldpath, &newpath).map_err(err_mapper)?
            } else if metadata.is_dir() {
              symlink_dir(&oldpath, &newpath).map_err(err_mapper)?
            }
          }
          Err(_) => return Err(type_error("you must pass a `options` argument for non-existent target path in windows".to_string())),
        }
      }
    };
    Ok(())
  }
}

#[op]
async fn op_symlink_async(
  state: Rc<RefCell<OpState>>,
  args: SymlinkArgs,
) -> Result<(), AnyError> {
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check_all()?;
    state.borrow_mut::<Permissions>().read.check_all()?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_symlink_async {} {}", oldpath.display(), newpath.display());
    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!(
          "{}, symlink '{}' -> '{}'",
          err,
          oldpath.display(),
          newpath.display()
        ),
      )
    };
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(&oldpath, &newpath).map_err(err_mapper)?;
      Ok(())
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::{symlink_dir, symlink_file};

      match args.options {
        Some(options) => match options._type.as_ref() {
          "file" => symlink_file(&oldpath, &newpath).map_err(err_mapper)?,
          "dir" => symlink_dir(&oldpath, &newpath).map_err(err_mapper)?,
          _ => return Err(type_error("unsupported type")),
        },
        None => {
          let old_meta = std::fs::metadata(&oldpath);
          match old_meta {
            Ok(metadata) => {
              if metadata.is_file() {
                symlink_file(&oldpath, &newpath).map_err(err_mapper)?
              } else if metadata.is_dir() {
                symlink_dir(&oldpath, &newpath).map_err(err_mapper)?
              }
            }
            Err(_) => return Err(type_error("you must pass a `options` argument for non-existent target path in windows".to_string())),
          }
        }
      };
      Ok(())
    }
  })
  .await
  .unwrap()
}

#[op]
fn op_read_link_sync(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError> {
  let path = PathBuf::from(&path);

  state.borrow_mut::<Permissions>().read.check(&path)?;

  debug!("op_read_link_value {}", path.display());
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!("{}, readlink '{}'", err, path.display()),
    )
  };
  let target = std::fs::read_link(&path)
    .map_err(err_mapper)?
    .into_os_string();
  let targetstr = into_string(target)?;
  Ok(targetstr)
}

#[op]
async fn op_read_link_async(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError> {
  let path = PathBuf::from(&path);
  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().read.check(&path)?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_read_link_async {}", path.display());
    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!("{}, readlink '{}'", err, path.display()),
      )
    };
    let target = std::fs::read_link(&path)
      .map_err(err_mapper)?
      .into_os_string();
    let targetstr = into_string(target)?;
    Ok(targetstr)
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FtruncateArgs {
  rid: ResourceId,
  len: i32,
}

#[op]
fn op_ftruncate_sync(
  state: &mut OpState,
  args: FtruncateArgs,
) -> Result<(), AnyError> {
  let rid = args.rid;
  let len = args.len as u64;
  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => std_file.set_len(len).map_err(AnyError::from),
    Err(_) => Err(type_error("cannot truncate this type of resource")),
  })?;
  Ok(())
}

#[op]
async fn op_ftruncate_async(
  state: Rc<RefCell<OpState>>,
  args: FtruncateArgs,
) -> Result<(), AnyError> {
  let rid = args.rid;
  let len = args.len as u64;

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();

  tokio::task::spawn_blocking(move || {
    let std_file = std_file.lock().unwrap();
    std_file.set_len(len)
  })
  .await?
  .map_err(AnyError::from)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TruncateArgs {
  path: String,
  len: u64,
}

#[op]
fn op_truncate_sync(
  state: &mut OpState,
  args: TruncateArgs,
) -> Result<(), AnyError> {
  let path = PathBuf::from(&args.path);
  let len = args.len;

  state.borrow_mut::<Permissions>().write.check(&path)?;

  debug!("op_truncate_sync {} {}", path.display(), len);
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!("{}, truncate '{}'", err, path.display()),
    )
  };
  let f = std::fs::OpenOptions::new()
    .write(true)
    .open(&path)
    .map_err(err_mapper)?;
  f.set_len(len).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_truncate_async(
  state: Rc<RefCell<OpState>>,
  args: TruncateArgs,
) -> Result<(), AnyError> {
  let path = PathBuf::from(&args.path);
  let len = args.len;
  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<Permissions>().write.check(&path)?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_truncate_async {} {}", path.display(), len);
    let err_mapper = |err: Error| {
      Error::new(
        err.kind(),
        format!("{}, truncate '{}'", err, path.display()),
      )
    };
    let f = std::fs::OpenOptions::new()
      .write(true)
      .open(&path)
      .map_err(err_mapper)?;
    f.set_len(len).map_err(err_mapper)?;
    Ok(())
  })
  .await
  .unwrap()
}

fn make_temp(
  dir: Option<&Path>,
  prefix: Option<&str>,
  suffix: Option<&str>,
  is_dir: bool,
) -> std::io::Result<PathBuf> {
  let prefix_ = prefix.unwrap_or("");
  let suffix_ = suffix.unwrap_or("");
  let mut buf: PathBuf = match dir {
    Some(p) => p.to_path_buf(),
    None => temp_dir(),
  }
  .join("_");
  let mut rng = thread_rng();
  loop {
    let unique = rng.gen::<u32>();
    buf.set_file_name(format!("{}{:08x}{}", prefix_, unique, suffix_));
    let r = if is_dir {
      #[allow(unused_mut)]
      let mut builder = std::fs::DirBuilder::new();
      #[cfg(unix)]
      {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
      }
      builder.create(buf.as_path())
    } else {
      let mut open_options = std::fs::OpenOptions::new();
      open_options.write(true).create_new(true);
      #[cfg(unix)]
      {
        use std::os::unix::fs::OpenOptionsExt;
        open_options.mode(0o600);
      }
      open_options.open(buf.as_path())?;
      Ok(())
    };
    match r {
      Err(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
      Ok(_) => return Ok(buf),
      Err(e) => return Err(e),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeTempArgs {
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
}

#[op]
fn op_make_temp_dir_sync(
  state: &mut OpState,
  args: MakeTempArgs,
) -> Result<String, AnyError> {
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state
    .borrow_mut::<Permissions>()
    .write
    .check(dir.clone().unwrap_or_else(temp_dir).as_path())?;

  // TODO(piscisaureus): use byte vector for paths, not a string.
  // See https://github.com/denoland/deno/issues/627.
  // We can't assume that paths are always valid utf8 strings.
  let path = make_temp(
    // Converting Option<String> to Option<&str>
    dir.as_deref(),
    prefix.as_deref(),
    suffix.as_deref(),
    true,
  )?;
  let path_str = into_string(path.into_os_string())?;

  Ok(path_str)
}

#[op]
async fn op_make_temp_dir_async(
  state: Rc<RefCell<OpState>>,
  args: MakeTempArgs,
) -> Result<String, AnyError> {
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<Permissions>()
      .write
      .check(dir.clone().unwrap_or_else(temp_dir).as_path())?;
  }
  tokio::task::spawn_blocking(move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = make_temp(
      // Converting Option<String> to Option<&str>
      dir.as_deref(),
      prefix.as_deref(),
      suffix.as_deref(),
      true,
    )?;
    let path_str = into_string(path.into_os_string())?;

    Ok(path_str)
  })
  .await
  .unwrap()
}

#[op]
fn op_make_temp_file_sync(
  state: &mut OpState,
  args: MakeTempArgs,
) -> Result<String, AnyError> {
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state
    .borrow_mut::<Permissions>()
    .write
    .check(dir.clone().unwrap_or_else(temp_dir).as_path())?;

  // TODO(piscisaureus): use byte vector for paths, not a string.
  // See https://github.com/denoland/deno/issues/627.
  // We can't assume that paths are always valid utf8 strings.
  let path = make_temp(
    // Converting Option<String> to Option<&str>
    dir.as_deref(),
    prefix.as_deref(),
    suffix.as_deref(),
    false,
  )?;
  let path_str = into_string(path.into_os_string())?;

  Ok(path_str)
}

#[op]
async fn op_make_temp_file_async(
  state: Rc<RefCell<OpState>>,
  args: MakeTempArgs,
) -> Result<String, AnyError> {
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<Permissions>()
      .write
      .check(dir.clone().unwrap_or_else(temp_dir).as_path())?;
  }
  tokio::task::spawn_blocking(move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = make_temp(
      // Converting Option<String> to Option<&str>
      dir.as_deref(),
      prefix.as_deref(),
      suffix.as_deref(),
      false,
    )?;
    let path_str = into_string(path.into_os_string())?;

    Ok(path_str)
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FutimeArgs {
  rid: ResourceId,
  atime: (i64, u32),
  mtime: (i64, u32),
}

#[op]
fn op_futime_sync(
  state: &mut OpState,
  args: FutimeArgs,
) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.futimeSync");
  let rid = args.rid;
  let atime = filetime::FileTime::from_unix_time(args.atime.0, args.atime.1);
  let mtime = filetime::FileTime::from_unix_time(args.mtime.0, args.mtime.1);

  StdFileResource::with(state, rid, |r| match r {
    Ok(std_file) => {
      filetime::set_file_handle_times(std_file, Some(atime), Some(mtime))
        .map_err(AnyError::from)
    }
    Err(_) => Err(type_error(
      "cannot futime on this type of resource".to_string(),
    )),
  })?;

  Ok(())
}

#[op]
async fn op_futime_async(
  state: Rc<RefCell<OpState>>,
  args: FutimeArgs,
) -> Result<(), AnyError> {
  super::check_unstable2(&state, "Deno.futime");
  let rid = args.rid;
  let atime = filetime::FileTime::from_unix_time(args.atime.0, args.atime.1);
  let mtime = filetime::FileTime::from_unix_time(args.mtime.0, args.mtime.1);

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<StdFileResource>(rid)?;

  if resource.fs_file.is_none() {
    return Err(bad_resource_id());
  }

  let fs_file = resource.fs_file.as_ref().unwrap();
  let std_file = fs_file.0.as_ref().unwrap().clone();
  tokio::task::spawn_blocking(move || {
    let std_file = std_file.lock().unwrap();
    filetime::set_file_handle_times(&std_file, Some(atime), Some(mtime))?;
    Ok(())
  })
  .await
  .unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtimeArgs {
  path: String,
  atime: (i64, u32),
  mtime: (i64, u32),
}

#[op]
fn op_utime_sync(state: &mut OpState, args: UtimeArgs) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.utime");

  let path = PathBuf::from(&args.path);
  let atime = filetime::FileTime::from_unix_time(args.atime.0, args.atime.1);
  let mtime = filetime::FileTime::from_unix_time(args.mtime.0, args.mtime.1);

  state.borrow_mut::<Permissions>().write.check(&path)?;
  filetime::set_file_times(&path, atime, mtime).map_err(|err| {
    Error::new(err.kind(), format!("{}, utime '{}'", err, path.display()))
  })?;
  Ok(())
}

#[op]
async fn op_utime_async(
  state: Rc<RefCell<OpState>>,
  args: UtimeArgs,
) -> Result<(), AnyError> {
  super::check_unstable(&state.borrow(), "Deno.utime");

  let path = PathBuf::from(&args.path);
  let atime = filetime::FileTime::from_unix_time(args.atime.0, args.atime.1);
  let mtime = filetime::FileTime::from_unix_time(args.mtime.0, args.mtime.1);

  state
    .borrow_mut()
    .borrow_mut::<Permissions>()
    .write
    .check(&path)?;

  tokio::task::spawn_blocking(move || {
    filetime::set_file_times(&path, atime, mtime).map_err(|err| {
      Error::new(err.kind(), format!("{}, utime '{}'", err, path.display()))
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

#[op]
fn op_cwd(state: &mut OpState) -> Result<String, AnyError> {
  let path = current_dir()?;
  state
    .borrow_mut::<Permissions>()
    .read
    .check_blind(&path, "CWD")?;
  let path_str = into_string(path.into_os_string())?;
  Ok(path_str)
}
