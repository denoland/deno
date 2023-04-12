// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::Resource;
use serde::Deserialize;
use serde::Serialize;
use tokio::task::JoinError;

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
  fn check_write_blind(
    &mut self,
    p: &Path,
    display: &str,
    api_name: &str,
  ) -> Result<(), AnyError>;
}

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

  pub(crate) fn check<P: FsPermissions>(
    &self,
    permissions: &mut P,
    path: &Path,
    api_name: &str,
  ) -> Result<(), AnyError> {
    if self.read {
      permissions.check_read(path, api_name)?;
    }
    if self.write || self.append {
      permissions.check_write(path, api_name)?;
    }
    Ok(())
  }
}

pub struct FsStat {
  pub is_file: bool,
  pub is_directory: bool,
  pub is_symlink: bool,
  pub size: u64,

  pub mtime: Option<u64>,
  pub atime: Option<u64>,
  pub birthtime: Option<u64>,

  pub dev: u64,
  pub ino: u64,
  pub mode: u32,
  pub nlink: u64,
  pub uid: u32,
  pub gid: u32,
  pub rdev: u64,
  pub blksize: u64,
  pub blocks: u64,
}

#[derive(Deserialize)]
pub enum FsFileType {
  #[serde(rename = "file")]
  File,
  #[serde(rename = "dir")]
  Directory,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsDirEntry {
  pub name: String,
  pub is_file: bool,
  pub is_directory: bool,
  pub is_symlink: bool,
}

pub enum FsError {
  Io(io::Error),
  FileBusy,
  NotSupported,
}

impl From<io::Error> for FsError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl From<JoinError> for FsError {
  fn from(err: JoinError) -> Self {
    if err.is_cancelled() {
      todo!("async tasks must not be cancelled")
    }
    if err.is_panic() {
      std::panic::resume_unwind(err.into_panic()); // resume the panic on the main thread
    }
    unreachable!()
  }
}

impl From<FsError> for AnyError {
  fn from(err: FsError) -> Self {
    match err {
      FsError::Io(err) => AnyError::from(err),
      FsError::FileBusy => resource_unavailable(),
      FsError::NotSupported => not_supported(),
    }
  }
}

pub type FsResult<T> = Result<T, FsError>;

#[async_trait::async_trait(?Send)]
pub trait File {
  fn write_all_sync(self: Rc<Self>, buf: &[u8]) -> FsResult<()>;
  async fn write_all_async(self: Rc<Self>, buf: Vec<u8>) -> FsResult<()>;

  fn read_all_sync(self: Rc<Self>) -> FsResult<Vec<u8>>;
  async fn read_all_async(self: Rc<Self>) -> FsResult<Vec<u8>>;

  fn chmod_sync(self: Rc<Self>, pathmode: u32) -> FsResult<()>;
  async fn chmod_async(self: Rc<Self>, mode: u32) -> FsResult<()>;

  fn seek_sync(self: Rc<Self>, pos: io::SeekFrom) -> FsResult<u64>;
  async fn seek_async(self: Rc<Self>, pos: io::SeekFrom) -> FsResult<u64>;

  fn datasync_sync(self: Rc<Self>) -> FsResult<()>;
  async fn datasync_async(self: Rc<Self>) -> FsResult<()>;

  fn sync_sync(self: Rc<Self>) -> FsResult<()>;
  async fn sync_async(self: Rc<Self>) -> FsResult<()>;

  fn stat_sync(self: Rc<Self>) -> FsResult<FsStat>;
  async fn stat_async(self: Rc<Self>) -> FsResult<FsStat>;

  fn lock_sync(self: Rc<Self>, exclusive: bool) -> FsResult<()>;
  async fn lock_async(self: Rc<Self>, exclusive: bool) -> FsResult<()>;
  fn unlock_sync(self: Rc<Self>) -> FsResult<()>;
  async fn unlock_async(self: Rc<Self>) -> FsResult<()>;

  fn truncate_sync(self: Rc<Self>, len: u64) -> FsResult<()>;
  async fn truncate_async(self: Rc<Self>, len: u64) -> FsResult<()>;

