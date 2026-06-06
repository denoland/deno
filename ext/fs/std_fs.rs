// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods, reason = "file system implementation")]

use std::borrow::Cow;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::unsync::spawn_blocking;
use deno_io::StdFileResourceInner;
use deno_io::fs::File;
use deno_io::fs::FsError;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;
use deno_io::fs::FsStatFs;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;

use crate::FileSystem;
use crate::OpenOptions;
use crate::interface::FsDirEntry;
use crate::interface::FsFileType;

#[derive(Debug, Default, Clone)]
pub struct RealFs;

#[async_trait::async_trait(?Send)]
impl FileSystem for RealFs {
  fn cwd(&self) -> FsResult<PathBuf> {
    std::env::current_dir().map_err(Into::into)
  }

  fn tmp_dir(&self) -> FsResult<PathBuf> {
    Ok(std::env::temp_dir())
  }

  fn chdir(&self, path: &CheckedPath) -> FsResult<()> {
    std::env::set_current_dir(path).map_err(Into::into)
  }

  #[cfg(windows)]
  fn umask(&self, mask: Option<u32>) -> FsResult<u32> {
    unsafe extern "C" {
      fn _umask(mask: std::ffi::c_int) -> std::ffi::c_int;
    }
    // SAFETY: `_umask` is a Windows CRT function that sets the file mode
    // creation mask and returns the previous value.
    unsafe {
      let old = if let Some(mask) = mask {
        _umask(mask as std::ffi::c_int)
      } else {
        let prev = _umask(0);
        _umask(prev);
        prev
      };
      Ok(old as u32)
    }
  }

