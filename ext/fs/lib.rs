// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it

use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_crypto::rand::thread_rng;
use deno_crypto::rand::Rng;
use deno_io::StdFileResource;
use log::debug;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::env::current_dir;
use std::env::set_current_dir;
use std::env::temp_dir;
use std::io;
use std::io::Error;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  let mut canonicalized_path = path.canonicalize()?;
  if cfg!(windows) {
    canonicalized_path = PathBuf::from(
      canonicalized_path
        .display()
        .to_string()
        .trim_start_matches("\\\\?\\"),
    );
  }
  Ok(canonicalized_path)
}

/// A utility function to map OsStrings to Strings
fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {s:?} is not valid UTF-8");
    custom_error("InvalidData", message)
  })
}

#[cfg(unix)]
pub fn get_nix_error_class(error: &nix::Error) -> &'static str {
  match error {
    nix::Error::ECHILD => "NotFound",
    nix::Error::EINVAL => "TypeError",
    nix::Error::ENOENT => "NotFound",
    nix::Error::ENOTTY => "BadResource",
    nix::Error::EPERM => "PermissionDenied",
    nix::Error::ESRCH => "NotFound",
    nix::Error::UnknownErrno => "Error",
    &nix::Error::ENOTSUP => unreachable!(),
    _ => "Error",
  }
}

struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{api_name}'. The --unstable flag must be provided."
      );
      std::process::exit(70);
    }
  }
}

/// Helper for checking unstable features. Used for sync ops.
fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

pub trait FsPermissions {
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_read_all(&mut self, api_name: &str) -> Result<(), AnyError>;
  fn check_read_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError>;
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_write_all(&mut self, api_name: &str) -> Result<(), AnyError>;
}

#[cfg(not(unix))]
use deno_core::error::generic_error;
#[cfg(not(unix))]
use deno_core::error::not_supported;

pub fn init<P: FsPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder("deno_fs")
    .esm(include_js_files!("30_fs.js",))
    .state(move |state| {
      state.put(UnstableChecker { unstable });
    })
    .ops(vec![
      op_open_sync::decl::<P>(),
      op_open_async::decl::<P>(),
      op_write_file_sync::decl::<P>(),
      op_write_file_async::decl::<P>(),
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
      op_chdir::decl::<P>(),
      op_mkdir_sync::decl::<P>(),
      op_mkdir_async::decl::<P>(),
      op_chmod_sync::decl::<P>(),
      op_chmod_async::decl::<P>(),
      op_chown_sync::decl::<P>(),
      op_chown_async::decl::<P>(),
      op_remove_sync::decl::<P>(),
      op_remove_async::decl::<P>(),
      op_copy_file_sync::decl::<P>(),
      op_copy_file_async::decl::<P>(),
      op_stat_sync::decl::<P>(),
      op_stat_async::decl::<P>(),
      op_realpath_sync::decl::<P>(),
      op_realpath_async::decl::<P>(),
      op_read_dir_sync::decl::<P>(),
      op_read_dir_async::decl::<P>(),
      op_rename_sync::decl::<P>(),
      op_rename_async::decl::<P>(),
      op_link_sync::decl::<P>(),
      op_link_async::decl::<P>(),
      op_symlink_sync::decl::<P>(),
      op_symlink_async::decl::<P>(),
      op_read_link_sync::decl::<P>(),
      op_read_link_async::decl::<P>(),
      op_ftruncate_sync::decl(),
      op_ftruncate_async::decl(),
      op_truncate_sync::decl::<P>(),
      op_truncate_async::decl::<P>(),
      op_make_temp_dir_sync::decl::<P>(),
      op_make_temp_dir_async::decl::<P>(),
      op_make_temp_file_sync::decl::<P>(),
      op_make_temp_file_async::decl::<P>(),
      op_cwd::decl::<P>(),
      op_futime_sync::decl(),
      op_futime_async::decl(),
      op_utime_sync::decl::<P>(),
      op_utime_async::decl::<P>(),
      op_readfile_sync::decl::<P>(),
      op_readfile_text_sync::decl::<P>(),
      op_readfile_async::decl::<P>(),
      op_readfile_text_async::decl::<P>(),
    ])
    .build()
}

