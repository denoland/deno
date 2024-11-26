// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use deno_io::fs::File;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;

use crate::sync::MaybeSend;
use crate::sync::MaybeSync;

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
      mode,
    }
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsDirEntry {
  pub name: String,
  pub is_file: bool,
  pub is_directory: bool,
  pub is_symlink: bool,
}

#[allow(clippy::disallowed_types)]
pub type FileSystemRc = crate::sync::MaybeArc<dyn FileSystem>;

pub trait AccessCheckFn:
  for<'a> FnMut(
  bool,
  &'a Path,
  &'a OpenOptions,
) -> FsResult<std::borrow::Cow<'a, Path>>
{
}
impl<T> AccessCheckFn for T where
  T: for<'a> FnMut(
    bool,
    &'a Path,
    &'a OpenOptions,
  ) -> FsResult<std::borrow::Cow<'a, Path>>
{
}

pub type AccessCheckCb<'a> = &'a mut (dyn AccessCheckFn + 'a);

#[async_trait::async_trait(?Send)]
pub trait FileSystem: std::fmt::Debug + MaybeSend + MaybeSync {
  fn cwd(&self) -> FsResult<PathBuf>;
  fn tmp_dir(&self) -> FsResult<PathBuf>;
  fn chdir(&self, path: &Path) -> FsResult<()>;
  fn umask(&self, mask: Option<u32>) -> FsResult<u32>;

  fn open_sync(
    &self,
    path: &Path,
    options: OpenOptions,
    access_check: Option<AccessCheckCb>,
  ) -> FsResult<Rc<dyn File>>;
  async fn open_async<'a>(
    &'a self,
    path: PathBuf,
    options: OpenOptions,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Rc<dyn File>>;

  fn mkdir_sync(
    &self,
    path: &Path,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()>;
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()>;

  fn chmod_sync(&self, path: &Path, mode: u32) -> FsResult<()>;
  async fn chmod_async(&self, path: PathBuf, mode: u32) -> FsResult<()>;

  fn chown_sync(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;
  async fn chown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;

  fn lchown_sync(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;
  async fn lchown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;

  fn remove_sync(&self, path: &Path, recursive: bool) -> FsResult<()>;
  async fn remove_async(&self, path: PathBuf, recursive: bool) -> FsResult<()>;

  fn copy_file_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()>;
  async fn copy_file_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn cp_sync(&self, path: &Path, new_path: &Path) -> FsResult<()>;
  async fn cp_async(&self, path: PathBuf, new_path: PathBuf) -> FsResult<()>;

  fn stat_sync(&self, path: &Path) -> FsResult<FsStat>;
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat>;

  fn lstat_sync(&self, path: &Path) -> FsResult<FsStat>;
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat>;

  fn realpath_sync(&self, path: &Path) -> FsResult<PathBuf>;
  async fn realpath_async(&self, path: PathBuf) -> FsResult<PathBuf>;

  fn read_dir_sync(&self, path: &Path) -> FsResult<Vec<FsDirEntry>>;
  async fn read_dir_async(&self, path: PathBuf) -> FsResult<Vec<FsDirEntry>>;

  fn rename_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()>;
  async fn rename_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn link_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()>;
  async fn link_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn symlink_sync(
    &self,
    oldpath: &Path,
    newpath: &Path,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;
  async fn symlink_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;

  fn read_link_sync(&self, path: &Path) -> FsResult<PathBuf>;
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf>;

  fn truncate_sync(&self, path: &Path, len: u64) -> FsResult<()>;
  async fn truncate_async(&self, path: PathBuf, len: u64) -> FsResult<()>;

  fn utime_sync(
    &self,
    path: &Path,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
  async fn utime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  fn lutime_sync(
    &self,
    path: &Path,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
  async fn lutime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  fn write_file_sync(
    &self,
    path: &Path,
    options: OpenOptions,
    access_check: Option<AccessCheckCb>,
    data: &[u8],
  ) -> FsResult<()> {
    let file = self.open_sync(path, options, access_check)?;
    if let Some(mode) = options.mode {
      file.clone().chmod_sync(mode)?;
    }
    file.write_all_sync(data)?;
    Ok(())
  }
  async fn write_file_async<'a>(
    &'a self,
    path: PathBuf,
    options: OpenOptions,
    access_check: Option<AccessCheckCb<'a>>,
    data: Vec<u8>,
  ) -> FsResult<()> {
    let file = self.open_async(path, options, access_check).await?;
    if let Some(mode) = options.mode {
      file.clone().chmod_async(mode).await?;
    }
    file.write_all(data.into()).await?;
    Ok(())
  }

  fn read_file_sync(
    &self,
    path: &Path,
    access_check: Option<AccessCheckCb>,
  ) -> FsResult<Vec<u8>> {
    let options = OpenOptions::read();
    let file = self.open_sync(path, options, access_check)?;
    let buf = file.read_all_sync()?;
    Ok(buf)
  }
  async fn read_file_async<'a>(
    &'a self,
    path: PathBuf,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Vec<u8>> {
    let options = OpenOptions::read();
    let file = self.open_async(path, options, access_check).await?;
    let buf = file.read_all_async().await?;
    Ok(buf)
  }

  fn is_file_sync(&self, path: &Path) -> bool {
    self.stat_sync(path).map(|m| m.is_file).unwrap_or(false)
  }

  fn is_dir_sync(&self, path: &Path) -> bool {
    self
      .stat_sync(path)
      .map(|m| m.is_directory)
      .unwrap_or(false)
  }

  fn exists_sync(&self, path: &Path) -> bool {
    self.stat_sync(path).is_ok()
  }
  async fn exists_async(&self, path: PathBuf) -> FsResult<bool> {
    Ok(self.stat_async(path).await.is_ok())
  }

  fn read_text_file_lossy_sync(
    &self,
    path: &Path,
    access_check: Option<AccessCheckCb>,
  ) -> FsResult<String> {
    let buf = self.read_file_sync(path, access_check)?;
    Ok(string_from_utf8_lossy(buf))
  }
  async fn read_text_file_lossy_async<'a>(
    &'a self,
    path: PathBuf,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<String> {
    let buf = self.read_file_async(path, access_check).await?;
    Ok(string_from_utf8_lossy(buf))
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
