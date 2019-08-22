// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::utils::*;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::fs as deno_fs;
use crate::msg;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use remove_dir_all::remove_dir_all;
use std::convert::From;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;
use utime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn op_chdir(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_chdir().unwrap();
  let directory = inner.directory().unwrap();
  std::env::set_current_dir(&directory)?;
  ok_buf(empty_buf())
}

pub fn op_mkdir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_mkdir().unwrap();
  let (path, path_) = deno_fs::resolve_from_cwd(inner.path().unwrap())?;
  let recursive = inner.recursive();
  let mode = inner.mode();

  state.check_write(&path_)?;

  blocking(base.sync(), move || {
    debug!("op_mkdir {}", path_);
    deno_fs::mkdir(&path, mode, recursive)?;
    Ok(empty_buf())
  })
}

pub fn op_chmod(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_chmod().unwrap();
  let _mode = inner.mode();
  let (path, path_) = deno_fs::resolve_from_cwd(inner.path().unwrap())?;

  state.check_write(&path_)?;

  blocking(base.sync(), move || {
    debug!("op_chmod {}", &path_);
    // Still check file/dir exists on windows
    let _metadata = fs::metadata(&path)?;
    #[cfg(any(unix))]
    {
      let mut permissions = _metadata.permissions();
      permissions.set_mode(_mode);
      fs::set_permissions(&path, permissions)?;
    }
    Ok(empty_buf())
  })
}

pub fn op_chown(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_chown().unwrap();
  let path = String::from(inner.path().unwrap());
  let uid = inner.uid();
  let gid = inner.gid();

  state.check_write(&path)?;

  blocking(base.sync(), move || {
    debug!("op_chown {}", &path);
    match deno_fs::chown(&path, uid, gid) {
      Ok(_) => Ok(empty_buf()),
      Err(e) => Err(e),
    }
  })
}

pub fn op_remove(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_remove().unwrap();
  let (path, path_) = deno_fs::resolve_from_cwd(inner.path().unwrap())?;
  let recursive = inner.recursive();

  state.check_write(&path_)?;

  blocking(base.sync(), move || {
    debug!("op_remove {}", path.display());
    let metadata = fs::metadata(&path)?;
    if metadata.is_file() {
      fs::remove_file(&path)?;
    } else if recursive {
      remove_dir_all(&path)?;
    } else {
      fs::remove_dir(&path)?;
    }
    Ok(empty_buf())
  })
}

pub fn op_copy_file(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_copy_file().unwrap();
  let (from, from_) = deno_fs::resolve_from_cwd(inner.from().unwrap())?;
  let (to, to_) = deno_fs::resolve_from_cwd(inner.to().unwrap())?;

  state.check_read(&from_)?;
  state.check_write(&to_)?;

  debug!("op_copy_file {} {}", from.display(), to.display());
  blocking(base.sync(), move || {
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
    Ok(empty_buf())
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

pub fn op_stat(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_stat().unwrap();
  let cmd_id = base.cmd_id();
  let (filename, filename_) =
    deno_fs::resolve_from_cwd(inner.filename().unwrap())?;
  let lstat = inner.lstat();

  state.check_read(&filename_)?;

  blocking(base.sync(), move || {
    let builder = &mut FlatBufferBuilder::new();
    debug!("op_stat {} {}", filename.display(), lstat);
    let metadata = if lstat {
      fs::symlink_metadata(&filename)?
    } else {
      fs::metadata(&filename)?
    };

    let inner = msg::StatRes::create(
      builder,
      &msg::StatResArgs {
        is_file: metadata.is_file(),
        is_symlink: metadata.file_type().is_symlink(),
        len: metadata.len(),
        modified: to_seconds!(metadata.modified()),
        accessed: to_seconds!(metadata.accessed()),
        created: to_seconds!(metadata.created()),
        mode: get_mode(&metadata.permissions()),
        has_mode: cfg!(target_family = "unix"),
        ..Default::default()
      },
    );

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::StatRes,
        ..Default::default()
      },
    ))
  })
}

pub fn op_read_dir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_read_dir().unwrap();
  let cmd_id = base.cmd_id();
  let (path, path_) = deno_fs::resolve_from_cwd(inner.path().unwrap())?;

  state.check_read(&path_)?;

  blocking(base.sync(), move || {
    debug!("op_read_dir {}", path.display());
    let builder = &mut FlatBufferBuilder::new();
    let entries: Vec<_> = fs::read_dir(path)?
      .map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        let file_type = metadata.file_type();
        let name = builder.create_string(entry.file_name().to_str().unwrap());

        msg::StatRes::create(
          builder,
          &msg::StatResArgs {
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
            len: metadata.len(),
            modified: to_seconds!(metadata.modified()),
            accessed: to_seconds!(metadata.accessed()),
            created: to_seconds!(metadata.created()),
            name: Some(name),
            mode: get_mode(&metadata.permissions()),
            has_mode: cfg!(target_family = "unix"),
          },
        )
      })
      .collect();

    let entries = builder.create_vector(&entries);
    let inner = msg::ReadDirRes::create(
      builder,
      &msg::ReadDirResArgs {
        entries: Some(entries),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadDirRes,
        ..Default::default()
      },
    ))
  })
}