  #[cfg(unix)]
  fn umask(&self, mask: Option<u32>) -> FsResult<u32> {
    use nix::sys::stat::Mode;
    use nix::sys::stat::mode_t;
    use nix::sys::stat::umask;
    let r = if let Some(mask) = mask {
      // If mask provided, return previous.
      umask(Mode::from_bits_truncate(mask as mode_t))
    } else {
      // If no mask provided, we query the current. Requires two syscalls.
      let prev = umask(Mode::from_bits_truncate(0));
      let _ = umask(prev);
      prev
    };
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
      Ok(r.bits())
    }
    #[cfg(any(
      target_os = "macos",
      target_os = "openbsd",
      target_os = "freebsd"
    ))]
    {
      Ok(r.bits() as u32)
    }
  }

  fn open_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>> {
    let std_file = open_with_checked_path(options, path)?;
    Ok(Rc::new(StdFileResourceInner::file(
      std_file,
      Some(path.to_path_buf()),
    )))
  }
  async fn open_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>> {
    // Open on the blocking pool: opening a FIFO with O_RDONLY (or O_WRONLY)
    // blocks until the other end is opened, which would otherwise stall the
    // runtime thread.
    let std_file = spawn_blocking(move || {
      open_with_checked_path(options, &path.as_checked_path())
        .map(|f| (f, path))
    })
    .await??;
    let (std_file, path) = std_file;
    Ok(Rc::new(StdFileResourceInner::file(
      std_file,
      Some(path.to_path_buf()),
    )))
  }

  fn mkdir_sync(
    &self,
    path: &CheckedPath,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()> {
    mkdir(path, recursive, mode)
  }
  async fn mkdir_async(
    &self,
    path: CheckedPathBuf,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()> {
    spawn_blocking(move || mkdir(&path, recursive, mode)).await?
  }

  #[cfg(unix)]
  fn chmod_sync(&self, path: &CheckedPath, mode: u32) -> FsResult<()> {
    chmod(path, mode)
  }
  #[cfg(not(unix))]
  fn chmod_sync(&self, path: &CheckedPath, mode: i32) -> FsResult<()> {
    chmod(path, mode)
  }

  #[cfg(unix)]
  async fn chmod_async(&self, path: CheckedPathBuf, mode: u32) -> FsResult<()> {
    spawn_blocking(move || chmod(&path, mode)).await?
  }
  #[cfg(not(unix))]
  async fn chmod_async(&self, path: CheckedPathBuf, mode: i32) -> FsResult<()> {
    spawn_blocking(move || chmod(&path, mode)).await?
  }

  fn chown_sync(
    &self,
    path: &CheckedPath,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    chown(path, uid, gid)
  }
  async fn chown_async(
    &self,
    path: CheckedPathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    spawn_blocking(move || chown(&path, uid, gid)).await?
  }

  fn remove_sync(&self, path: &CheckedPath, recursive: bool) -> FsResult<()> {
    remove(path, recursive)
  }
  async fn remove_async(
    &self,
    path: CheckedPathBuf,
    recursive: bool,
  ) -> FsResult<()> {
    spawn_blocking(move || remove(&path, recursive)).await?
  }

  fn copy_file_sync(
    &self,
    from: &CheckedPath,
    to: &CheckedPath,
  ) -> FsResult<()> {
    copy_file(from, to)
  }
  async fn copy_file_async(
    &self,
    from: CheckedPathBuf,
    to: CheckedPathBuf,
  ) -> FsResult<()> {
    spawn_blocking(move || copy_file(&from, &to)).await?
  }

  fn cp_sync(&self, fro: &CheckedPath, to: &CheckedPath) -> FsResult<()> {
    cp(fro, to)
  }
  async fn cp_async(
    &self,
    fro: CheckedPathBuf,
    to: CheckedPathBuf,
  ) -> FsResult<()> {
    spawn_blocking(move || cp(&fro, &to)).await?
  }

  fn stat_sync(&self, path: &CheckedPath) -> FsResult<FsStat> {
    stat(path)
  }
  async fn stat_async(&self, path: CheckedPathBuf) -> FsResult<FsStat> {
    spawn_blocking(move || stat(&path)).await?
  }

  fn lstat_sync(&self, path: &CheckedPath) -> FsResult<FsStat> {
    lstat(path)
  }
  async fn lstat_async(&self, path: CheckedPathBuf) -> FsResult<FsStat> {
    spawn_blocking(move || lstat(&path)).await?
  }

  fn statfs_sync(
    &self,
    path: &CheckedPath,
    bigint: bool,
  ) -> FsResult<FsStatFs> {
    statfs(path, bigint)
  }
  async fn statfs_async(
    &self,
    path: CheckedPathBuf,
    bigint: bool,
  ) -> FsResult<FsStatFs> {
    spawn_blocking(move || statfs(&path, bigint)).await?
  }

  fn exists_sync(&self, path: &CheckedPath) -> bool {
    exists(path)
  }
  async fn exists_async(&self, path: CheckedPathBuf) -> FsResult<bool> {
    spawn_blocking(move || exists(&path))
      .await
      .map_err(Into::into)
  }

  fn realpath_sync(&self, path: &CheckedPath) -> FsResult<PathBuf> {
    realpath(path)
  }
  async fn realpath_async(&self, path: CheckedPathBuf) -> FsResult<PathBuf> {
    spawn_blocking(move || realpath(&path)).await?
  }

  fn read_dir_sync(&self, path: &CheckedPath) -> FsResult<Vec<FsDirEntry>> {
    read_dir(path)
  }
  async fn read_dir_async(
    &self,
    path: CheckedPathBuf,
  ) -> FsResult<Vec<FsDirEntry>> {
    spawn_blocking(move || read_dir(&path)).await?
  }

  fn rename_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
  ) -> FsResult<()> {
    fs::rename(oldpath, newpath).map_err(Into::into)
  }
  async fn rename_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
  ) -> FsResult<()> {
    spawn_blocking(move || fs::rename(oldpath, newpath))
      .await?
      .map_err(Into::into)
  }

  fn lchmod_sync(&self, path: &CheckedPath, mode: u32) -> FsResult<()> {
    lchmod(path, mode)
  }

  async fn lchmod_async(
    &self,
    path: CheckedPathBuf,
    mode: u32,
  ) -> FsResult<()> {
    spawn_blocking(move || lchmod(&path, mode)).await?
  }

  fn link_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
  ) -> FsResult<()> {
    fs::hard_link(oldpath, newpath).map_err(Into::into)
  }
  async fn link_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
  ) -> FsResult<()> {
    spawn_blocking(move || fs::hard_link(oldpath, newpath))
      .await?
      .map_err(Into::into)
  }

  fn symlink_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    symlink(oldpath, newpath, file_type)
  }
  async fn symlink_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    spawn_blocking(move || symlink(&oldpath, &newpath, file_type)).await?
  }

  fn read_link_sync(&self, path: &CheckedPath) -> FsResult<PathBuf> {
    fs::read_link(path).map_err(Into::into)
  }
  async fn read_link_async(&self, path: CheckedPathBuf) -> FsResult<PathBuf> {
    spawn_blocking(move || fs::read_link(path))
      .await?
      .map_err(Into::into)
  }

  fn rmdir_sync(&self, path: &CheckedPath) -> FsResult<()> {
    fs::remove_dir(path).map_err(Into::into)
  }
  async fn rmdir_async(&self, path: CheckedPathBuf) -> FsResult<()> {
    spawn_blocking(move || fs::remove_dir(path))
      .await?
      .map_err(Into::into)
  }

  fn truncate_sync(&self, path: &CheckedPath, len: u64) -> FsResult<()> {
    truncate(path, len)
  }
  async fn truncate_async(
    &self,
    path: CheckedPathBuf,
    len: u64,
  ) -> FsResult<()> {
    spawn_blocking(move || truncate(&path, len)).await?
  }

  fn utime_sync(
    &self,
    path: &CheckedPath,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
    let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);
    filetime::set_file_times(path, atime, mtime).map_err(Into::into)
  }
  async fn utime_async(
    &self,
    path: CheckedPathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
    let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);
    spawn_blocking(move || {
      filetime::set_file_times(path, atime, mtime).map_err(Into::into)
    })
    .await?
  }

  fn lutime_sync(
    &self,
    path: &CheckedPath,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
    let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);
    filetime::set_symlink_file_times(path, atime, mtime).map_err(Into::into)
  }

  async fn lutime_async(
    &self,
    path: CheckedPathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    let atime = filetime::FileTime::from_unix_time(atime_secs, atime_nanos);
    let mtime = filetime::FileTime::from_unix_time(mtime_secs, mtime_nanos);
    spawn_blocking(move || {
      filetime::set_symlink_file_times(path, atime, mtime).map_err(Into::into)
    })
    .await?
  }

  fn lchown_sync(
    &self,
    path: &CheckedPath,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    lchown(path, uid, gid)
  }

  async fn lchown_async(
    &self,
    path: CheckedPathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    spawn_blocking(move || lchown(&path, uid, gid)).await?
  }

  fn write_file_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
    data: &[u8],
  ) -> FsResult<()> {
    let mut file = open_with_checked_path(options, path)?;
    #[cfg(unix)]
    if let Some(mode) = options.mode {
      use std::os::unix::fs::PermissionsExt;
      file.set_permissions(fs::Permissions::from_mode(mode))?;
    }
    file.write_all(data)?;
    Ok(())
  }

  async fn write_file_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
    data: Box<[u8]>,
  ) -> FsResult<()> {
    let mut file = open_with_checked_path(options, &path.as_checked_path())?;
    spawn_blocking(move || {
      #[cfg(unix)]
      if let Some(mode) = options.mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
      }
      file.write_all(&data)?;
      Ok(())
    })
    .await?
  }

  fn read_file_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
  ) -> FsResult<Cow<'static, [u8]>> {
    let mut file = open_with_checked_path(options, path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(Cow::Owned(buf))
  }
  async fn read_file_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
  ) -> FsResult<Cow<'static, [u8]>> {
    let mut file = open_with_checked_path(options, &path.as_checked_path())?;
    spawn_blocking(move || {
      let mut buf = Vec::new();
      file.read_to_end(&mut buf)?;
      Ok::<_, FsError>(Cow::Owned(buf))
    })
    .await?
  }
}