  fn utime_sync(
    self: Rc<Self>,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
  async fn utime_async(
    self: Rc<Self>,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;
}

#[async_trait::async_trait(?Send)]
pub trait FileSystem: Clone {
  type File: File + Resource;

  fn cwd(&self) -> FsResult<PathBuf>;
  fn tmp_dir(&self) -> FsResult<PathBuf>;
  fn chdir(&self, path: impl AsRef<Path>) -> FsResult<()>;
  fn umask(&self, mask: Option<u32>) -> FsResult<u32>;

  fn open_sync(
    &self,
    path: impl AsRef<Path>,
    options: OpenOptions,
  ) -> FsResult<Self::File>;
  async fn open_async(
    &self,
    path: PathBuf,
    options: OpenOptions,
  ) -> FsResult<Self::File>;

  fn mkdir_sync(
    &self,
    path: impl AsRef<Path>,
    recusive: bool,
    mode: u32,
  ) -> FsResult<()>;
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recusive: bool,
    mode: u32,
  ) -> FsResult<()>;

  fn chmod_sync(&self, path: impl AsRef<Path>, mode: u32) -> FsResult<()>;
  async fn chmod_async(&self, path: PathBuf, mode: u32) -> FsResult<()>;

  fn chown_sync(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;
  async fn chown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()>;

  fn remove_sync(
    &self,
    path: impl AsRef<Path>,
    recursive: bool,
  ) -> FsResult<()>;
  async fn remove_async(&self, path: PathBuf, recursive: bool) -> FsResult<()>;

  fn copy_file_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()>;
  async fn copy_file_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn stat_sync(&self, path: impl AsRef<Path>) -> FsResult<FsStat>;
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat>;

  fn lstat_sync(&self, path: impl AsRef<Path>) -> FsResult<FsStat>;
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat>;

  fn realpath_sync(&self, path: impl AsRef<Path>) -> FsResult<PathBuf>;
  async fn realpath_async(&self, path: PathBuf) -> FsResult<PathBuf>;

  fn read_dir_sync(&self, path: impl AsRef<Path>) -> FsResult<Vec<FsDirEntry>>;
  async fn read_dir_async(&self, path: PathBuf) -> FsResult<Vec<FsDirEntry>>;

  fn rename_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()>;
  async fn rename_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn link_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()>;
  async fn link_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()>;

  fn symlink_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;
  async fn symlink_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()>;

  fn read_link_sync(&self, path: impl AsRef<Path>) -> FsResult<PathBuf>;
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf>;

  fn truncate_sync(&self, path: impl AsRef<Path>, len: u64) -> FsResult<()>;
  async fn truncate_async(&self, path: PathBuf, len: u64) -> FsResult<()>;

  fn utime_sync(
    &self,
    path: impl AsRef<Path>,
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

  fn write_file_sync(
    &self,
    path: impl AsRef<Path>,
    options: OpenOptions,
    data: &[u8],
  ) -> FsResult<()> {
    let file = self.open_sync(path, options)?;
    let file = Rc::new(file);
    if let Some(mode) = options.mode {
      file.clone().chmod_sync(mode)?;
    }
    file.write_all_sync(data)?;
    Ok(())
  }
  async fn write_file_async(
    &self,
    path: PathBuf,
    options: OpenOptions,
    data: Vec<u8>,
  ) -> FsResult<()> {
    let file = self.open_async(path, options).await?;
    let file = Rc::new(file);
    if let Some(mode) = options.mode {
      file.clone().chmod_async(mode).await?;
    }
    file.write_all_async(data).await?;
    Ok(())
  }

  fn read_file_sync(&self, path: impl AsRef<Path>) -> FsResult<Vec<u8>> {
    let options = OpenOptions::read();
    let file = self.open_sync(path, options)?;
    let file = Rc::new(file);
    let buf = file.read_all_sync()?;
    Ok(buf)
  }
  async fn read_file_async(&self, path: PathBuf) -> FsResult<Vec<u8>> {
    let options = OpenOptions::read();
    let file = self.clone().open_async(path, options).await?;
    let file = Rc::new(file);
    let buf = file.read_all_async().await?;
    Ok(buf)
  }
}