fn default_err_mapper(err: Error, desc: String) -> Error {
  Error::new(err.kind(), format!("{err}, {desc}"))
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

#[inline]
fn open_helper<P>(
  state: &mut OpState,
  path: &str,
  mode: Option<u32>,
  options: Option<&OpenOptions>,
  api_name: &str,
) -> Result<(PathBuf, std::fs::OpenOptions), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(path).to_path_buf();

  let mut open_options = std::fs::OpenOptions::new();

  if let Some(mode) = mode {
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

  let permissions = state.borrow_mut::<P>();

  match options {
    None => {
      permissions.check_read(&path, api_name)?;
      open_options
        .read(true)
        .create(false)
        .write(false)
        .truncate(false)
        .append(false)
        .create_new(false);
    }
    Some(options) => {
      if options.read {
        permissions.check_read(&path, api_name)?;
      }

      if options.write || options.append {
        permissions.check_write(&path, api_name)?;
      }

      open_options
        .read(options.read)
        .create(options.create)
        .write(options.write)
        .truncate(options.truncate)
        .append(options.append)
        .create_new(options.create_new);
    }
  }

  Ok((path, open_options))
}

#[op]
fn op_open_sync<P>(
  state: &mut OpState,
  path: String,
  options: Option<OpenOptions>,
  mode: Option<u32>,
) -> Result<ResourceId, AnyError>
where
  P: FsPermissions + 'static,
{
  let (path, open_options) =
    open_helper::<P>(state, &path, mode, options.as_ref(), "Deno.openSync()")?;
  let std_file = open_options.open(&path).map_err(|err| {
    default_err_mapper(err, format!("open '{}'", path.display()))
  })?;
  let resource = StdFileResource::fs_file(std_file);
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[op]
async fn op_open_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  options: Option<OpenOptions>,
  mode: Option<u32>,
) -> Result<ResourceId, AnyError>
where
  P: FsPermissions + 'static,
{
  let (path, open_options) = open_helper::<P>(
    &mut state.borrow_mut(),
    &path,
    mode,
    options.as_ref(),
    "Deno.open()",
  )?;
  let std_file = tokio::task::spawn_blocking(move || {
    open_options.open(&path).map_err(|err| {
      default_err_mapper(err, format!("open '{}'", path.display()))
    })
  })
  .await?;
  let resource = StdFileResource::fs_file(std_file?);
  let rid = state.borrow_mut().resource_table.add(resource);
  Ok(rid)
}

#[inline]
fn write_open_options(
  create: bool,
  append: bool,
  create_new: bool,
) -> OpenOptions {
  OpenOptions {
    read: false,
    write: true,
    create,
    truncate: !append,
    append,
    create_new,
  }
}

#[op]
fn op_write_file_sync<P>(
  state: &mut OpState,
  path: String,
  mode: Option<u32>,
  append: bool,
  create: bool,
  create_new: bool,
  data: ZeroCopyBuf,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let (path, open_options) = open_helper::<P>(
    state,
    &path,
    mode,
    Some(&write_open_options(create, append, create_new)),
    "Deno.writeFileSync()",
  )?;
  write_file(&path, open_options, mode, data)
}

#[op]
async fn op_write_file_async<P>(
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
  P: FsPermissions + 'static,
{
  let (path, open_options) = open_helper::<P>(
    &mut state.borrow_mut(),
    &path,
    mode,
    Some(&write_open_options(create, append, create_new)),
    "Deno.writeFile()",
  )?;

  let write_future = tokio::task::spawn_blocking(move || {
    write_file(&path, open_options, mode, data)
  });

  let cancel_handle = cancel_rid.and_then(|rid| {
    state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(rid)
      .ok()
  });

  if let Some(cancel_handle) = cancel_handle {
    let write_future_rv = write_future.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    return write_future_rv??;
  }

  write_future.await?
}

fn write_file(
  path: &Path,
  open_options: std::fs::OpenOptions,
  _mode: Option<u32>,
  data: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let mut std_file = open_options.open(path).map_err(|err| {
    default_err_mapper(err, format!("open '{}'", path.display()))
  })?;

  // need to chmod the file if it already exists and a mode is specified
  #[cfg(unix)]
  if let Some(mode) = _mode {
    use std::os::unix::fs::PermissionsExt;
    let permissions = PermissionsExt::from_mode(mode & 0o777);
    std_file.set_permissions(permissions).map_err(|err| {
      default_err_mapper(err, format!("chmod '{}'", path.display()))
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
      return Err(type_error(format!("Invalid seek mode: {whence}")));
    }
  };

  Ok((rid, seek_from))
}

#[op]
fn op_seek_sync(state: &mut OpState, args: SeekArgs) -> Result<u64, AnyError> {
  let (rid, seek_from) = seek_helper(args)?;
  StdFileResource::with_file(state, rid, |std_file| {
    std_file.seek(seek_from).map_err(AnyError::from)
  })
}

#[op]
async fn op_seek_async(
  state: Rc<RefCell<OpState>>,
  args: SeekArgs,
) -> Result<u64, AnyError> {
  let (rid, seek_from) = seek_helper(args)?;

  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    std_file.seek(seek_from).map_err(AnyError::from)
  })
  .await
}

#[op]
fn op_fdatasync_sync(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  StdFileResource::with_file(state, rid, |std_file| {
    std_file.sync_data().map_err(AnyError::from)
  })
}

#[op]
async fn op_fdatasync_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    std_file.sync_data().map_err(AnyError::from)
  })
  .await
}

