// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use super::io::std_file_resource;
use super::io::{FileMetadata, StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::ops::dispatch_json::JsonResult;
use crate::state::State;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use std::cell::RefCell;
use std::convert::From;
use std::env::{current_dir, set_current_dir, temp_dir};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use rand::{thread_rng, Rng};

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  let t = &CoreIsolate::state(i).borrow().resource_table.clone();

  i.register_op("op_open_sync", s.stateful_json_op_sync(t, op_open_sync));
  i.register_op("op_open_async", s.stateful_json_op_async(t, op_open_async));

  i.register_op("op_seek", s.stateful_json_op2(op_seek));
  i.register_op("op_fdatasync", s.stateful_json_op2(op_fdatasync));
  i.register_op("op_fsync", s.stateful_json_op2(op_fsync));
  i.register_op("op_fstat", s.stateful_json_op2(op_fstat));
  i.register_op("op_umask", s.stateful_json_op(op_umask));
  i.register_op("op_chdir", s.stateful_json_op(op_chdir));
  i.register_op("op_mkdir", s.stateful_json_op(op_mkdir));
  i.register_op("op_chmod", s.stateful_json_op(op_chmod));
  i.register_op("op_chown", s.stateful_json_op(op_chown));
  i.register_op("op_remove", s.stateful_json_op(op_remove));
  i.register_op("op_copy_file", s.stateful_json_op(op_copy_file));
  i.register_op("op_stat", s.stateful_json_op(op_stat));
  i.register_op("op_realpath", s.stateful_json_op(op_realpath));
  i.register_op("op_read_dir", s.stateful_json_op(op_read_dir));
  i.register_op("op_rename", s.stateful_json_op(op_rename));
  i.register_op("op_link", s.stateful_json_op(op_link));
  i.register_op("op_symlink", s.stateful_json_op(op_symlink));
  i.register_op("op_read_link", s.stateful_json_op(op_read_link));
  i.register_op("op_ftruncate", s.stateful_json_op2(op_ftruncate));
  i.register_op("op_truncate", s.stateful_json_op(op_truncate));
  i.register_op("op_make_temp_dir", s.stateful_json_op(op_make_temp_dir));
  i.register_op("op_make_temp_file", s.stateful_json_op(op_make_temp_file));
  i.register_op("op_cwd", s.stateful_json_op(op_cwd));
  i.register_op("op_utime", s.stateful_json_op(op_utime));
}