pub fn op_rename(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_rename().unwrap();
  let (oldpath, oldpath_) =
    deno_fs::resolve_from_cwd(inner.oldpath().unwrap())?;
  let (newpath, newpath_) =
    deno_fs::resolve_from_cwd(inner.newpath().unwrap())?;

  state.check_read(&oldpath_)?;
  state.check_write(&oldpath_)?;
  state.check_write(&newpath_)?;

  blocking(base.sync(), move || {
    debug!("op_rename {} {}", oldpath.display(), newpath.display());
    fs::rename(&oldpath, &newpath)?;
    Ok(empty_buf())
  })
}

pub fn op_link(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_link().unwrap();
  let (oldname, oldpath_) =
    deno_fs::resolve_from_cwd(inner.oldname().unwrap())?;
  let (newname, newname_) =
    deno_fs::resolve_from_cwd(inner.newname().unwrap())?;

  state.check_read(&oldpath_)?;
  state.check_write(&newname_)?;

  blocking(base.sync(), move || {
    debug!("op_link {} {}", oldname.display(), newname.display());
    std::fs::hard_link(&oldname, &newname)?;
    Ok(empty_buf())
  })
}

pub fn op_symlink(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_symlink().unwrap();
  let (oldname, _) = deno_fs::resolve_from_cwd(inner.oldname().unwrap())?;
  let (newname, newname_) =
    deno_fs::resolve_from_cwd(inner.newname().unwrap())?;

  state.check_write(&newname_)?;
  // TODO Use type for Windows.
  if cfg!(windows) {
    return Err(
      DenoError::new(ErrorKind::Other, "Not implemented".to_string()).into(),
    );
  }
  blocking(base.sync(), move || {
    debug!("op_symlink {} {}", oldname.display(), newname.display());
    #[cfg(any(unix))]
    std::os::unix::fs::symlink(&oldname, &newname)?;
    Ok(empty_buf())
  })
}

pub fn op_read_link(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_readlink().unwrap();
  let cmd_id = base.cmd_id();
  let (name, name_) = deno_fs::resolve_from_cwd(inner.name().unwrap())?;

  state.check_read(&name_)?;

  blocking(base.sync(), move || {
    debug!("op_read_link {}", name.display());
    let path = fs::read_link(&name)?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let inner = msg::ReadlinkRes::create(
      builder,
      &msg::ReadlinkResArgs {
        path: Some(path_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadlinkRes,
        ..Default::default()
      },
    ))
  })
}

pub fn op_truncate(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());

  let inner = base.inner_as_truncate().unwrap();
  let (filename, filename_) = deno_fs::resolve_from_cwd(inner.name().unwrap())?;
  let len = inner.len();

  state.check_write(&filename_)?;

  blocking(base.sync(), move || {
    debug!("op_truncate {} {}", filename_, len);
    let f = fs::OpenOptions::new().write(true).open(&filename)?;
    f.set_len(u64::from(len))?;
    Ok(empty_buf())
  })
}

pub fn op_make_temp_dir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let base = Box::new(*base);
  let inner = base.inner_as_make_temp_dir().unwrap();
  let cmd_id = base.cmd_id();

  // FIXME
  state.check_write("make_temp")?;

  let dir = inner.dir().map(PathBuf::from);
  let prefix = inner.prefix().map(String::from);
  let suffix = inner.suffix().map(String::from);

  blocking(base.sync(), move || {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp_dir(
      // Converting Option<String> to Option<&str>
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
    )?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let inner = msg::MakeTempDirRes::create(
      builder,
      &msg::MakeTempDirResArgs {
        path: Some(path_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::MakeTempDirRes,
        ..Default::default()
      },
    ))
  })
}

pub fn op_utime(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());

  let inner = base.inner_as_utime().unwrap();
  let filename = String::from(inner.filename().unwrap());
  let atime = inner.atime();
  let mtime = inner.mtime();

  state.check_write(&filename)?;

  blocking(base.sync(), move || {
    debug!("op_utimes {} {} {}", filename, atime, mtime);
    utime::set_file_times(filename, atime, mtime)?;
    Ok(empty_buf())
  })
}

pub fn op_cwd(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let path = std::env::current_dir()?;
  let builder = &mut FlatBufferBuilder::new();
  let cwd =
    builder.create_string(&path.into_os_string().into_string().unwrap());
  let inner = msg::CwdRes::create(builder, &msg::CwdResArgs { cwd: Some(cwd) });
  let response_buf = serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::CwdRes,
      ..Default::default()
    },
  );
  ok_buf(response_buf)
}