fn mkdir(path: &Path, recursive: bool, mode: Option<u32>) -> FsResult<()> {
  let mut builder = fs::DirBuilder::new();
  builder.recursive(recursive);
  #[cfg(unix)]
  if let Some(mode) = mode {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(mode);
  }
  #[cfg(not(unix))]
  {
    _ = mode;
  }
  builder.create(path).map_err(Into::into)
}

#[cfg(unix)]
fn chmod(path: &Path, mode: u32) -> FsResult<()> {
  use std::os::unix::fs::PermissionsExt;
  let permissions = fs::Permissions::from_mode(mode);
  fs::set_permissions(path, permissions)?;
  Ok(())
}

#[cfg(not(unix))]
fn chmod(path: &Path, mode: i32) -> FsResult<()> {
  use std::os::windows::ffi::OsStrExt;

  // Windows chmod doesn't follow symlinks unlike the UNIX counterpart,
  // so we have to resolve the symlink manually
  let resolved_path = realpath(path)?;

  let wchar_path = resolved_path
    .as_os_str()
    .encode_wide()
    .chain(std::iter::once(0))
    .collect::<Vec<_>>();

  // SAFETY: `path` is a null-terminated string.
  let result = unsafe { libc::wchmod(wchar_path.as_ptr(), mode) };
  if result != 0 {
    return Err(io::Error::last_os_error().into());
  }
  Ok(())
}

