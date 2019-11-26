// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use remove_dir_all::remove_dir_all;
use std::convert::From;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("chdir", s.core_op(json_op(s.stateful_op(op_chdir))));
  i.register_op("mkdir", s.core_op(json_op(s.stateful_op(op_mkdir))));
  i.register_op("chmod", s.core_op(json_op(s.stateful_op(op_chmod))));
  i.register_op("chown", s.core_op(json_op(s.stateful_op(op_chown))));
  i.register_op("remove", s.core_op(json_op(s.stateful_op(op_remove))));
  i.register_op("copy_file", s.core_op(json_op(s.stateful_op(op_copy_file))));
  i.register_op("stat", s.core_op(json_op(s.stateful_op(op_stat))));
  i.register_op("realpath", s.core_op(json_op(s.stateful_op(op_realpath))));
  i.register_op("read_dir", s.core_op(json_op(s.stateful_op(op_read_dir))));
  i.register_op("rename", s.core_op(json_op(s.stateful_op(op_rename))));
  i.register_op("link", s.core_op(json_op(s.stateful_op(op_link))));
  i.register_op("symlink", s.core_op(json_op(s.stateful_op(op_symlink))));
  i.register_op("read_link", s.core_op(json_op(s.stateful_op(op_read_link))));
  i.register_op("truncate", s.core_op(json_op(s.stateful_op(op_truncate))));
  i.register_op(
    "make_temp_dir",
    s.core_op(json_op(s.stateful_op(op_make_temp_dir))),
  );
  i.register_op("cwd", s.core_op(json_op(s.stateful_op(op_cwd))));
  i.register_op("utime", s.core_op(json_op(s.stateful_op(op_utime))));
}

#[derive(Deserialize)]
struct ChdirArgs {
  directory: String,
}

fn op_chdir(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
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
  mode: u32,
}

