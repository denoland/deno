// Copyright 2018-2025 the Deno authors. MIT license.

use core::str;
use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;

use deno_io::fs::File;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Default, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct OpenOptions {
  pub read: bool,
  pub write: bool,
  pub create: bool,
  pub truncate: bool,
  pub append: bool,
  pub create_new: bool,
  pub custom_flags: Option<i32>,
  pub mode: Option<u32>,
}

impl OpenOptions {
  pub fn read() -> Self {
    Self {
      read: true,
      write: false,
      create: false,
      truncate: false,
      append: false,
      create_new: false,
      custom_flags: None,
      mode: None,
    }
  }

  pub fn write(
    create: bool,
    append: bool,
    create_new: bool,
    mode: Option<u32>,
  ) -> Self {
    Self {
      read: false,
      write: true,
      create,
      truncate: !append,
      append,
      create_new,
      custom_flags: None,
      mode,
    }
  }
}

impl From<i32> for OpenOptions {
  fn from(flags: i32) -> Self {
    let mut options = OpenOptions {
      ..Default::default()
    };
    let mut flags = flags;

    if (flags & libc::O_APPEND) == libc::O_APPEND {
      options.append = true;
      flags &= !libc::O_APPEND;
    }
    if (flags & libc::O_CREAT) == libc::O_CREAT {
      options.create = true;
      flags &= !libc::O_CREAT;
    }
    if (flags & libc::O_EXCL) == libc::O_EXCL {
      options.create_new = true;
      options.write = true;
      flags &= !libc::O_EXCL;
    }
    if (flags & libc::O_RDWR) == libc::O_RDWR {
      options.read = true;
      options.write = true;
      flags &= !libc::O_RDWR;
    }
    if (flags & libc::O_TRUNC) == libc::O_TRUNC {
      options.truncate = true;
      flags &= !libc::O_TRUNC;
    }
    if (flags & libc::O_WRONLY) == libc::O_WRONLY {
      options.write = true;
      flags &= !libc::O_WRONLY;
    }

    if flags != 0 {
      options.custom_flags = Some(flags);
    }

    if !options.append
      && !options.create
      && !options.create_new
      && !options.read
      && !options.truncate
      && !options.write
    {
      options.read = true;
    }

    Self { ..options }
  }
}

#[derive(Deserialize)]
pub enum FsFileType {
  #[serde(rename = "file")]
  File,
  #[serde(rename = "dir")]
  Directory,
  #[serde(rename = "junction")]
  Junction,
}

/// WARNING: This is part of the public JS Deno API.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsDirEntry {
  pub name: String,
  pub is_file: bool,
  pub is_directory: bool,
  pub is_symlink: bool,
}

#[allow(clippy::disallowed_types)]
pub type FileSystemRc = deno_maybe_sync::MaybeArc<dyn FileSystem>;

#[async_trait::async_trait(?Send)]
pub trait FileSystem: std::fmt::Debug + MaybeSend + MaybeSync {
  fn cwd(&self) -> FsResult<PathBuf>;
  fn tmp_dir(&self) -> FsResult<PathBuf>;
  fn chdir(&self, path: &CheckedPath) -> FsResult<()>;
  fn umask(&self, mask: Option<u32>) -> FsResult<u32>;

  fn open_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>>;
  async fn open_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>>;

  fn mkdir_sync(
    &self,
    path: &CheckedPath,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()>;
  async fn mkdir_async(
    &self,
    path: CheckedPathBuf,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()>;

  #[cfg(unix)]
  fn chmod_sync(&self, path: &CheckedPath, mode: u32) -> FsResult<()>;
  #[cfg(not(unix))]
  fn chmod_sync(&self, path: &CheckedPath, mode: i32) -> FsResult<()>;

  #[cfg(unix)]
  async fn chmod_async(&self, path: CheckedPathBuf, mode: u32) -> FsResult<()>;
  #[cfg(not(unix))]
  async fn chmod_async(&self, path: CheckedPathBuf, mode: i32) -> FsResult<()>;

  fn chown_sync(
    &self,
    path: &CheckedPath,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;
  async fn chown_async(
    &self,
    path: CheckedPathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;

  fn lchmod_sync(&self, path: &CheckedPath, mode: u32) -> FsResult<()>;
  async fn lchmod_async(&self, path: CheckedPathBuf, mode: u32)
  -> FsResult<()>;

  fn lchown_sync(
    &self,
    path: &CheckedPath,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;
  async fn lchown_async(
    &self,
    path: CheckedPathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;

  fn remove_sync(&self, path: &CheckedPath, recursive: bool) -> FsResult<()>;
  async fn remove_async(
    &self,
    path: CheckedPathBuf,
    recursive: bool,
  ) -> FsResult<()>;

  fn copy_file_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
  ) -> FsResult<()>;
  async fn copy_file_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
  ) -> FsResult<()>;

  fn cp_sync(&self, path: &CheckedPath, new_path: &CheckedPath)
  -> FsResult<()>;
  async fn cp_async(
    &self,
    path: CheckedPathBuf,
    new_path: CheckedPathBuf,
  ) -> FsResult<()>;

  fn stat_sync(&self, path: &CheckedPath) -> FsResult<FsStat>;
  async fn stat_async(&self, path: CheckedPathBuf) -> FsResult<FsStat>;

  fn lstat_sync(&self, path: &CheckedPath) -> FsResult<FsStat>;
  async fn lstat_async(&self, path: CheckedPathBuf) -> FsResult<FsStat>;

  fn realpath_sync(&self, path: &CheckedPath) -> FsResult<PathBuf>;
  async fn realpath_async(&self, path: CheckedPathBuf) -> FsResult<PathBuf>;

  fn read_dir_sync(&self, path: &CheckedPath) -> FsResult<Vec<FsDirEntry>>;
  async fn read_dir_async(
    &self,
    path: CheckedPathBuf,
  ) -> FsResult<Vec<FsDirEntry>>;

  fn rename_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
  ) -> FsResult<()>;
  async fn rename_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
  ) -> FsResult<()>;

  fn link_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
  ) -> FsResult<()>;
  async fn link_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
  ) -> FsResult<()>;