#[op]
fn op_fsync_sync(state: &mut OpState, rid: ResourceId) -> Result<(), AnyError> {
  StdFileResource::with_file(state, rid, |std_file| {
    std_file.sync_all().map_err(AnyError::from)
  })
}

#[op]
async fn op_fsync_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    std_file.sync_all().map_err(AnyError::from)
  })
  .await
}

#[op]
fn op_fstat_sync(
  state: &mut OpState,
  rid: ResourceId,
  out_buf: &mut [u32],
) -> Result<(), AnyError> {
  let metadata = StdFileResource::with_file(state, rid, |std_file| {
    std_file.metadata().map_err(AnyError::from)
  })?;
  let stat = get_stat(metadata);
  stat.write(out_buf);
  Ok(())
}

#[op]
async fn op_fstat_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<FsStat, AnyError> {
  let metadata =
    StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
      std_file.metadata().map_err(AnyError::from)
    })
    .await?;
  Ok(get_stat(metadata))
}

#[op]
fn op_flock_sync(
  state: &mut OpState,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  check_unstable(state, "Deno.flockSync");

  StdFileResource::with_file(state, rid, |std_file| {
    if exclusive {
      std_file.lock_exclusive()?;
    } else {
      std_file.lock_shared()?;
    }
    Ok(())
  })
}

#[op]
async fn op_flock_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  exclusive: bool,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  check_unstable2(&state, "Deno.flock");

  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    if exclusive {
      std_file.lock_exclusive()?;
    } else {
      std_file.lock_shared()?;
    }
    Ok(())
  })
  .await
}

#[op]
fn op_funlock_sync(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  check_unstable(state, "Deno.funlockSync");

  StdFileResource::with_file(state, rid, |std_file| {
    std_file.unlock()?;
    Ok(())
  })
}

#[op]
async fn op_funlock_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(), AnyError> {
  use fs3::FileExt;
  check_unstable2(&state, "Deno.funlock");

  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    std_file.unlock()?;
    Ok(())
  })
  .await
}

#[op]
fn op_umask(state: &mut OpState, mask: Option<u32>) -> Result<u32, AnyError> {
  check_unstable(state, "Deno.umask");
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
    #[cfg(target_os = "linux")]
    {
      Ok(r.bits())
    }
    #[cfg(target_os = "macos")]
    {
      Ok(r.bits() as u32)
    }
  }
}

#[op]
fn op_chdir<P>(state: &mut OpState, directory: &str) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let d = PathBuf::from(&directory);
  state.borrow_mut::<P>().check_read(&d, "Deno.chdir()")?;
  set_current_dir(&d)
    .map_err(|err| default_err_mapper(err, format!("chdir '{directory}'")))?;
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
fn op_mkdir_sync<P>(
  state: &mut OpState,
  args: MkdirArgs,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode.unwrap_or(0o777) & 0o777;
  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.mkdirSync()")?;
  debug!("op_mkdir {} {:o} {}", path.display(), mode, args.recursive);
  let mut builder = std::fs::DirBuilder::new();
  builder.recursive(args.recursive);
  #[cfg(unix)]
  {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(mode);
  }
  builder.create(&path).map_err(|err| {
    default_err_mapper(err, format!("mkdir '{}'", path.display()))
  })?;
  Ok(())
}

#[op]
async fn op_mkdir_async<P>(
  state: Rc<RefCell<OpState>>,
  args: MkdirArgs,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&args.path).to_path_buf();
  let mode = args.mode.unwrap_or(0o777) & 0o777;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.mkdir()")?;
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
      default_err_mapper(err, format!("mkdir '{}'", path.display()))
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