fn into_string(s: std::ffi::OsString) -> Result<String, OpError> {
  s.into_string().map_err(|_| OpError::invalid_utf8())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  path: String,
  mode: Option<u32>,
  options: OpenOptions,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct OpenOptions {
  read: bool,
  write: bool,
  create: bool,
  truncate: bool,
  append: bool,
  create_new: bool,
}

fn open_helper(
  state: &State,
  args: Value,
) -> Result<(PathBuf, std::fs::OpenOptions), OpError> {
  let args: OpenArgs = serde_json::from_value(args)?;
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

  let options = args.options;
  if options.read {
    state.check_read(&path)?;
  }

  if options.write || options.append {
    state.check_write(&path)?;
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

fn op_open_sync(
  state: &State,
  resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, OpError> {
  let (path, open_options) = open_helper(state, args)?;
  let std_file = open_options.open(path)?;
  let tokio_file = tokio::fs::File::from_std(std_file);
  let rid = resource_table.add(
    "fsFile",
    Box::new(StreamResourceHolder::new(StreamResource::FsFile(Some((
      tokio_file,
      FileMetadata::default(),
    ))))),
  );
  Ok(json!(rid))
}

async fn op_open_async(
  state: Rc<State>,
  resource_table: Rc<RefCell<ResourceTable>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, OpError> {
  let (path, open_options) = open_helper(&state, args)?;
  let tokio_file = tokio::fs::OpenOptions::from(open_options)
    .open(path)
    .await?;
  let rid = resource_table.borrow_mut().add(
    "fsFile",
    Box::new(StreamResourceHolder::new(StreamResource::FsFile(Some((
      tokio_file,
      FileMetadata::default(),
    ))))),
  );
  Ok(json!(rid))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekArgs {
  promise_id: Option<u64>,
  rid: i32,
  offset: i64,
  whence: i32,
}

fn op_seek(
  isolate_state: &mut CoreIsolateState,
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  use std::io::{Seek, SeekFrom};
  let args: SeekArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let offset = args.offset;
  let whence = args.whence as u32;
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(offset),
    2 => SeekFrom::End(offset),
    _ => {
      return Err(OpError::type_error(format!(
        "Invalid seek mode: {}",
        whence
      )));
    }
  };

  let resource_table = isolate_state.resource_table.clone();
  let is_sync = args.promise_id.is_none();

  if is_sync {
    let mut resource_table = resource_table.borrow_mut();
    let pos = std_file_resource(&mut resource_table, rid, |r| match r {
      Ok(std_file) => std_file.seek(seek_from).map_err(OpError::from),
      Err(_) => Err(OpError::type_error(
        "cannot seek on this type of resource".to_string(),
      )),
    })?;
    Ok(JsonOp::Sync(json!(pos)))
  } else {
    // TODO(ry) This is a fake async op. We need to use poll_fn,
    // tokio::fs::File::start_seek and tokio::fs::File::poll_complete
    let fut = async move {
      let mut resource_table = resource_table.borrow_mut();
      let pos = std_file_resource(&mut resource_table, rid, |r| match r {
        Ok(std_file) => std_file.seek(seek_from).map_err(OpError::from),
        Err(_) => Err(OpError::type_error(
          "cannot seek on this type of resource".to_string(),
        )),
      })?;
      Ok(json!(pos))
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FdatasyncArgs {
  promise_id: Option<u64>,
  rid: i32,
}

fn op_fdatasync(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.fdatasync");
  let args: FdatasyncArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let resource_table = isolate_state.resource_table.clone();
  let is_sync = args.promise_id.is_none();

  if is_sync {
    let mut resource_table = resource_table.borrow_mut();
    std_file_resource(&mut resource_table, rid, |r| match r {
      Ok(std_file) => std_file.sync_data().map_err(OpError::from),
      Err(_) => Err(OpError::type_error(
        "cannot sync this type of resource".to_string(),
      )),
    })?;
    Ok(JsonOp::Sync(json!({})))
  } else {
    let fut = async move {
      let mut resource_table = resource_table.borrow_mut();
      std_file_resource(&mut resource_table, rid, |r| match r {
        Ok(std_file) => std_file.sync_data().map_err(OpError::from),
        Err(_) => Err(OpError::type_error(
          "cannot sync this type of resource".to_string(),
        )),
      })?;
      Ok(json!({}))
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsyncArgs {
  promise_id: Option<u64>,
  rid: i32,
}

fn op_fsync(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.fsync");
  let args: FsyncArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let resource_table = isolate_state.resource_table.clone();
  let is_sync = args.promise_id.is_none();

  if is_sync {
    let mut resource_table = resource_table.borrow_mut();
    std_file_resource(&mut resource_table, rid, |r| match r {
      Ok(std_file) => std_file.sync_all().map_err(OpError::from),
      Err(_) => Err(OpError::type_error(
        "cannot sync this type of resource".to_string(),
      )),
    })?;
    Ok(JsonOp::Sync(json!({})))
  } else {
    let fut = async move {
      let mut resource_table = resource_table.borrow_mut();
      std_file_resource(&mut resource_table, rid, |r| match r {
        Ok(std_file) => std_file.sync_all().map_err(OpError::from),
        Err(_) => Err(OpError::type_error(
          "cannot sync this type of resource".to_string(),
        )),
      })?;
      Ok(json!({}))
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FstatArgs {
  promise_id: Option<u64>,
  rid: i32,
}

fn op_fstat(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.fstat");
  let args: FstatArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let resource_table = isolate_state.resource_table.clone();
  let is_sync = args.promise_id.is_none();

  if is_sync {
    let mut resource_table = resource_table.borrow_mut();
    let metadata = std_file_resource(&mut resource_table, rid, |r| match r {
      Ok(std_file) => std_file.metadata().map_err(OpError::from),
      Err(_) => Err(OpError::type_error(
        "cannot stat this type of resource".to_string(),
      )),
    })?;
    Ok(JsonOp::Sync(get_stat_json(metadata).unwrap()))
  } else {
    let fut = async move {
      let mut resource_table = resource_table.borrow_mut();
      let metadata =
        std_file_resource(&mut resource_table, rid, |r| match r {
          Ok(std_file) => std_file.metadata().map_err(OpError::from),
          Err(_) => Err(OpError::type_error(
            "cannot stat this type of resource".to_string(),
          )),
        })?;
      Ok(get_stat_json(metadata).unwrap())
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
struct UmaskArgs {
  mask: Option<u32>,
}

fn op_umask(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.umask");
  let args: UmaskArgs = serde_json::from_value(args)?;
  // TODO implement umask for Windows
  // see https://github.com/nodejs/node/blob/master/src/node_process_methods.cc
  // and https://docs.microsoft.com/fr-fr/cpp/c-runtime-library/reference/umask?view=vs-2019
  #[cfg(not(unix))]
  {
    let _ = args.mask; // avoid unused warning.
    Err(OpError::not_implemented())
  }
  #[cfg(unix)]
  {
    use nix::sys::stat::mode_t;
    use nix::sys::stat::umask;
    use nix::sys::stat::Mode;
    let r = if let Some(mask) = args.mask {
      // If mask provided, return previous.
      umask(Mode::from_bits_truncate(mask as mode_t))
    } else {
      // If no mask provided, we query the current. Requires two syscalls.
      let prev = umask(Mode::from_bits_truncate(0o777));
      let _ = umask(prev);
      prev
    };
    Ok(JsonOp::Sync(json!(r.bits() as u32)))
  }
}

#[derive(Deserialize)]
struct ChdirArgs {
  directory: String,
}

fn op_chdir(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ChdirArgs = serde_json::from_value(args)?;
  let d = PathBuf::from(&args.directory);
  state.check_read(&d)?;
  set_current_dir(&d)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MkdirArgs {
  promise_id: Option<u64>,
  path: String,
  recursive: bool,
  mode: Option<u32>,
}

fn op_mkdir(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: MkdirArgs = serde_json::from_value(args)?;
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode.unwrap_or(0o777) & 0o777;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_mkdir {} {:o} {}", path.display(), mode, args.recursive);
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(args.recursive);
    #[cfg(unix)]
    {
      use std::os::unix::fs::DirBuilderExt;
      builder.mode(mode);
    }
    builder.create(path)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChmodArgs {
  promise_id: Option<u64>,
  path: String,
  mode: u32,
}

fn op_chmod(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ChmodArgs = serde_json::from_value(args)?;
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode & 0o777;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chmod {} {:o}", path.display(), mode);
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let permissions = PermissionsExt::from_mode(mode);
      std::fs::set_permissions(&path, permissions)?;
      Ok(json!({}))
    }
    // TODO Implement chmod for Windows (#4357)
    #[cfg(not(unix))]
    {
      // Still check file/dir exists on Windows
      let _metadata = std::fs::metadata(&path)?;
      Err(OpError::not_implemented())
    }
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChownArgs {
  promise_id: Option<u64>,
  path: String,
  uid: Option<u32>,
  gid: Option<u32>,
}

fn op_chown(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ChownArgs = serde_json::from_value(args)?;
  let path = Path::new(&args.path).to_path_buf();

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chown {} {:?} {:?}", path.display(), args.uid, args.gid,);
    #[cfg(unix)]
    {
      use nix::unistd::{chown, Gid, Uid};
      let nix_uid = args.uid.map(Uid::from_raw);
      let nix_gid = args.gid.map(Gid::from_raw);
      chown(&path, nix_uid, nix_gid)?;
      Ok(json!({}))
    }
    // TODO Implement chown for Windows
    #[cfg(not(unix))]
    {
      Err(OpError::not_implemented())
    }
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveArgs {
  promise_id: Option<u64>,
  path: String,
  recursive: bool,
}

fn op_remove(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: RemoveArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);
  let recursive = args.recursive;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    #[cfg(not(unix))]
    use std::os::windows::prelude::MetadataExt;

    let metadata = std::fs::symlink_metadata(&path)?;

    debug!("op_remove {} {}", path.display(), recursive);
    let file_type = metadata.file_type();
    if file_type.is_file() {
      std::fs::remove_file(&path)?;
    } else if recursive {
      std::fs::remove_dir_all(&path)?;
    } else if file_type.is_symlink() {
      #[cfg(unix)]
      std::fs::remove_file(&path)?;
      #[cfg(not(unix))]
      {
        use winapi::um::winnt::FILE_ATTRIBUTE_DIRECTORY;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
          std::fs::remove_dir(&path)?;
        } else {
          std::fs::remove_file(&path)?;
        }
      }
    } else if file_type.is_dir() {
      std::fs::remove_dir(&path)?;
    } else {
      // pipes, sockets, etc...
      std::fs::remove_file(&path)?;
    }
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyFileArgs {
  promise_id: Option<u64>,
  from: String,
  to: String,
}

fn op_copy_file(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: CopyFileArgs = serde_json::from_value(args)?;
  let from = PathBuf::from(&args.from);
  let to = PathBuf::from(&args.to);

  state.check_read(&from)?;
  state.check_write(&to)?;

  debug!("op_copy_file {} {}", from.display(), to.display());
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // On *nix, Rust reports non-existent `from` as ErrorKind::InvalidInput
    // See https://github.com/rust-lang/rust/issues/54800
    // Once the issue is resolved, we should remove this workaround.
    if cfg!(unix) && !from.is_file() {
      return Err(OpError::not_found("File not found".to_string()));
    }

    // returns size of from as u64 (we ignore)
    std::fs::copy(&from, &to)?;
    Ok(json!({}))
  })
}

fn to_msec(maybe_time: Result<SystemTime, io::Error>) -> serde_json::Value {
  match maybe_time {
    Ok(time) => {
      let msec = time
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs_f64() * 1000f64)
        .unwrap_or_else(|err| err.duration().as_secs_f64() * -1000f64);
      serde_json::Number::from_f64(msec)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
    }
    Err(_) => serde_json::Value::Null,
  }
}

#[inline(always)]
fn get_stat_json(metadata: std::fs::Metadata) -> JsonResult {
  // Unix stat member (number types only). 0 if not on unix.
  macro_rules! usm {
    ($member: ident) => {{
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
  let json_val = json!({
    "isFile": metadata.is_file(),
    "isDirectory": metadata.is_dir(),
    "isSymlink": metadata.file_type().is_symlink(),
    "size": metadata.len(),
    // In milliseconds, like JavaScript. Available on both Unix or Windows.
    "mtime": to_msec(metadata.modified()),
    "atime": to_msec(metadata.accessed()),
    "birthtime": to_msec(metadata.created()),
    // Following are only valid under Unix.
    "dev": usm!(dev),
    "ino": usm!(ino),
    "mode": usm!(mode),
    "nlink": usm!(nlink),
    "uid": usm!(uid),
    "gid": usm!(gid),
    "rdev": usm!(rdev),
    // TODO(kevinkassimo): *time_nsec requires BigInt.
    // Probably should be treated as String if we need to add them.
    "blksize": usm!(blksize),
    "blocks": usm!(blocks),
  });
  Ok(json_val)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatArgs {
  promise_id: Option<u64>,
  path: String,
  lstat: bool,
}

fn op_stat(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: StatArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);
  let lstat = args.lstat;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_stat {} {}", path.display(), lstat);
    let metadata = if lstat {
      std::fs::symlink_metadata(&path)?
    } else {
      std::fs::metadata(&path)?
    };
    get_stat_json(metadata)
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RealpathArgs {
  promise_id: Option<u64>,
  path: String,
}

fn op_realpath(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: RealpathArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);

  state.check_read(&path)?;
  if path.is_relative() {
    state.check_read_blind(&current_dir()?, "CWD")?;
  }

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_realpath {}", path.display());
    // corresponds to the realpath on Unix and
    // CreateFile and GetFinalPathNameByHandle on Windows
    let realpath = std::fs::canonicalize(&path)?;
    let mut realpath_str =
      into_string(realpath.into_os_string())?.replace("\\", "/");
    if cfg!(windows) {
      realpath_str = realpath_str.trim_start_matches("//?/").to_string();
    }
    Ok(json!(realpath_str))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadDirArgs {
  promise_id: Option<u64>,
  path: String,
}

fn op_read_dir(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ReadDirArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_dir {}", path.display());
    let entries: Vec<_> = std::fs::read_dir(path)?
      .filter_map(|entry| {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        // Not all filenames can be encoded as UTF-8. Skip those for now.
        if let Ok(name) = into_string(entry.file_name()) {
          Some(json!({
            "name": name,
            "isFile": file_type.is_file(),
            "isDirectory": file_type.is_dir(),
            "isSymlink": file_type.is_symlink()
          }))
        } else {
          None
        }
      })
      .collect();

    Ok(json!({ "entries": entries }))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameArgs {
  promise_id: Option<u64>,
  oldpath: String,
  newpath: String,
}

fn op_rename(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: RenameArgs = serde_json::from_value(args)?;
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  state.check_read(&oldpath)?;
  state.check_write(&oldpath)?;
  state.check_write(&newpath)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_rename {} {}", oldpath.display(), newpath.display());
    std::fs::rename(&oldpath, &newpath)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkArgs {
  promise_id: Option<u64>,
  oldpath: String,
  newpath: String,
}

fn op_link(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.link");
  let args: LinkArgs = serde_json::from_value(args)?;
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  state.check_read(&oldpath)?;
  state.check_write(&newpath)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_link {} {}", oldpath.display(), newpath.display());
    std::fs::hard_link(&oldpath, &newpath)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymlinkArgs {
  promise_id: Option<u64>,
  oldpath: String,
  newpath: String,
  #[cfg(not(unix))]
  options: Option<SymlinkOptions>,
}

#[cfg(not(unix))]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymlinkOptions {
  _type: String,
}

fn op_symlink(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.symlink");
  let args: SymlinkArgs = serde_json::from_value(args)?;
  let oldpath = PathBuf::from(&args.oldpath);
  let newpath = PathBuf::from(&args.newpath);

  state.check_write(&newpath)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_symlink {} {}", oldpath.display(), newpath.display());
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(&oldpath, &newpath)?;
      Ok(json!({}))
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::{symlink_dir, symlink_file};

      match args.options {
        Some(options) => match options._type.as_ref() {
          "file" => symlink_file(&oldpath, &newpath)?,
          "dir" => symlink_dir(&oldpath, &newpath)?,
          _ => return Err(OpError::type_error("unsupported type".to_string())),
        },
        None => {
          let old_meta = std::fs::metadata(&oldpath);
          match old_meta {
            Ok(metadata) => {
              if metadata.is_file() {
                symlink_file(&oldpath, &newpath)?
              } else if metadata.is_dir() {
                symlink_dir(&oldpath, &newpath)?
              }
            }
            Err(_) => return Err(OpError::type_error(
              "you must pass a `options` argument for non-existent target path in windows"
                .to_string(),
            )),
          }
        }
      };
      Ok(json!({}))
    }
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLinkArgs {
  promise_id: Option<u64>,
  path: String,
}

fn op_read_link(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: ReadLinkArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_link {}", path.display());
    let target = std::fs::read_link(&path)?.into_os_string();
    let targetstr = into_string(target)?;
    Ok(json!(targetstr))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FtruncateArgs {
  promise_id: Option<u64>,
  rid: i32,
  len: i32,
}

fn op_ftruncate(
  isolate_state: &mut CoreIsolateState,
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.ftruncate");
  let args: FtruncateArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let len = args.len as u64;

  let resource_table = isolate_state.resource_table.clone();
  let is_sync = args.promise_id.is_none();

  if is_sync {
    let mut resource_table = resource_table.borrow_mut();
    std_file_resource(&mut resource_table, rid, |r| match r {
      Ok(std_file) => std_file.set_len(len).map_err(OpError::from),
      Err(_) => Err(OpError::type_error(
        "cannot truncate this type of resource".to_string(),
      )),
    })?;
    Ok(JsonOp::Sync(json!({})))
  } else {
    let fut = async move {
      let mut resource_table = resource_table.borrow_mut();
      std_file_resource(&mut resource_table, rid, |r| match r {
        Ok(std_file) => std_file.set_len(len).map_err(OpError::from),
        Err(_) => Err(OpError::type_error(
          "cannot truncate this type of resource".to_string(),
        )),
      })?;
      Ok(json!({}))
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TruncateArgs {
  promise_id: Option<u64>,
  path: String,
  len: u64,
}

fn op_truncate(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: TruncateArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);
  let len = args.len;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_truncate {} {}", path.display(), len);
    let f = std::fs::OpenOptions::new().write(true).open(&path)?;
    f.set_len(len)?;
    Ok(json!({}))
  })
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
    Some(ref p) => p.to_path_buf(),
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
struct MakeTempArgs {
  promise_id: Option<u64>,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
}

fn op_make_temp_dir(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state.check_write(dir.clone().unwrap_or_else(temp_dir).as_path())?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
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

    Ok(json!(path_str))
  })
}

fn op_make_temp_file(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state.check_write(dir.clone().unwrap_or_else(temp_dir).as_path())?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
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

    Ok(json!(path_str))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UtimeArgs {
  promise_id: Option<u64>,
  path: String,
  atime: i64,
  mtime: i64,
}

fn op_utime(
  state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.utime");

  let args: UtimeArgs = serde_json::from_value(args)?;
  let path = PathBuf::from(&args.path);

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_utime {} {} {}", args.path, args.atime, args.mtime);
    utime::set_file_times(args.path, args.atime, args.mtime)?;
    Ok(json!({}))
  })
}

fn op_cwd(
  state: &Rc<State>,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let path = current_dir()?;
  state.check_read_blind(&path, "CWD")?;
  let path_str = into_string(path.into_os_string())?;
  Ok(JsonOp::Sync(json!(path_str)))
}