#[cfg(unix)]
fn chown(path: &Path, uid: Option<u32>, gid: Option<u32>) -> FsResult<()> {
  use nix::unistd::Gid;
  use nix::unistd::Uid;
  use nix::unistd::chown;
  let owner = uid.map(Uid::from_raw);
  let group = gid.map(Gid::from_raw);
  let res = chown(path, owner, group);
  if let Err(err) = res {
    return Err(io::Error::from_raw_os_error(err as i32).into());
  }
  Ok(())
}

// TODO: implement chown for Windows
#[cfg(not(unix))]
fn chown(_path: &Path, _uid: Option<u32>, _gid: Option<u32>) -> FsResult<()> {
  Err(FsError::NotSupported)
}

#[cfg(target_os = "macos")]
fn lchmod(path: &Path, mode: u32) -> FsResult<()> {
  use std::os::unix::fs::OpenOptionsExt;
  use std::os::unix::fs::PermissionsExt;

  use libc::O_SYMLINK;

  let file = fs::OpenOptions::new()
    .write(true)
    .custom_flags(O_SYMLINK)
    .open(path)?;
  file.set_permissions(fs::Permissions::from_mode(mode))?;
  Ok(())
}

#[cfg(not(target_os = "macos"))]
fn lchmod(_path: &Path, _mode: u32) -> FsResult<()> {
  Err(FsError::NotSupported)
}

#[cfg(unix)]
fn lchown(path: &Path, uid: Option<u32>, gid: Option<u32>) -> FsResult<()> {
  use std::os::unix::ffi::OsStrExt;
  let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
  // -1 = leave unchanged
  let uid = uid
    .map(|uid| uid as libc::uid_t)
    .unwrap_or(-1i32 as libc::uid_t);
  let gid = gid
    .map(|gid| gid as libc::gid_t)
    .unwrap_or(-1i32 as libc::gid_t);
  // SAFETY: `c_path` is a valid C string and lives throughout this function call.
  let result = unsafe { libc::lchown(c_path.as_ptr(), uid, gid) };
  if result != 0 {
    return Err(io::Error::last_os_error().into());
  }
  Ok(())
}

// TODO: implement lchown for Windows
#[cfg(not(unix))]
fn lchown(_path: &Path, _uid: Option<u32>, _gid: Option<u32>) -> FsResult<()> {
  Err(FsError::NotSupported)
}

fn remove(path: &Path, recursive: bool) -> FsResult<()> {
  // TODO: this is racy. This should open fds, and then `unlink` those.
  let metadata = fs::symlink_metadata(path)?;

  let file_type = metadata.file_type();
  let res = if file_type.is_dir() {
    if recursive {
      fs::remove_dir_all(path)
    } else {
      fs::remove_dir(path)
    }
  } else if file_type.is_symlink() {
    #[cfg(unix)]
    {
      fs::remove_file(path)
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::prelude::MetadataExt;

      use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
      if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
        fs::remove_dir(path)
      } else {
        fs::remove_file(path)
      }
    }
  } else {
    fs::remove_file(path)
  };

  res.map_err(Into::into)
}

fn copy_file(from: &Path, to: &Path) -> FsResult<()> {
  // Guard against copying a file onto itself. Otherwise the destination is
  // opened with truncation (or unlinked) before the source is read, which
  // silently empties the file. Match `cp` behavior and error instead. The
  // `to` path is canonicalized first so the common case where it does not yet
  // exist fails fast and skips canonicalizing `from` entirely; the full check
  // only runs when overwriting an existing file, and it also catches
  // equivalent paths such as `./`, `..` and symlinks.
  if let Ok(to_real) = to.canonicalize()
    && let Ok(from_real) = from.canonicalize()
    && from_real == to_real
  {
    return Err(
      io::Error::new(
        io::ErrorKind::InvalidInput,
        "Source and destination paths refer to the same file",
      )
      .into(),
    );
  }

  #[cfg(target_os = "macos")]
  {
    use std::ffi::CString;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;

    use libc::clonefile;
    use libc::stat;
    use libc::unlink;

    let from_str = CString::new(from.as_os_str().as_encoded_bytes())
      .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let to_str = CString::new(to.as_os_str().as_encoded_bytes())
      .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    // SAFETY: `from` and `to` are valid C strings.
    // std::fs::copy does open() + fcopyfile() on macOS. We try to use
    // clonefile() instead, which is more efficient.
    unsafe {
      let mut st = std::mem::zeroed();
      let ret = stat(from_str.as_ptr(), &mut st);
      if ret != 0 {
        return Err(io::Error::last_os_error().into());
      }

      if st.st_size > 128 * 1024 {
        // Try unlink. If it fails, we are going to try clonefile() anyway.
        let _ = unlink(to_str.as_ptr());
        // Matches rust stdlib behavior for io::copy.
        // https://github.com/rust-lang/rust/blob/3fdd578d72a24d4efc2fe2ad18eec3b6ba72271e/library/std/src/sys/unix/fs.rs#L1613-L1616
        if clonefile(from_str.as_ptr(), to_str.as_ptr(), 0) == 0 {
          return Ok(());
        }
      } else {
        // Do a regular copy. fcopyfile() is an overkill for < 128KB
        // files.
        let mut buf = [0u8; 128 * 1024];
        let mut from_file = fs::File::open(from)?;
        let perm = from_file.metadata()?.permissions();

        let mut to_file = fs::OpenOptions::new()
          // create the file with the correct mode right away
          .mode(perm.mode())
          .write(true)
          .create(true)
          .truncate(true)
          .open(to)?;
        let writer_metadata = to_file.metadata()?;
        if writer_metadata.is_file() {
          // Set the correct file permissions, in case the file already existed.
          // Don't set the permissions on already existing non-files like
          // pipes/FIFOs or device nodes.
          to_file.set_permissions(perm)?;
        }
        loop {
          let nread = from_file.read(&mut buf)?;
          if nread == 0 {
            break;
          }
          to_file.write_all(&buf[..nread])?;
        }
        return Ok(());
      }
    }

    // clonefile() failed, fall back to std::fs::copy().
  }

  fs::copy(from, to)?;

  Ok(())
}

