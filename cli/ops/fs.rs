// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use super::io::std_file_resource;
use super::io::{FileMetadata, StreamResource, StreamResourceHolder};
use crate::fs::resolve_from_cwd;
use crate::op_error::OpError;
use crate::ops::dispatch_json::JsonResult;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use std::convert::From;
use std::env::{current_dir, set_current_dir, temp_dir};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rand::{thread_rng, Rng};

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_open", s.stateful_json_op2(op_open));
  i.register_op("op_seek", s.stateful_json_op2(op_seek));
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
  promise_id: Option<u64>,
  path: String,
  options: OpenOptions,
  mode: Option<u32>,
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

fn op_open(
  isolate: &mut CoreIsolate,
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: OpenArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
  let resource_table = isolate.resource_table.clone();

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

  let is_sync = args.promise_id.is_none();

  if is_sync {
    let std_file = open_options.open(path)?;
    let tokio_file = tokio::fs::File::from_std(std_file);
    let mut resource_table = resource_table.borrow_mut();
    let rid = resource_table.add(
      "fsFile",
      Box::new(StreamResourceHolder::new(StreamResource::FsFile(Some((
        tokio_file,
        FileMetadata::default(),
      ))))),
    );
    Ok(JsonOp::Sync(json!(rid)))
  } else {
    let fut = async move {
      let tokio_file = tokio::fs::OpenOptions::from(open_options)
        .open(path)
        .await?;
      let mut resource_table = resource_table.borrow_mut();
      let rid = resource_table.add(
        "fsFile",
        Box::new(StreamResourceHolder::new(StreamResource::FsFile(Some((
          tokio_file,
          FileMetadata::default(),
        ))))),
      );
      Ok(json!(rid))
    };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekArgs {
  promise_id: Option<u64>,
  rid: i32,
  offset: i32,
  whence: i32,
}

fn op_seek(
  isolate: &mut CoreIsolate,
  _state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  use std::io::{Seek, SeekFrom};
  let args: SeekArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let offset = args.offset;
  let whence = args.whence as u32;
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(i64::from(offset)),
    2 => SeekFrom::End(i64::from(offset)),
    _ => {
      return Err(OpError::type_error(format!(
        "Invalid seek mode: {}",
        whence
      )));
    }
  };

  let resource_table = isolate.resource_table.clone();
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
struct UmaskArgs {
  mask: Option<u32>,
}

fn op_umask(
  _state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ChdirArgs = serde_json::from_value(args)?;
  let d = PathBuf::from(&args.directory);
  state.check_write(&d)?;
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MkdirArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ChmodArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
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
  uid: u32,
  gid: u32,
}

fn op_chown(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ChownArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chown {} {} {}", path.display(), args.uid, args.gid);
    #[cfg(unix)]
    {
      use nix::unistd::{chown, Gid, Uid};
      let nix_uid = Uid::from_raw(args.uid);
      let nix_gid = Gid::from_raw(args.gid);
      chown(&path, Option::Some(nix_uid), Option::Some(nix_gid))?;
      Ok(json!({}))
    }
    // TODO Implement chown for Windows
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
struct RemoveArgs {
  promise_id: Option<u64>,
  path: String,
  recursive: bool,
}

fn op_remove(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: RemoveArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
  let recursive = args.recursive;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    let metadata = std::fs::symlink_metadata(&path)?;
    debug!("op_remove {} {}", path.display(), recursive);
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_symlink() {
      std::fs::remove_file(&path)?;
    } else if recursive {
      std::fs::remove_dir_all(&path)?;
    } else {
      std::fs::remove_dir(&path)?;
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CopyFileArgs = serde_json::from_value(args)?;
  let from = resolve_from_cwd(Path::new(&args.from))?;
  let to = resolve_from_cwd(Path::new(&args.to))?;

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

macro_rules! to_seconds {
  ($time:expr) => {{
    // Unwrap is safe here as if the file is before the unix epoch
    // something is very wrong.
    $time
      .and_then(|t| Ok(t.duration_since(UNIX_EPOCH).unwrap().as_secs()))
      .unwrap_or(0)
  }};
}

#[inline(always)]
fn get_stat_json(
  metadata: std::fs::Metadata,
  maybe_name: Option<String>,
) -> JsonResult {
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
  let mut json_val = json!({
    "isFile": metadata.is_file(),
    "isDirectory": metadata.is_dir(),
    "isSymlink": metadata.file_type().is_symlink(),
    "size": metadata.len(),
    // In seconds. Available on both Unix or Windows.
    "modified":to_seconds!(metadata.modified()),
    "accessed":to_seconds!(metadata.accessed()),
    "created":to_seconds!(metadata.created()),
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

  // "name" is an optional field by our design.
  if let Some(name) = maybe_name {
    if let serde_json::Value::Object(ref mut m) = json_val {
      m.insert("name".to_owned(), json!(name));
    }
  }

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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: StatArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
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
    get_stat_json(metadata, None)
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RealpathArgs {
  promise_id: Option<u64>,
  path: String,
}

fn op_realpath(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: RealpathArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;

  state.check_read(&path)?;

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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ReadDirArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_dir {}", path.display());
    let entries: Vec<_> = std::fs::read_dir(path)?
      .filter_map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        // Not all filenames can be encoded as UTF-8. Skip those for now.
        if let Ok(filename) = into_string(entry.file_name()) {
          Some(get_stat_json(metadata, Some(filename)).unwrap())
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: RenameArgs = serde_json::from_value(args)?;
  let oldpath = resolve_from_cwd(Path::new(&args.oldpath))?;
  let newpath = resolve_from_cwd(Path::new(&args.newpath))?;

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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: LinkArgs = serde_json::from_value(args)?;
  let oldpath = resolve_from_cwd(Path::new(&args.oldpath))?;
  let newpath = resolve_from_cwd(Path::new(&args.newpath))?;

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
}

fn op_symlink(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SymlinkArgs = serde_json::from_value(args)?;
  let oldpath = resolve_from_cwd(Path::new(&args.oldpath))?;
  let newpath = resolve_from_cwd(Path::new(&args.newpath))?;

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
    // TODO Implement symlink, use type for Windows
    #[cfg(not(unix))]
    {
      // Unlike with chmod/chown, here we don't
      // require `oldpath` to exist on Windows
      let _ = oldpath; // avoid unused warning
      Err(OpError::not_implemented())
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ReadLinkArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;

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
struct TruncateArgs {
  promise_id: Option<u64>,
  path: String,
  len: u64,
}

fn op_truncate(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: TruncateArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args.dir.map(|s| resolve_from_cwd(Path::new(&s)).unwrap());
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
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
      true,
    )?;
    let path_str = into_string(path.into_os_string())?;

    Ok(json!(path_str))
  })
}

fn op_make_temp_file(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args.dir.map(|s| resolve_from_cwd(Path::new(&s)).unwrap());
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
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
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
  atime: u64,
  mtime: u64,
}

fn op_utime(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: UtimeArgs = serde_json::from_value(args)?;
  let path = resolve_from_cwd(Path::new(&args.path))?;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_utime {} {} {}", args.path, args.atime, args.mtime);
    utime::set_file_times(args.path, args.atime, args.mtime)?;
    Ok(json!({}))
  })
}

fn op_cwd(
  _state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let path = current_dir()?;
  let path_str = into_string(path.into_os_string())?;
  Ok(JsonOp::Sync(json!(path_str)))
}