fn op_mkdir(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: MkdirArgs = serde_json::from_value(args)?;
  let (path, path_) = deno_fs::resolve_from_cwd(args.path.as_ref())?;

  state.check_write(&path_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_mkdir {}", path_);
    deno_fs::mkdir(&path, args.mode, args.recursive)?;
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ChmodArgs = serde_json::from_value(args)?;
  let (path, path_) = deno_fs::resolve_from_cwd(args.path.as_ref())?;

  state.check_write(&path_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chmod {}", &path_);
    // Still check file/dir exists on windows
    let _metadata = fs::metadata(&path)?;
    #[cfg(any(unix))]
    {
      let mut permissions = _metadata.permissions();
      permissions.set_mode(args.mode);
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ChownArgs = serde_json::from_value(args)?;

  state.check_write(&args.path)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_chown {}", &args.path);
    match deno_fs::chown(args.path.as_ref(), args.uid, args.gid) {
      Ok(_) => Ok(json!({})),
      Err(e) => Err(e),
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RemoveArgs = serde_json::from_value(args)?;
  let (path, path_) = deno_fs::resolve_from_cwd(args.path.as_ref())?;
  let recursive = args.recursive;

  state.check_write(&path_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_remove {}", path.display());
    let metadata = fs::metadata(&path)?;
    if metadata.is_file() {
      fs::remove_file(&path)?;
    } else if recursive {
      remove_dir_all(&path)?;
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
}

fn op_copy_file(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CopyFileArgs = serde_json::from_value(args)?;

  let (from, from_) = deno_fs::resolve_from_cwd(args.from.as_ref())?;
  let (to, to_) = deno_fs::resolve_from_cwd(args.to.as_ref())?;

  state.check_read(&from_)?;
  state.check_write(&to_)?;

  debug!("op_copy_file {} {}", from.display(), to.display());
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // On *nix, Rust deem non-existent path as invalid input
    // See https://github.com/rust-lang/rust/issues/54800
    // Once the issue is reolved, we should remove this workaround.
    if cfg!(unix) && !from.is_file() {
      return Err(
        DenoError::new(ErrorKind::NotFound, "File not found".to_string())
          .into(),
      );
    }

    fs::copy(&from, &to)?;
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

#[cfg(any(unix))]
fn get_mode(perm: &fs::Permissions) -> u32 {
  perm.mode()
}

#[cfg(not(any(unix)))]
fn get_mode(_perm: &fs::Permissions) -> u32 {
  0
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatArgs {
  promise_id: Option<u64>,
  filename: String,
  lstat: bool,
}

fn op_stat(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: StatArgs = serde_json::from_value(args)?;

  let (filename, filename_) =
    deno_fs::resolve_from_cwd(args.filename.as_ref())?;
  let lstat = args.lstat;

  state.check_read(&filename_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_stat {} {}", filename.display(), lstat);
    let metadata = if lstat {
      fs::symlink_metadata(&filename)?
    } else {
      fs::metadata(&filename)?
    };

    Ok(json!({
      "isFile": metadata.is_file(),
      "isSymlink": metadata.file_type().is_symlink(),
      "len": metadata.len(),
      "modified":to_seconds!(metadata.modified()),
      "accessed":to_seconds!(metadata.accessed()),
      "created":to_seconds!(metadata.created()),
      "mode": get_mode(&metadata.permissions()),
      "hasMode": cfg!(target_family = "unix"), // false on windows,
    }))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RealpathArgs {
  promise_id: Option<u64>,
  path: String,
}

fn op_realpath(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RealpathArgs = serde_json::from_value(args)?;
  let (_, path_) = deno_fs::resolve_from_cwd(args.path.as_ref())?;
  state.check_read(&path_)?;
  let path = args.path.clone();
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_realpath {}", &path);
    // corresponds to the realpath on Unix and
    // CreateFile and GetFinalPathNameByHandle on Windows
    let realpath = fs::canonicalize(&path)?;
    let realpath_str = realpath.to_str().unwrap().to_owned().replace("\\", "/");
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReadDirArgs = serde_json::from_value(args)?;
  let (path, path_) = deno_fs::resolve_from_cwd(args.path.as_ref())?;

  state.check_read(&path_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_dir {}", path.display());

    let entries: Vec<_> = fs::read_dir(path)?
      .map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        let file_type = metadata.file_type();

        json!({
          "isFile": file_type.is_file(),
          "isSymlink": file_type.is_symlink(),
          "len": metadata.len(),
          "modified": to_seconds!(metadata.modified()),
          "accessed": to_seconds!(metadata.accessed()),
          "created": to_seconds!(metadata.created()),
          "mode": get_mode(&metadata.permissions()),
          "name": entry.file_name().to_str().unwrap(),
          "hasMode": cfg!(target_family = "unix"), // false on windows,
        })
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RenameArgs = serde_json::from_value(args)?;

  let (oldpath, oldpath_) = deno_fs::resolve_from_cwd(args.oldpath.as_ref())?;
  let (newpath, newpath_) = deno_fs::resolve_from_cwd(args.newpath.as_ref())?;

  state.check_read(&oldpath_)?;
  state.check_write(&oldpath_)?;
  state.check_write(&newpath_)?;

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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: LinkArgs = serde_json::from_value(args)?;

  let (oldname, oldname_) = deno_fs::resolve_from_cwd(args.oldname.as_ref())?;
  let (newname, newname_) = deno_fs::resolve_from_cwd(args.newname.as_ref())?;

  state.check_read(&oldname_)?;
  state.check_write(&newname_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_link {} {}", oldname.display(), newname.display());
    std::fs::hard_link(&oldname, &newname)?;
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
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SymlinkArgs = serde_json::from_value(args)?;

  let (oldname, _oldname_) = deno_fs::resolve_from_cwd(args.oldname.as_ref())?;
  let (newname, newname_) = deno_fs::resolve_from_cwd(args.newname.as_ref())?;

  state.check_write(&newname_)?;
  // TODO Use type for Windows.
  if cfg!(windows) {
    return Err(
      DenoError::new(ErrorKind::Other, "Not implemented".to_string()).into(),
    );
  }
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_symlink {} {}", oldname.display(), newname.display());
    #[cfg(any(unix))]
    std::os::unix::fs::symlink(&oldname, &newname)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadLinkArgs {
  promise_id: Option<u64>,
  name: String,
}

fn op_read_link(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReadLinkArgs = serde_json::from_value(args)?;

  let (name, name_) = deno_fs::resolve_from_cwd(args.name.as_ref())?;

  state.check_read(&name_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_read_link {}", name.display());
    let path = fs::read_link(&name)?;
    let path_str = path.to_str().unwrap();

    Ok(json!(path_str))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TruncateArgs {
  promise_id: Option<u64>,
  name: String,
  len: u64,
}

fn op_truncate(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: TruncateArgs = serde_json::from_value(args)?;

  let (filename, filename_) = deno_fs::resolve_from_cwd(args.name.as_ref())?;
  let len = args.len;

  state.check_write(&filename_)?;

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_truncate {} {}", filename_, len);
    let f = fs::OpenOptions::new().write(true).open(&filename)?;
    f.set_len(len)?;
    Ok(json!({}))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MakeTempDirArgs {
  promise_id: Option<u64>,
  dir: Option<String>,
  prefix: Option<String>,
  suffix: Option<String>,
}

fn op_make_temp_dir(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: MakeTempDirArgs = serde_json::from_value(args)?;

  // FIXME
  state.check_write("make_temp")?;

  let dir = args.dir.map(PathBuf::from);
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp_dir(
      // Converting Option<String> to Option<&str>
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
    )?;
    let path_str = path.to_str().unwrap();

    Ok(json!(path_str))
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Utime {
  promise_id: Option<u64>,
  filename: String,
  atime: u64,
  mtime: u64,
}

fn op_utime(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: Utime = serde_json::from_value(args)?;
  state.check_write(&args.filename)?;
  let is_sync = args.promise_id.is_none();
  blocking_json(is_sync, move || {
    debug!("op_utimes {} {} {}", args.filename, args.atime, args.mtime);
    utime::set_file_times(args.filename, args.atime, args.mtime)?;
    Ok(json!({}))
  })
}

fn op_cwd(
  _state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let path = std::env::current_dir()?;
  let path_str = path.into_os_string().into_string().unwrap();
  Ok(JsonOp::Sync(json!(path_str)))
}