fn cp(from: &Path, to: &Path) -> FsResult<()> {
  fn cp_(source_meta: fs::Metadata, from: &Path, to: &Path) -> FsResult<()> {
    let ty = source_meta.file_type();
    if ty.is_dir() {
      #[allow(unused_mut, reason = "mutable on unix")]
      let mut builder = fs::DirBuilder::new();
      #[cfg(unix)]
      {
        use std::os::unix::fs::DirBuilderExt;
        use std::os::unix::fs::PermissionsExt;
        builder.mode(fs::symlink_metadata(from)?.permissions().mode());
      }

      // The target directory might already exists. If it does,
      // continue copying all entries instead of aborting.
      if let Err(err) = builder.create(to)
        && err.kind() != ErrorKind::AlreadyExists
      {
        return Err(FsError::Io(err));
      }

      let mut entries: Vec<_> = fs::read_dir(from)?
        .map(|res| res.map(|e| e.file_name()))
        .collect::<Result<_, _>>()?;

      entries.shrink_to_fit();
      let entry_count = entries.len();
      let parallelism = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(entry_count);
      let entries = std::sync::Mutex::new(entries.into_iter());

      std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(parallelism);
        for _ in 0..parallelism {
          handles.push(scope.spawn(|| -> io::Result<()> {
            loop {
              let Some(file_name) = entries.lock().unwrap().next() else {
                return Ok(());
              };
              let from_path = from.join(&file_name);
              let to_path = to.join(&file_name);
              let meta = fs::symlink_metadata(&from_path).map_err(|err| {
                io::Error::new(
                  err.kind(),
                  format!(
                    "failed to copy '{}' to '{}': {:?}",
                    from_path.display(),
                    to_path.display(),
                    err,
                  ),
                )
              })?;
              cp_(meta, &from_path, &to_path).map_err(|err| {
                io::Error::new(
                  err.kind(),
                  format!(
                    "failed to copy '{}' to '{}': {:?}",
                    from_path.display(),
                    to_path.display(),
                    err,
                  ),
                )
              })?;
            }
          }));
        }

        for handle in handles {
          handle.join().unwrap()?;
        }

        Ok::<_, io::Error>(())
      })?;

      return Ok(());
    } else if ty.is_symlink() {
      let from = std::fs::read_link(from)?;

      #[cfg(unix)]
      std::os::unix::fs::symlink(from, to)?;
      #[cfg(windows)]
      std::os::windows::fs::symlink_file(from, to)?;

      return Ok(());
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::FileTypeExt;
      if ty.is_socket() {
        return Err(
          io::Error::new(
            io::ErrorKind::InvalidInput,
            "sockets cannot be copied",
          )
          .into(),
        );
      }
    }

    // Ensure parent destination directory exists
    if let Some(parent) = to.parent() {
      fs::create_dir_all(parent)?;
    }

    copy_file(from, to)
  }

  #[cfg(target_os = "macos")]
  {
    // Just clonefile()
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    use libc::clonefile;
    use libc::unlink;

    let from_str = CString::new(from.as_os_str().as_bytes())
      .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let to_str = CString::new(to.as_os_str().as_bytes())
      .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    // SAFETY: `from` and `to` are valid C strings.
    unsafe {
      // Try unlink. If it fails, we are going to try clonefile() anyway.
      let _ = unlink(to_str.as_ptr());

      if clonefile(from_str.as_ptr(), to_str.as_ptr(), 0) == 0 {
        return Ok(());
      }
    }
  }

  let source_meta = fs::symlink_metadata(from)?;

  #[inline]
  fn is_identical(
    source_meta: &fs::Metadata,
    dest_meta: &fs::Metadata,
  ) -> bool {
    #[cfg(unix)]
    {
      use std::os::unix::fs::MetadataExt;
      source_meta.ino() == dest_meta.ino()
    }
    #[cfg(windows)]
    {
      use std::os::windows::fs::MetadataExt;
      // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information
      //
      // The identifier (low and high parts) and the volume serial number uniquely identify a file on a single computer.
      // To determine whether two open handles represent the same file, combine the identifier and the volume serial
      // number for each file and compare them.
      //
      // Use this code once file_index() and volume_serial_number() is stabalized
      // See: https://github.com/rust-lang/rust/issues/63010
      //
      // source_meta.file_index() == dest_meta.file_index()
      //   && source_meta.volume_serial_number()
      //     == dest_meta.volume_serial_number()
      source_meta.last_write_time() == dest_meta.last_write_time()
        && source_meta.creation_time() == dest_meta.creation_time()
    }
  }

  if let Ok(m) = fs::metadata(to)
    && m.is_dir()
  {
    // Only target sub dir when source is not a dir itself
    if let Ok(from_meta) = fs::metadata(from)
      && !from_meta.is_dir()
    {
      return cp_(
        source_meta,
        from,
        &to.join(from.file_name().ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::InvalidInput,
            "the source path is not a valid file",
          )
        })?),
      );
    }
  }

  if let Ok(m) = fs::symlink_metadata(to)
    && is_identical(&source_meta, &m)
  {
    return Err(
      io::Error::new(
        io::ErrorKind::InvalidInput,
        "the source and destination are the same file",
      )
      .into(),
    );
  }

  cp_(source_meta, from, to)
}

