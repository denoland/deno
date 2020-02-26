// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::fs as deno_fs;
use crate::futures::FutureExt;
use crate::op_error::OpError;
use crate::ops::dispatch_json::JsonResult;
use crate::state::State;
use deno_core::*;
use std::convert::From;
use std::convert::TryInto;
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

pub fn init(i: &mut Isolate, s: &State) {
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
  i.register_op("op_ftruncate", s.stateful_json_op(op_ftruncate));
  i.register_op("op_fchmod", s.stateful_json_op(op_fchmod));
  i.register_op("op_futime", s.stateful_json_op(op_futime));
  i.register_op("op_fstat", s.stateful_json_op(op_fstat));
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
  let buf = deno_fs::umask(args.mask)?;
  Ok(JsonOp::Sync(json!(buf)))
}

#[derive(Deserialize)]
struct ChdirArgs {
  directory: String,
}

fn op_chdir(
  _state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ChdirArgs = serde_json::from_value(args)?;
  std::env::set_current_dir(&args.directory)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MkdirArgs {
  promise_id: Option<u64>,
  path: String,
  recursive: bool,
  perm: Option<u32>,
}

fn op_mkdir(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MkdirArgs = serde_json::from_value(args)?;
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  let perm = args.perm.unwrap_or(0o777);

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_mkdir {} {:o} {}", path.display(), perm, args.recursive);
    deno_fs::mkdir(&path, perm, args.recursive)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChmodArgs {
  promise_id: Option<u64>,
  path: String,
  perm: u32,
}

fn op_chmod(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ChmodArgs = serde_json::from_value(args)?;
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  #[allow(unused)]
  let perm = args.perm & 0o777;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // Still check file/dir exists on windows
    let _metadata = fs::metadata(&path)?;
    #[cfg(unix)]
    {
      debug!("op_chmod {} {:o}", path.display(), perm);
      let mut permissions = _metadata.permissions();
      permissions.set_mode(perm);
      fs::set_permissions(&path, permissions)?;
    }
    Ok(json!({}))
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chown {} {} {}", path.display(), args.uid, args.gid);
    /*
    match deno_fs::chown(args.path.as_ref(), args.uid, args.gid) {
      Ok(_) => Ok(json!({})),
      Err(e) => Err(OpError::from(e)),
    }
    */
    deno_fs::chown(args.path.as_ref(), args.uid, args.gid)?;
    Ok(json!({}))
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  let recursive = args.recursive;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    let metadata = fs::symlink_metadata(&path)?;
    debug!("op_remove {} {}", path.display(), recursive);
    let file_type = metadata.file_type();
    if file_type.is_file() || file_type.is_symlink() {
      fs::remove_file(&path)?;
    } else if recursive {
      deno_fs::remove_dir_all(&path)?;
    } else {
      fs::remove_dir(&path)?;
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
  create: bool,
  create_new: bool,
}

fn op_copy_file(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CopyFileArgs = serde_json::from_value(args)?;
  let from = deno_fs::resolve_from_cwd(Path::new(&args.from))?;
  let to = deno_fs::resolve_from_cwd(Path::new(&args.to))?;
  let create = args.create;
  let create_new = args.create_new;

  state.check_read(&from)?;
  state.check_write(&to)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_copy_file {} {}", from.display(), to.display());
    // On *nix, Rust reports non-existent `from` as ErrorKind::InvalidInput
    // See https://github.com/rust-lang/rust/issues/54800
    // Once the issue is resolved, we should remove this workaround.
    if cfg!(unix) && !from.is_file() {
      return Err(OpError::not_found("File not found".to_string()));
    }

    if create && !create_new {
      // default, most efficient version -- data never copied out of kernel space
      // returns length of from as u64 (we ignore)
      fs::copy(&from, &to)?;
    } else {
      let mut from_file = fs::OpenOptions::new().read(true).open(&from)?;
      let mut open_options = fs::OpenOptions::new();
      open_options
        .create(create)
        .create_new(create_new)
        .write(true);
      let mut to_file = open_options.open(&to)?;
      let from_meta = from_file.metadata()?;
      to_file.set_permissions(from_meta.permissions())?;
      // returns length of from as u64 (we ignore)
      io::copy(&mut from_file, &mut to_file)?;
    }
    Ok(json!({}))
  })
}

macro_rules! to_seconds {
  ($time:expr) => {{
    // Unwrap is safe here as if the file is before the unix epoch
    // something is very wrong.
    $time
      .and_then(|t| Ok(t.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64))
      .unwrap_or(0)
  }};
}

#[inline(always)]
fn get_stat_json(
  metadata: fs::Metadata,
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

  let mut json_val = json!({
    "isFile": metadata.is_file(),
    "isDir": metadata.is_dir(),
    "isSymlink": metadata.file_type().is_symlink(),
    "size": metadata.len(),
    // In seconds (i64). Available on both Unix or Windows.
    "accessed":to_seconds!(metadata.accessed()),
    "modified":to_seconds!(metadata.modified()), // changed when fdatasync
    "created":to_seconds!(metadata.created()),
    // Following are only valid under Unix.
    "ctime": usm!(ctime), // i64, changed when fdatasync or chown/chmod/rename/moved
    "dev": usm!(dev), // u64
    "ino": usm!(ino), // u64
    "mode": usm!(mode), // usually u32, may be u16 on Mac
    "nlink": usm!(nlink), // u64
    "uid": usm!(uid), // u32
    "gid": usm!(gid), // u32
    "rdev": usm!(rdev), // u64
    // TODO(kevinkassimo): *time_nsec requires BigInt.
    // Probably should be treated as String if we need to add them.
    "blksize": usm!(blksize) as i64, // was u64
    "blocks": usm!(blocks) as i64, // was u64
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  let lstat = args.lstat;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_stat {} {}", path.display(), lstat);
    let metadata = if lstat {
      fs::symlink_metadata(&path)?
    } else {
      fs::metadata(&path)?
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_realpath {}", path.display());
    // corresponds to the realpath on Unix and
    // CreateFile and GetFinalPathNameByHandle on Windows
    let realpath = fs::canonicalize(&path)?;
    let mut realpath_str =
      realpath.to_str().unwrap().to_owned().replace("\\", "/");
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_dir {}", path.display());
    let entries: Vec<_> = fs::read_dir(path)?
      .filter_map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        // Not all filenames can be encoded as UTF-8. Skip those for now.
        if let Some(filename) = entry.file_name().to_str() {
          let filename = Some(filename.to_owned());
          Some(get_stat_json(metadata, filename).unwrap())
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
  let oldpath = deno_fs::resolve_from_cwd(Path::new(&args.oldpath))?;
  let newpath = deno_fs::resolve_from_cwd(Path::new(&args.newpath))?;

  state.check_read(&oldpath)?;
  state.check_write(&oldpath)?;
  state.check_write(&newpath)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_rename {} {}", oldpath.display(), newpath.display());
    fs::rename(&oldpath, &newpath)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkArgs {
  promise_id: Option<u64>,
  oldname: String,
  newname: String,
}

fn op_link(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: LinkArgs = serde_json::from_value(args)?;
  let oldname = deno_fs::resolve_from_cwd(Path::new(&args.oldname))?;
  let newname = deno_fs::resolve_from_cwd(Path::new(&args.newname))?;

  state.check_read(&oldname)?;
  state.check_write(&newname)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_link {} {}", oldname.display(), newname.display());
    fs::hard_link(&oldname, &newname)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymlinkArgs {
  promise_id: Option<u64>,
  oldname: String,
  newname: String,
}

fn op_symlink(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SymlinkArgs = serde_json::from_value(args)?;
  #[allow(unused)]
  let oldname = deno_fs::resolve_from_cwd(Path::new(&args.oldname))?;
  let newname = deno_fs::resolve_from_cwd(Path::new(&args.newname))?;

  state.check_write(&newname)?;
  // TODO Use type for Windows.
  if cfg!(not(unix)) {
    return Err(OpError::other("Not implemented".to_string()));
  }
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    #[cfg(unix)]
    {
      debug!("op_symlink {} {}", oldname.display(), newname.display());
      deno_fs::symlink(&oldname, &newname)?;
    }
    Ok(json!({}))
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
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;

  state.check_read(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_link {}", path.display());
    let path = fs::read_link(&path)?;
    let path_str = path.to_str().unwrap();

    Ok(json!(path_str))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TruncateArgs {
  promise_id: Option<u64>,
  path: String,
  len: i64,
  perm: Option<u32>,
  create: bool,
  create_new: bool,
}

fn op_truncate(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: TruncateArgs = serde_json::from_value(args)?;
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  // require len to be 63 bit unsigned
  let len: u64 = args.len.try_into()?;
  let create = args.create;
  let create_new = args.create_new;

  state.check_write(&path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_truncate {} {}", path.display(), len);
    let mut open_options = fs::OpenOptions::new();
    if let Some(_perm) = args.perm {
      if !(create || create_new) {
        return Err(OpError::type_error(
          "specified perm without allowing file creation".to_string(),
        ));
      }
      // perm only used if creating the file on Unix
      // if not specified, defaults to 0o666
      #[cfg(unix)]
      open_options.mode(_perm & 0o777);
    }
    open_options
      .create(create)
      .create_new(create_new)
      .write(true);
    let f = open_options.open(&path)?;
    f.set_len(len)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MakeTempArgs {
  promise_id: Option<u64>,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
  perm: Option<u32>,
}

fn op_make_temp_dir(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args
    .dir
    .map(|s| deno_fs::resolve_from_cwd(Path::new(&s)).unwrap());
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  let perm = args.perm.unwrap_or(0o700);

  state
    .check_write(dir.clone().unwrap_or_else(std::env::temp_dir).as_path())?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp(
      // Converting Option<String> to Option<&str>
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
      perm,
      true,
    )?;
    let path_str = path.to_str().unwrap();

    Ok(json!(path_str))
  })
}

fn op_make_temp_file(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: MakeTempArgs = serde_json::from_value(args)?;

  let dir = args
    .dir
    .map(|s| deno_fs::resolve_from_cwd(Path::new(&s)).unwrap());
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  let perm = args.perm.unwrap_or(0o600);

  state
    .check_write(dir.clone().unwrap_or_else(std::env::temp_dir).as_path())?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp(
      // Converting Option<String> to Option<&str>
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
      perm,
      false,
    )?;
    let path_str = path.to_str().unwrap();

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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: UtimeArgs = serde_json::from_value(args)?;
  state.check_write(Path::new(&args.path))?;
  // require times to be 63 bit unsigned
  let atime: u64 = args.atime.try_into()?;
  let mtime: u64 = args.mtime.try_into()?;
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_utime {} {} {}", args.path, atime, mtime);
    deno_fs::set_file_times(args.path, atime, mtime)?;
    Ok(json!({}))
  })
}

fn op_cwd(
  _state: &State,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let path = std::env::current_dir()?;
  let path_str = path.into_os_string().into_string().unwrap();
  Ok(JsonOp::Sync(json!(path_str)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FTruncateArgs {
  promise_id: Option<u64>,
  rid: i32,
  len: i64,
}

fn op_ftruncate(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: FTruncateArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  // require len to be 63 bit unsigned
  let len: u64 = args.len.try_into()?;
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;

  let tokio_file = match resource {
    StreamResource::FsFile(ref file, _) => file,
    _ => return Err(OpError::bad_resource()),
  };
  let mut file = futures::executor::block_on(tokio_file.try_clone())?;

  let fut = async move {
    // Unix returns InvalidInput if fd was not opened for writing
    // For consistency with Windows, we check explicitly
    #[cfg(unix)]
    deno_fs::check_open_for_writing(&file)?;
    debug!("op_ftruncate {} {}", rid, len);
    file.set_len(len).await?;
    Ok(json!({}))
  };

  if args.promise_id.is_none() {
    let buf = futures::executor::block_on(fut)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FChmodArgs {
  promise_id: Option<u64>,
  rid: i32,
  // #[allow(unused)]
  perm: u32,
}

fn op_fchmod(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  if cfg!(not(unix)) {
    return Err(OpError::other("Not implemented".to_string()));
  }
  let args: FChmodArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  #[allow(unused)]
  let perm = args.perm & 0o777;
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;

  let tokio_file = match resource {
    // TODO(jp): save metadata instead of re-querying later?
    StreamResource::FsFile(ref file, _metadata) => file,
    _ => return Err(OpError::bad_resource()),
  };
  #[allow(unused)]
  let file = futures::executor::block_on(tokio_file.try_clone())?;

  let fut = async move {
    #[cfg(unix)]
    {
      deno_fs::check_open_for_writing(&file)?;
      debug!("op_fchmod {} {:o}", rid, perm);
      let metadata = file.metadata().await?;
      let mut permissions = metadata.permissions();
      permissions.set_mode(perm);
      file.set_permissions(permissions).await?;
    }
    Ok(json!({}))
  };

  if args.promise_id.is_none() {
    let buf = futures::executor::block_on(fut)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FUtimeArgs {
  promise_id: Option<u64>,
  rid: i32,
  atime: i64,
  mtime: i64,
}

fn op_futime(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  if cfg!(not(unix)) {
    return Err(OpError::other("Not implemented".to_string()));
  }
  let args: FUtimeArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  #[allow(unused)]
  let atime = args.atime;
  #[allow(unused)]
  let mtime = args.mtime;
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;

  let tokio_file = match resource {
    StreamResource::FsFile(ref file, _) => file,
    _ => return Err(OpError::bad_resource()),
  };
  #[allow(unused)]
  let file = futures::executor::block_on(tokio_file.try_clone())?;
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    #[cfg(unix)]
    {
      let fd = deno_fs::check_open_for_writing(&file)?;
      // require times to be 63 bit unsigned
      let atime: u64 = args.atime.try_into()?;
      let mtime: u64 = args.mtime.try_into()?;
      debug!("op_futime {} {} {}", rid, atime, mtime);
      deno_fs::fset_file_times(fd, atime, mtime)?;
    }
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FStatArgs {
  promise_id: Option<u64>,
  rid: i32,
}

fn op_fstat(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  if cfg!(not(unix)) {
    return Err(OpError::other("Not implemented".to_string()));
  }
  let args: FStatArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;

  let tokio_file = match resource {
    // TODO(jp): save metadata instead of re-querying later?
    StreamResource::FsFile(ref file, _metadata) => file,
    _ => return Err(OpError::bad_resource()),
  };
  #[allow(unused)]
  let file = futures::executor::block_on(tokio_file.try_clone())?;

  let fut = async move {
    #[cfg(unix)]
    {
      debug!("op_fstat {}", rid);
      #[allow(unused)]
      let fd = deno_fs::check_open_for_reading(&file)?;
      /*
      let filestat: nix::sys::stat::FileStat = deno_fs::fstat(fd)?;
      let sflag = deno_fs::SFlag::from_bits_truncate(filestat.st_mode);
      // see https://unix.stackexchange.com/questions/91197
      // not available on Linux, and their
      // libc::statx(dirfd, &path, flags, mask, &statxbuf_with_stx_btime)
      // doesn't apply to fd
      #[cfg(target_os = "linux")]
      let birthtime: i64 = 0;
      #[cfg(not(target_os = "linux"))]
      let birthtime: i64 = filestat.st_birthtime;
      let json_val = json!({
        "size": filestat.st_size,
        "isFile": sflag.contains(deno_fs::SFlag::S_IFREG),
        "isDir": sflag.contains(deno_fs::SFlag::S_IFLNK),
        "isSymlink": sflag.contains(deno_fs::SFlag::S_IFDIR),
        // all times are i64
        "accessed": filestat.st_atime,
        "modified": filestat.st_mtime, // changed when fdatasync
        "created": birthtime,
        "ctime": filestat.st_ctime, // changed when fdatasync or chown/chmod/rename/moved
        "dev": filestat.st_dev, // u64
        "ino": filestat.st_ino, // u64
        "mode": filestat.st_mode, // usually u32, may be u16 on Mac
        "nlink": filestat.st_nlink, // u64
        "uid": filestat.st_uid, // u32
        "gid": filestat.st_gid, // u32
        "rdev": filestat.st_rdev, // u64
        "blksize": filestat.st_blksize, // i64
        "blocks": filestat.st_blocks, // i64
      });
      Ok(json_val)
       */
      let metadata = file.metadata().await?;
      get_stat_json(metadata, None)
    }
    #[cfg(not(unix))]
    Ok(json!({}))
  };

  if args.promise_id.is_none() {
    let buf = futures::executor::block_on(fut)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}