#[op]
fn op_chmod_sync<P>(
  state: &mut OpState,
  path: &str,
  mode: u32,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(path);
  let mode = mode & 0o777;

  state
    .borrow_mut::<P>()
    .check_write(path, "Deno.chmodSync()")?;
  raw_chmod(path, mode)
}

#[op]
async fn op_chmod_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  mode: u32,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&path).to_path_buf();
  let mode = mode & 0o777;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.chmod()")?;
  }

  tokio::task::spawn_blocking(move || raw_chmod(&path, mode))
    .await
    .unwrap()
}

fn raw_chmod(path: &Path, _raw_mode: u32) -> Result<(), AnyError> {
  let err_mapper =
    |err| default_err_mapper(err, format!("chmod '{}'", path.display()));
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let permissions = PermissionsExt::from_mode(_raw_mode);
    std::fs::set_permissions(path, permissions).map_err(err_mapper)?;
    Ok(())
  }
  // TODO Implement chmod for Windows (#4357)
  #[cfg(not(unix))]
  {
    // Still check file/dir exists on Windows
    let _metadata = std::fs::metadata(path).map_err(err_mapper)?;
    Err(not_supported())
  }
}

#[op]
fn op_chown_sync<P>(
  state: &mut OpState,
  path: &str,
  #[cfg_attr(windows, allow(unused_variables))] uid: Option<u32>,
  #[cfg_attr(windows, allow(unused_variables))] gid: Option<u32>,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(path).to_path_buf();
  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.chownSync()")?;
  #[cfg(unix)]
  {
    use nix::unistd::chown;
    use nix::unistd::Gid;
    use nix::unistd::Uid;
    let nix_uid = uid.map(Uid::from_raw);
    let nix_gid = gid.map(Gid::from_raw);
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
async fn op_chown_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  #[cfg_attr(windows, allow(unused_variables))] uid: Option<u32>,
  #[cfg_attr(windows, allow(unused_variables))] gid: Option<u32>,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&path).to_path_buf();

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(&path, "Deno.chown()")?;
  }

  tokio::task::spawn_blocking(move || {
    #[cfg(unix)]
    {
      use nix::unistd::chown;
      use nix::unistd::Gid;
      use nix::unistd::Uid;
      let nix_uid = uid.map(Uid::from_raw);
      let nix_gid = gid.map(Gid::from_raw);
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

#[op]
fn op_remove_sync<P>(
  state: &mut OpState,
  path: &str,
  recursive: bool,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.removeSync()")?;

  #[cfg(not(unix))]
  use std::os::windows::prelude::MetadataExt;

  let err_mapper =
    |err| default_err_mapper(err, format!("remove '{}'", path.display()));
  let metadata = std::fs::symlink_metadata(&path).map_err(err_mapper)?;

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
async fn op_remove_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  recursive: bool,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_write(&path, "Deno.remove()")?;
  }

  tokio::task::spawn_blocking(move || {
    #[cfg(not(unix))]
    use std::os::windows::prelude::MetadataExt;
    let err_mapper =
      |err| default_err_mapper(err, format!("remove '{}'", path.display()));
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

#[op]
fn op_copy_file_sync<P>(
  state: &mut OpState,
  from: &str,
  to: &str,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let from_path = PathBuf::from(from);
  let to_path = PathBuf::from(to);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&from_path, "Deno.copyFileSync()")?;
  permissions.check_write(&to_path, "Deno.copyFileSync()")?;

  // On *nix, Rust reports non-existent `from` as ErrorKind::InvalidInput
  // See https://github.com/rust-lang/rust/issues/54800
  // Once the issue is resolved, we should remove this workaround.
  if cfg!(unix) && !from_path.is_file() {
    return Err(custom_error(
      "NotFound",
      format!(
        "File not found, copy '{}' -> '{}'",
        from_path.display(),
        to_path.display()
      ),
    ));
  }

  let err_mapper = |err| {
    default_err_mapper(
      err,
      format!("copy '{}' -> '{}'", from_path.display(), to_path.display()),
    )
  };

  #[cfg(target_os = "macos")]
  {
    use libc::clonefile;
    use libc::stat;
    use libc::unlink;
    use std::ffi::CString;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;

    let from = CString::new(from).unwrap();
    let to = CString::new(to).unwrap();

    // SAFETY: `from` and `to` are valid C strings.
    // std::fs::copy does open() + fcopyfile() on macOS. We try to use
    // clonefile() instead, which is more efficient.
    unsafe {
      let mut st = std::mem::zeroed();
      let ret = stat(from.as_ptr(), &mut st);
      if ret != 0 {
        return Err(err_mapper(Error::last_os_error()).into());
      }

      if st.st_size > 128 * 1024 {
        // Try unlink. If it fails, we are going to try clonefile() anyway.
        let _ = unlink(to.as_ptr());
        // Matches rust stdlib behavior for io::copy.
        // https://github.com/rust-lang/rust/blob/3fdd578d72a24d4efc2fe2ad18eec3b6ba72271e/library/std/src/sys/unix/fs.rs#L1613-L1616
        if clonefile(from.as_ptr(), to.as_ptr(), 0) == 0 {
          return Ok(());
        }
      } else {
        // Do a regular copy. fcopyfile() is an overkill for < 128KB
        // files.
        let mut buf = [0u8; 128 * 1024];
        let mut from_file =
          std::fs::File::open(&from_path).map_err(err_mapper)?;
        let perm = from_file.metadata().map_err(err_mapper)?.permissions();

        let mut to_file = std::fs::OpenOptions::new()
          // create the file with the correct mode right away
          .mode(perm.mode())
          .write(true)
          .create(true)
          .truncate(true)
          .open(&to_path)
          .map_err(err_mapper)?;
        let writer_metadata = to_file.metadata()?;
        if writer_metadata.is_file() {
          // Set the correct file permissions, in case the file already existed.
          // Don't set the permissions on already existing non-files like
          // pipes/FIFOs or device nodes.
          to_file.set_permissions(perm)?;
        }
        loop {
          let nread = from_file.read(&mut buf).map_err(err_mapper)?;
          if nread == 0 {
            break;
          }
          to_file.write_all(&buf[..nread]).map_err(err_mapper)?;
        }
        return Ok(());
      }
    }

    // clonefile() failed, fall back to std::fs::copy().
  }

  // returns size of from as u64 (we ignore)
  std::fs::copy(&from_path, &to_path).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_copy_file_async<P>(
  state: Rc<RefCell<OpState>>,
  from: String,
  to: String,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let from = PathBuf::from(&from);
  let to = PathBuf::from(&to);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&from, "Deno.copyFile()")?;
    permissions.check_write(&to, "Deno.copyFile()")?;
  }

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
    // returns size of from as u64 (we ignore)
    std::fs::copy(&from, &to).map_err(|err| {
      default_err_mapper(
        err,
        format!("copy '{}' -> '{}'", from.display(), to.display()),
      )
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

fn to_msec(maybe_time: Result<SystemTime, io::Error>) -> (u64, bool) {
  match maybe_time {
    Ok(time) => (
      time
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as u64)
        .unwrap_or_else(|err| err.duration().as_millis() as u64),
      true,
    ),
    Err(_) => (0, false),
  }
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
  pub struct FsStat {
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
  let (mtime, mtime_set) = to_msec(metadata.modified());
  let (atime, atime_set) = to_msec(metadata.accessed());
  let (birthtime, birthtime_set) = to_msec(metadata.created());

  FsStat {
    is_file: metadata.is_file(),
    is_directory: metadata.is_dir(),
    is_symlink: metadata.file_type().is_symlink(),
    size: metadata.len(),
    // In milliseconds, like JavaScript. Available on both Unix or Windows.
    mtime_set,
    mtime,
    atime_set,
    atime,
    birthtime_set,
    birthtime,
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
fn op_stat_sync<P>(
  state: &mut OpState,
  path: &str,
  lstat: bool,
  out_buf: &mut [u32],
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.statSync()")?;
  let err_mapper =
    |err| default_err_mapper(err, format!("stat '{}'", path.display()));
  let metadata = if lstat {
    std::fs::symlink_metadata(&path).map_err(err_mapper)?
  } else {
    std::fs::metadata(&path).map_err(err_mapper)?
  };

  let stat = get_stat(metadata);
  stat.write(out_buf);

  Ok(())
}

#[op]
async fn op_stat_async<P>(
  state: Rc<RefCell<OpState>>,
  args: StatArgs,
) -> Result<FsStat, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&args.path);
  let lstat = args.lstat;

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_read(&path, "Deno.stat()")?;
  }

  tokio::task::spawn_blocking(move || {
    debug!("op_stat_async {} {}", path.display(), lstat);
    let err_mapper =
      |err| default_err_mapper(err, format!("stat '{}'", path.display()));
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
fn op_realpath_sync<P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&path, "Deno.realPathSync()")?;
  if path.is_relative() {
    permissions.check_read_blind(
      &current_dir()?,
      "CWD",
      "Deno.realPathSync()",
    )?;
  }

  debug!("op_realpath_sync {}", path.display());
  // corresponds to the realpath on Unix and
  // CreateFile and GetFinalPathNameByHandle on Windows
  let realpath = canonicalize_path(&path)?;
  let realpath_str = into_string(realpath.into_os_string())?;
  Ok(realpath_str)
}

#[op]
async fn op_realpath_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&path, "Deno.realPath()")?;
    if path.is_relative() {
      permissions.check_read_blind(
        &current_dir()?,
        "CWD",
        "Deno.realPath()",
      )?;
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
fn op_read_dir_sync<P>(
  state: &mut OpState,
  path: String,
) -> Result<Vec<DirEntry>, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readDirSync()")?;

  debug!("op_read_dir_sync {}", path.display());

  let entries: Vec<_> = std::fs::read_dir(&path)
    .map_err(|err| {
      default_err_mapper(err, format!("readdir '{}'", path.display()))
    })?
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
async fn op_read_dir_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<Vec<DirEntry>, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);
  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readDir()")?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_read_dir_async {}", path.display());

    let entries: Vec<_> = std::fs::read_dir(&path)
      .map_err(|err| {
        default_err_mapper(err, format!("readdir '{}'", path.display()))
      })?
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

#[op]
fn op_rename_sync<P>(
  state: &mut OpState,
  oldpath: &str,
  newpath: &str,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&oldpath, "Deno.renameSync()")?;
  permissions.check_write(&oldpath, "Deno.renameSync()")?;
  permissions.check_write(&newpath, "Deno.renameSync()")?;

  std::fs::rename(&oldpath, &newpath).map_err(|err| {
    default_err_mapper(
      err,
      format!("rename '{}' -> '{}'", oldpath.display(), newpath.display()),
    )
  })?;
  Ok(())
}

#[op]
async fn op_rename_async<P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&oldpath, "Deno.rename()")?;
    permissions.check_write(&oldpath, "Deno.rename()")?;
    permissions.check_write(&newpath, "Deno.rename()")?;
  }

  tokio::task::spawn_blocking(move || {
    std::fs::rename(&oldpath, &newpath).map_err(|err| {
      default_err_mapper(
        err,
        format!("rename '{}' -> '{}'", oldpath.display(), newpath.display()),
      )
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

#[op]
fn op_link_sync<P>(
  state: &mut OpState,
  oldpath: &str,
  newpath: &str,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  let permissions = state.borrow_mut::<P>();
  permissions.check_read(&oldpath, "Deno.linkSync()")?;
  permissions.check_write(&oldpath, "Deno.linkSync()")?;
  permissions.check_read(&newpath, "Deno.linkSync()")?;
  permissions.check_write(&newpath, "Deno.linkSync()")?;

  std::fs::hard_link(&oldpath, &newpath).map_err(|err| {
    default_err_mapper(
      err,
      format!("link '{}' -> '{}'", oldpath.display(), newpath.display()),
    )
  })?;
  Ok(())
}

#[op]
async fn op_link_async<P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions.check_read(&oldpath, "Deno.link()")?;
    permissions.check_write(&oldpath, "Deno.link()")?;
    permissions.check_read(&newpath, "Deno.link()")?;
    permissions.check_write(&newpath, "Deno.link()")?;
  }

  tokio::task::spawn_blocking(move || {
    let err_mapper = |err| {
      default_err_mapper(
        err,
        format!("link '{}' -> '{}'", oldpath.display(), newpath.display()),
      )
    };
    std::fs::hard_link(&oldpath, &newpath).map_err(err_mapper)?;
    Ok(())
  })
  .await
  .unwrap()
}

#[op]
fn op_symlink_sync<P>(
  state: &mut OpState,
  oldpath: &str,
  newpath: &str,
  _type: Option<String>,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(oldpath);
  let newpath = PathBuf::from(newpath);

  state
    .borrow_mut::<P>()
    .check_write_all("Deno.symlinkSync()")?;
  state
    .borrow_mut::<P>()
    .check_read_all("Deno.symlinkSync()")?;

  let err_mapper = |err| {
    default_err_mapper(
      err,
      format!("symlink '{}' -> '{}'", oldpath.display(), newpath.display()),
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
    use std::os::windows::fs::symlink_dir;
    use std::os::windows::fs::symlink_file;

    match _type {
      Some(ty) => match ty.as_ref() {
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
async fn op_symlink_async<P>(
  state: Rc<RefCell<OpState>>,
  oldpath: String,
  newpath: String,
  _type: Option<String>,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let oldpath = PathBuf::from(&oldpath);
  let newpath = PathBuf::from(&newpath);

  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write_all("Deno.symlink()")?;
    state.borrow_mut::<P>().check_read_all("Deno.symlink()")?;
  }

  tokio::task::spawn_blocking(move || {
    let err_mapper = |err| default_err_mapper(err, format!(
      "symlink '{}' -> '{}'",
      oldpath.display(),
      newpath.display()
    ));

    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(&oldpath, &newpath).map_err(err_mapper)?;
      Ok(())
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::{symlink_dir, symlink_file};

      match _type {
        Some(ty) => match ty.as_ref() {
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
fn op_read_link_sync<P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  state
    .borrow_mut::<P>()
    .check_read(&path, "Deno.readLink()")?;

  debug!("op_read_link_value {}", path.display());
  let target = std::fs::read_link(&path)
    .map_err(|err| {
      default_err_mapper(err, format!("readlink '{}'", path.display()))
    })?
    .into_os_string();
  let targetstr = into_string(target)?;
  Ok(targetstr)
}

#[op]
async fn op_read_link_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);
  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(&path, "Deno.readLink()")?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_read_link_async {}", path.display());
    let target = std::fs::read_link(&path)
      .map_err(|err| {
        default_err_mapper(err, format!("readlink '{}'", path.display()))
      })?
      .into_os_string();
    let targetstr = into_string(target)?;
    Ok(targetstr)
  })
  .await
  .unwrap()
}

#[op]
fn op_ftruncate_sync(
  state: &mut OpState,
  rid: u32,
  len: i32,
) -> Result<(), AnyError> {
  let len = len as u64;
  StdFileResource::with_file(state, rid, |std_file| {
    std_file.set_len(len).map_err(AnyError::from)
  })?;
  Ok(())
}

#[op]
async fn op_ftruncate_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  len: i32,
) -> Result<(), AnyError> {
  let len = len as u64;

  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    std_file.set_len(len)?;
    Ok(())
  })
  .await
}

#[op]
fn op_truncate_sync<P>(
  state: &mut OpState,
  path: &str,
  len: u64,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);

  state
    .borrow_mut::<P>()
    .check_write(&path, "Deno.truncateSync()")?;

  debug!("op_truncate_sync {} {}", path.display(), len);
  let err_mapper =
    |err| default_err_mapper(err, format!("truncate '{}'", path.display()));
  let f = std::fs::OpenOptions::new()
    .write(true)
    .open(&path)
    .map_err(err_mapper)?;
  f.set_len(len).map_err(err_mapper)?;
  Ok(())
}

#[op]
async fn op_truncate_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  len: u64,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);

  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_write(&path, "Deno.truncate()")?;
  }
  tokio::task::spawn_blocking(move || {
    debug!("op_truncate_async {} {}", path.display(), len);
    let err_mapper =
      |err| default_err_mapper(err, format!("truncate '{}'", path.display()));
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
    buf.set_file_name(format!("{prefix_}{unique:08x}{suffix_}"));
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
fn op_make_temp_dir_sync<P>(
  state: &mut OpState,
  args: MakeTempArgs,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state.borrow_mut::<P>().check_write(
    dir.clone().unwrap_or_else(temp_dir).as_path(),
    "Deno.makeTempDirSync()",
  )?;

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
async fn op_make_temp_dir_async<P>(
  state: Rc<RefCell<OpState>>,
  args: MakeTempArgs,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(
      dir.clone().unwrap_or_else(temp_dir).as_path(),
      "Deno.makeTempDir()",
    )?;
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
fn op_make_temp_file_sync<P>(
  state: &mut OpState,
  args: MakeTempArgs,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);

  state.borrow_mut::<P>().check_write(
    dir.clone().unwrap_or_else(temp_dir).as_path(),
    "Deno.makeTempFileSync()",
  )?;

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
async fn op_make_temp_file_async<P>(
  state: Rc<RefCell<OpState>>,
  args: MakeTempArgs,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let dir = args.dir.map(|s| PathBuf::from(&s));
  let prefix = args.prefix.map(String::from);
  let suffix = args.suffix.map(String::from);
  {
    let mut state = state.borrow_mut();
    state.borrow_mut::<P>().check_write(
      dir.clone().unwrap_or_else(temp_dir).as_path(),
      "Deno.makeTempFile()",
    )?;
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

#[op]
fn op_futime_sync(
  state: &mut OpState,
  rid: ResourceId,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError> {
  let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
  let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);

  StdFileResource::with_file(state, rid, |std_file| {
    filetime::set_file_handle_times(std_file, Some(atime), Some(mtime))
      .map_err(AnyError::from)
  })?;

  Ok(())
}

#[op]
async fn op_futime_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError> {
  let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
  let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);

  StdFileResource::with_file_blocking_task(state, rid, move |std_file| {
    filetime::set_file_handle_times(std_file, Some(atime), Some(mtime))?;
    Ok(())
  })
  .await
}

#[op]
fn op_utime_sync<P>(
  state: &mut OpState,
  path: &str,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(path);
  let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
  let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);

  state.borrow_mut::<P>().check_write(&path, "Deno.utime()")?;
  filetime::set_file_times(&path, atime, mtime).map_err(|err| {
    default_err_mapper(err, format!("utime '{}'", path.display()))
  })?;
  Ok(())
}

#[op]
async fn op_utime_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  atime_secs: i64,
  atime_nanos: u32,
  mtime_secs: i64,
  mtime_nanos: u32,
) -> Result<(), AnyError>
where
  P: FsPermissions + 'static,
{
  let path = PathBuf::from(&path);
  let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
  let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);

  state
    .borrow_mut()
    .borrow_mut::<P>()
    .check_write(&path, "Deno.utime()")?;

  tokio::task::spawn_blocking(move || {
    filetime::set_file_times(&path, atime, mtime).map_err(|err| {
      default_err_mapper(err, format!("utime '{}'", path.display()))
    })?;
    Ok(())
  })
  .await
  .unwrap()
}

#[op]
fn op_cwd<P>(state: &mut OpState) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = current_dir()?;
  state
    .borrow_mut::<P>()
    .check_read_blind(&path, "CWD", "Deno.cwd()")?;
  let path_str = into_string(path.into_os_string())?;
  Ok(path_str)
}

#[op]
fn op_readfile_sync<P>(
  state: &mut OpState,
  path: String,
) -> Result<ZeroCopyBuf, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&path);
  state
    .borrow_mut::<P>()
    .check_read(path, "Deno.readFileSync()")?;
  Ok(
    std::fs::read(path)
      .map_err(|err| {
        default_err_mapper(err, format!("readfile '{}'", path.display()))
      })?
      .into(),
  )
}

#[op]
fn op_readfile_text_sync<P>(
  state: &mut OpState,
  path: String,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  let path = Path::new(&path);
  state
    .borrow_mut::<P>()
    .check_read(path, "Deno.readTextFileSync()")?;
  Ok(string_from_utf8_lossy(std::fs::read(path).map_err(
    |err| default_err_mapper(err, format!("readfile '{}'", path.display())),
  )?))
}

#[op]
async fn op_readfile_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  cancel_rid: Option<ResourceId>,
) -> Result<ZeroCopyBuf, AnyError>
where
  P: FsPermissions + 'static,
{
  {
    let path = Path::new(&path);
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(path, "Deno.readFile()")?;
  }

  let read_future = tokio::task::spawn_blocking(move || {
    let path = Path::new(&path);
    Ok(std::fs::read(path).map(ZeroCopyBuf::from).map_err(|err| {
      default_err_mapper(err, format!("readfile '{}'", path.display()))
    })?)
  });

  let cancel_handle = cancel_rid.and_then(|rid| {
    state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(rid)
      .ok()
  });

  if let Some(cancel_handle) = cancel_handle {
    let read_future_rv = read_future.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    return read_future_rv??;
  }

  read_future.await?
}

#[op]
async fn op_readfile_text_async<P>(
  state: Rc<RefCell<OpState>>,
  path: String,
  cancel_rid: Option<ResourceId>,
) -> Result<String, AnyError>
where
  P: FsPermissions + 'static,
{
  {
    let path = Path::new(&path);
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<P>()
      .check_read(path, "Deno.readTextFile()")?;
  }

  let read_future = tokio::task::spawn_blocking(move || {
    let path = Path::new(&path);
    Ok(string_from_utf8_lossy(std::fs::read(path).map_err(
      |err| default_err_mapper(err, format!("readfile '{}'", path.display())),
    )?))
  });

  let cancel_handle = cancel_rid.and_then(|rid| {
    state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(rid)
      .ok()
  });

  if let Some(cancel_handle) = cancel_handle {
    let read_future_rv = read_future.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      state.borrow_mut().resource_table.close(cancel_rid).ok();
    };

    return read_future_rv??;
  }

  read_future.await?
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