  fn symlink_sync(
    &self,
    oldpath: &CheckedPath,
    newpath: &CheckedPath,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;
  async fn symlink_async(
    &self,
    oldpath: CheckedPathBuf,
    newpath: CheckedPathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;

  fn read_link_sync(&self, path: &CheckedPath) -> FsResult<PathBuf>;
  async fn read_link_async(&self, path: CheckedPathBuf) -> FsResult<PathBuf>;

  fn truncate_sync(&self, path: &CheckedPath, len: u64) -> FsResult<()>;
  async fn truncate_async(
    &self,
    path: CheckedPathBuf,
    len: u64,
  ) -> FsResult<()>;

  fn utime_sync(
    &self,
    path: &CheckedPath,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
  async fn utime_async(
    &self,
    path: CheckedPathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  fn lutime_sync(
    &self,
    path: &CheckedPath,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
  async fn lutime_async(
    &self,
    path: CheckedPathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  fn write_file_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
    data: &[u8],
  ) -> FsResult<()> {
    let file = self.open_sync(path, options)?;
    if let Some(mode) = options.mode {
      file.clone().chmod_sync(mode)?;
    }
    file.write_all_sync(data)?;
    Ok(())
  }
  async fn write_file_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
    data: Vec<u8>,
  ) -> FsResult<()> {
    let file = self.open_async(path, options).await?;
    if let Some(mode) = options.mode {
      file.clone().chmod_async(mode).await?;
    }
    file.write_all(data.into()).await?;
    Ok(())
  }

  fn read_file_sync(
    &self,
    path: &CheckedPath,
    options: OpenOptions,
  ) -> FsResult<Cow<'static, [u8]>> {
    let file = self.open_sync(path, options)?;
    let buf = file.read_all_sync()?;
    Ok(buf)
  }
  async fn read_file_async<'a>(
    &'a self,
    path: CheckedPathBuf,
    options: OpenOptions,
  ) -> FsResult<Cow<'static, [u8]>> {
    let file = self.open_async(path, options).await?;
    let buf = file.read_all_async().await?;
    Ok(buf)
  }

  fn is_file_sync(&self, path: &CheckedPath) -> bool {
    self.stat_sync(path).map(|m| m.is_file).unwrap_or(false)
  }

  fn is_dir_sync(&self, path: &CheckedPath) -> bool {
    self
      .stat_sync(path)
      .map(|m| m.is_directory)
      .unwrap_or(false)
  }

  fn exists_sync(&self, path: &CheckedPath) -> bool;
  async fn exists_async(&self, path: CheckedPathBuf) -> FsResult<bool>;

  fn read_text_file_lossy_sync(
    &self,
    path: &CheckedPath,
  ) -> FsResult<Cow<'static, str>> {
    let buf = self.read_file_sync(path, OpenOptions::read())?;
    Ok(string_from_cow_utf8_lossy(buf))
  }
  async fn read_text_file_lossy_async<'a>(
    &'a self,
    path: CheckedPathBuf,
  ) -> FsResult<Cow<'static, str>> {
    let buf = self.read_file_async(path, OpenOptions::read()).await?;
    Ok(string_from_cow_utf8_lossy(buf))
  }
}

#[inline(always)]
fn string_from_cow_utf8_lossy(buf: Cow<'static, [u8]>) -> Cow<'static, str> {
  match buf {
    Cow::Owned(buf) => Cow::Owned(string_from_utf8_lossy(buf)),
    Cow::Borrowed(buf) => String::from_utf8_lossy(buf),
  }
}

// Like String::from_utf8_lossy but operates on owned values
#[inline(always)]
fn string_from_utf8_lossy(buf: Vec<u8>) -> String {
  match String::from_utf8_lossy(&buf) {
    // buf contained non-utf8 chars than have been patched
    Cow::Owned(s) => s,
    // SAFETY: if Borrowed then the buf only contains utf8 chars,
    // we do this instead of .into_owned() to avoid copying the input buf
    Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(buf) },
  }
}