#[cfg(not(windows))]
fn stat(path: &Path) -> FsResult<FsStat> {
  let metadata = fs::metadata(path)?;
  Ok(FsStat::from_std(metadata))
}

#[cfg(windows)]
fn stat(path: &Path) -> FsResult<FsStat> {
  let file = open_for_stat_windows(path, false)?;
  let metadata = file.metadata()?;
  let mut fsstat = FsStat::from_std(metadata);
  deno_io::stat_extra(&file, &mut fsstat)?;
  Ok(fsstat)
}

#[cfg(not(windows))]
fn lstat(path: &Path) -> FsResult<FsStat> {
  let metadata = fs::symlink_metadata(path)?;
  Ok(FsStat::from_std(metadata))
}

#[cfg(windows)]
fn lstat(path: &Path) -> FsResult<FsStat> {
  let file = open_for_stat_windows(path, true)?;
  let metadata = file.metadata()?;
  let mut fsstat = FsStat::from_std(metadata);
  deno_io::stat_extra(&file, &mut fsstat)?;
  Ok(fsstat)
}

// Some Windows file system drivers (notably ImDisk-backed memory disks)
// reject `FILE_FLAG_BACKUP_SEMANTICS` for regular files and return
// `ERROR_INVALID_FUNCTION` (1). Deno passes that flag so `CreateFile` can
// open directories; on those drivers we transparently retry without it.
// See https://github.com/denoland/deno/issues/26257.
#[cfg(windows)]
fn open_for_stat_windows(
  path: &Path,
  do_not_follow_symlink: bool,
) -> io::Result<fs::File> {
  use std::os::windows::fs::OpenOptionsExt;

  use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
  use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

  let reparse_flag = if do_not_follow_symlink {
    FILE_FLAG_OPEN_REPARSE_POINT
  } else {
    0
  };

  let mut opts = fs::OpenOptions::new();
  opts.access_mode(0); // no read or write
  opts.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | reparse_flag);
  match opts.open(path) {
    Ok(file) => Ok(file),
    Err(err) if err.raw_os_error() == Some(ERROR_INVALID_FUNCTION) => {
      let mut fallback = fs::OpenOptions::new();
      fallback.access_mode(0);
      fallback.custom_flags(reparse_flag);
      fallback.open(path).map_err(|_| err)
    }
    Err(err) => Err(err),
  }
}

#[cfg(windows)]
const ERROR_INVALID_FUNCTION: i32 = 1;

fn statfs(path: &Path, bigint: bool) -> FsResult<FsStatFs> {
  #[cfg(unix)]
  {
    use std::os::unix::ffi::OsStrExt;

    let mut cpath = path.as_os_str().as_bytes().to_vec();
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
      Ok(FsStatFs {
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
      Ok(FsStatFs {
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
    let path = path.canonicalize()?;
    let root = path.ancestors().last().ok_or_else(|| {
      std::io::Error::new(ErrorKind::NotFound, "Path has no root.")
    })?;
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
    Ok(FsStatFs {
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
    Err(FsError::NotSupported)
  }
}

fn exists(path: &Path) -> bool {
  #[cfg(unix)]
  {
    use nix::unistd::AccessFlags;
    use nix::unistd::access;
    access(path, AccessFlags::F_OK).is_ok()
  }

  #[cfg(windows)]
  {
    fs::exists(path).unwrap_or(false)
  }
}

fn realpath(path: &Path) -> FsResult<PathBuf> {
  Ok(deno_path_util::strip_unc_prefix(path.canonicalize()?))
}

fn read_dir(path: &Path) -> FsResult<Vec<FsDirEntry>> {
  let entries = fs::read_dir(path)?
    .filter_map(|entry| {
      let entry = entry.ok()?;
      // Non-UTF-8 filenames are decoded lossily (invalid bytes become U+FFFD)
      // so that they still surface in directory listings; Node's default
      // utf8 readdir does the same. Previously these entries were silently
      // dropped, which made globSync/readdirSync invisibly skip such files.
      let name = entry.file_name().to_string_lossy().into_owned();
      let metadata = entry.file_type();
      macro_rules! method_or_false {
        ($method:ident) => {
          if let Ok(metadata) = &metadata {
            metadata.$method()
          } else {
            false
          }
        };
      }
      Some(FsDirEntry {
        name,
        is_file: method_or_false!(is_file),
        is_directory: method_or_false!(is_dir),
        is_symlink: method_or_false!(is_symlink),
      })
    })
    .collect();

  Ok(entries)
}

#[cfg(not(windows))]
fn symlink(
  oldpath: &Path,
  newpath: &Path,
  _file_type: Option<FsFileType>,
) -> FsResult<()> {
  std::os::unix::fs::symlink(oldpath, newpath)?;
  Ok(())
}

#[cfg(windows)]
fn symlink(
  oldpath: &Path,
  newpath: &Path,
  file_type: Option<FsFileType>,
) -> FsResult<()> {
  let file_type = match file_type {
    Some(file_type) => file_type,
    None => {
      let old_meta = fs::metadata(oldpath);
      match old_meta {
        Ok(metadata) => {
          if metadata.is_file() {
            FsFileType::File
          } else if metadata.is_dir() {
            FsFileType::Directory
          } else {
            return Err(FsError::Io(io::Error::new(
              io::ErrorKind::InvalidInput,
              "On Windows the target must be a file or directory",
            )));
          }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
          return Err(FsError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "On Windows an `options` argument is required if the target does not exist",
          )));
        }
        Err(err) => return Err(err.into()),
      }
    }
  };

  match file_type {
    FsFileType::File => {
      std::os::windows::fs::symlink_file(oldpath, newpath)?;
    }
    FsFileType::Directory => {
      std::os::windows::fs::symlink_dir(oldpath, newpath)?;
    }
    FsFileType::Junction => {
      junction::create(oldpath, newpath)?;
    }
  };

  Ok(())
}

fn truncate(path: &Path, len: u64) -> FsResult<()> {
  let file = fs::OpenOptions::new().write(true).open(path)?;
  file.set_len(len)?;
  Ok(())
}

fn open_options(options: OpenOptions) -> fs::OpenOptions {
  let mut open_options = fs::OpenOptions::new();
  if let Some(mode) = options.mode {
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
  if let Some(custom_flags) = options.custom_flags {
    #[cfg(unix)]
    {
      use std::os::unix::fs::OpenOptionsExt;
      open_options.custom_flags(custom_flags);
    }
    #[cfg(not(unix))]
    let _ = custom_flags;
  }
  open_options.read(options.read);
  open_options.create(options.create);
  open_options.write(options.write);
  // On Windows, truncate and create_new produce conflicting
  // dwCreationDisposition flags (TRUNCATE_EXISTING vs CREATE_NEW).
  // When create_new is set, the file must not exist, so truncation
  // is meaningless. Passing both can cause spurious "os error 0"
  // (ERROR_SUCCESS) failures and file corruption on Windows.
  open_options.truncate(options.truncate && !options.create_new);
  open_options.append(options.append);
  open_options.create_new(options.create_new);
  open_options
}

pub fn open_with_checked_path(
  opts: OpenOptions,
  path: &CheckedPath,
) -> FsResult<std::fs::File> {
  // Rust's std::fs::OpenOptions requires write or append when create is set.
  // However, POSIX allows O_RDONLY | O_CREAT (create the file if it doesn't
  // exist, then open for reading). Handle this by creating the file first
  // if needed, then opening for read only.
  if opts.create && !opts.write && !opts.append {
    if !path.exists() {
      let create_opts = OpenOptions {
        read: false,
        write: true,
        create: true,
        truncate: false,
        append: false,
        create_new: opts.create_new,
        custom_flags: None,
        mode: opts.mode,
      };
      // Create and immediately close the file
      drop(open_path_with_options(create_opts, path)?);
    }
    let read_opts = OpenOptions {
      read: true,
      write: false,
      create: false,
      truncate: false,
      append: false,
      create_new: false,
      custom_flags: opts.custom_flags,
      mode: None,
    };
    return Ok(open_path_with_options(read_opts, path)?);
  }
  Ok(open_path_with_options(opts, path)?)
}

// Open the path using the configured options. On Windows we set
// `FILE_FLAG_BACKUP_SEMANTICS` so directories can be opened, but a few
// filesystem drivers (notably ImDisk-backed memory disks) reject that flag
// for regular files and return `ERROR_INVALID_FUNCTION` (1). Retry without
// the flag in that case so reads/writes succeed on those volumes.
// See https://github.com/denoland/deno/issues/26257.
fn open_path_with_options(
  opts: OpenOptions,
  path: &CheckedPath,
) -> io::Result<fs::File> {
  let std_opts = open_options_for_checked_path(opts, path);
  match std_opts.open(path) {
    Ok(file) => Ok(file),
    #[cfg(windows)]
    Err(err) if err.raw_os_error() == Some(ERROR_INVALID_FUNCTION) => {
      let fallback = open_options_for_checked_path_no_backup(opts, path);
      fallback.open(path).map_err(|_| err)
    }
    // A canonicalized path is opened with `O_NOFOLLOW` (see
    // `open_options_for_checked_path`). If the final component is still a
    // symlink at this point it must be a broken/dangling link: its target was
    // removed after canonicalization resolved the parent directory, so the link
    // survived as the path tail. `O_NOFOLLOW` reports it as `ELOOP` ("Too many
    // levels of symbolic links"), which is misleading. Translate it to
    // `ENOENT` so the error matches reading a nonexistent file.
    // See https://github.com/denoland/deno/issues/29139.
    #[cfg(unix)]
    Err(err)
      if path.canonicalized() && err.raw_os_error() == Some(libc::ELOOP) =>
    {
      Err(io::Error::from_raw_os_error(libc::ENOENT))
    }
    Err(err) => Err(err),
  }
}

#[cfg(windows)]
fn open_options_for_checked_path_no_backup(
  options: OpenOptions,
  _path: &CheckedPath,
) -> fs::OpenOptions {
  // Same as open_options_for_checked_path but without
  // FILE_FLAG_BACKUP_SEMANTICS so drivers that reject it can still open
  // regular files. Cannot open directories without the flag, but for the
  // drivers in question that's acceptable.
  open_options(options)
}

#[inline(always)]
pub fn open_options_for_checked_path(
  options: OpenOptions,
  path: &CheckedPath,
) -> fs::OpenOptions {
  let mut opts: fs::OpenOptions = open_options(options);
  #[cfg(windows)]
  {
    _ = path; // not used on windows
    // allow opening directories
    use std::os::windows::fs::OpenOptionsExt;
    opts.custom_flags(
      windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS,
    );
  }

  #[cfg(unix)]
  if path.canonicalized() {
    // Don't follow symlinks on open -- we must always pass fully-resolved files
    // with the exception of /proc/ which is too special, and /dev/std* which might point to
    // proc.
    use std::os::unix::fs::OpenOptionsExt;
    match options.custom_flags {
      Some(flags) => {
        opts.custom_flags(flags | libc::O_NOFOLLOW);
      }
      None => {
        opts.custom_flags(libc::O_NOFOLLOW);
      }
    }
  }

  opts
}
