// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use core::str;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

use deno_io::fs::File;
use deno_io::fs::FsResult;
use deno_io::fs::FsStat;
use sys_traits::FsFile;
use sys_traits::FsFileSetPermissions;

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
#[derive(Debug, Serialize)]
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

#[derive(Debug)]
pub struct FsStatSlim {
  file_type: sys_traits::FileType,
  modified: Result<SystemTime, std::io::Error>,
}

impl FsStatSlim {
  pub fn from_std(metadata: &std::fs::Metadata) -> Self {
    Self {
      file_type: metadata.file_type().into(),
      modified: metadata.modified(),
    }
  }

  pub fn from_deno_fs_stat(data: &FsStat) -> Self {
    FsStatSlim {
      file_type: if data.is_file {
        sys_traits::FileType::File
      } else if data.is_directory {
        sys_traits::FileType::Dir
      } else if data.is_symlink {
        sys_traits::FileType::Symlink
      } else {
        sys_traits::FileType::Unknown
      },
      modified: data
        .mtime
        .map(|ms| SystemTime::UNIX_EPOCH + Duration::from_millis(ms))
        .ok_or_else(|| {
          std::io::Error::new(std::io::ErrorKind::InvalidData, "No mtime")
        }),
    }
  }
}

impl sys_traits::FsMetadataValue for FsStatSlim {
  #[inline]
  fn file_type(&self) -> sys_traits::FileType {
    self.file_type
  }

  fn modified(&self) -> Result<SystemTime, std::io::Error> {
    self
      .modified
      .as_ref()
      .map_err(|err| std::io::Error::new(err.kind(), err.to_string()))
  }
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
  ) -> FsResult<Cow<'static, [u8]>> {
    let options = OpenOptions::read();
    let file = self.open_sync(path, options, access_check)?;
    let buf = file.read_all_sync()?;
    Ok(buf)
  }
  async fn read_file_async<'a>(
    &'a self,
    path: PathBuf,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Cow<'static, [u8]>> {
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
  ) -> FsResult<Cow<'static, str>> {
    let buf = self.read_file_sync(path, access_check)?;
    Ok(string_from_cow_utf8_lossy(buf))
  }
  async fn read_text_file_lossy_async<'a>(
    &'a self,
    path: PathBuf,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Cow<'static, str>> {
    let buf = self.read_file_async(path, access_check).await?;
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

// todo(dsherret): this is temporary. Instead of using the `FileSystem` trait implementation
// in the CLI, the CLI should instead create it's own file system using `sys_traits` traits
// then that can implement the `FileSystem` trait. Then this `FileSystem` trait can stay here
// for use only for `ext/fs` and not the entire CLI.
#[derive(Debug, Clone)]
pub struct FsSysTraitsAdapter(pub FileSystemRc);

impl FsSysTraitsAdapter {
  pub fn new_real() -> Self {
    Self(crate::sync::new_rc(crate::RealFs))
  }
}

impl sys_traits::BaseFsHardLink for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    self
      .0
      .link_sync(src, dst)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRead for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_read(&self, path: &Path) -> std::io::Result<Cow<'static, [u8]>> {
    self
      .0
      .read_file_sync(path, None)
      .map_err(|err| err.into_io_error())
  }
}

#[derive(Debug)]
pub struct FsSysTraitsAdapterReadDirEntry {
  path: PathBuf,
  entry: FsDirEntry,
}

impl sys_traits::FsDirEntry for FsSysTraitsAdapterReadDirEntry {
  type Metadata = FsStatSlim;

  fn file_name(&self) -> Cow<std::ffi::OsStr> {
    Cow::Borrowed(self.entry.name.as_ref())
  }

  fn file_type(&self) -> std::io::Result<sys_traits::FileType> {
    if self.entry.is_file {
      Ok(sys_traits::FileType::File)
    } else if self.entry.is_directory {
      Ok(sys_traits::FileType::Dir)
    } else if self.entry.is_symlink {
      Ok(sys_traits::FileType::Symlink)
    } else {
      Ok(sys_traits::FileType::Unknown)
    }
  }

  fn metadata(&self) -> std::io::Result<Self::Metadata> {
    Ok(FsStatSlim {
      file_type: self.file_type().unwrap(),
      modified: Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "not supported",
      )),
    })
  }

  fn path(&self) -> Cow<Path> {
    Cow::Borrowed(&self.path)
  }
}

impl sys_traits::BaseFsReadDir for FsSysTraitsAdapter {
  type ReadDirEntry = FsSysTraitsAdapterReadDirEntry;

  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>>>,
  > {
    // todo(dsherret): needs to actually be iterable and not allocate a vector
    let entries = self
      .0
      .read_dir_sync(path)
      .map_err(|err| err.into_io_error())?;
    let parent_dir = path.to_path_buf();
    Ok(Box::new(entries.into_iter().map(move |entry| {
      Ok(FsSysTraitsAdapterReadDirEntry {
        path: parent_dir.join(&entry.name),
        entry,
      })
    })))
  }
}

impl sys_traits::BaseFsCanonicalize for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    self
      .0
      .realpath_sync(path)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsMetadata for FsSysTraitsAdapter {
  type Metadata = FsStatSlim;

  #[inline]
  fn base_fs_metadata(&self, path: &Path) -> std::io::Result<Self::Metadata> {
    self
      .0
      .stat_sync(path)
      .map(|data| FsStatSlim::from_deno_fs_stat(&data))
      .map_err(|err| err.into_io_error())
  }

  #[inline]
  fn base_fs_symlink_metadata(
    &self,
    path: &Path,
  ) -> std::io::Result<Self::Metadata> {
    self
      .0
      .lstat_sync(path)
      .map(|data| FsStatSlim::from_deno_fs_stat(&data))
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsCreateDirAll for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_create_dir_all(&self, path: &Path) -> std::io::Result<()> {
    self
      .0
      .mkdir_sync(path, true, None)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRemoveFile for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    self
      .0
      .remove_sync(path, false)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRename for FsSysTraitsAdapter {
  #[inline]
  fn base_fs_rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    self
      .0
      .rename_sync(from, to)
      .map_err(|err| err.into_io_error())
  }
}

pub struct FsFileAdapter(pub Rc<dyn File>);

impl FsFile for FsFileAdapter {}

impl FsFileSetPermissions for FsFileAdapter {
  #[inline]
  fn fs_file_set_permissions(&mut self, mode: u32) -> std::io::Result<()> {
    self
      .0
      .clone()
      .chmod_sync(mode)
      .map_err(|err| err.into_io_error())
  }
}

impl std::io::Read for FsFileAdapter {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    self
      .0
      .clone()
      .read_sync(buf)
      .map_err(|err| err.into_io_error())
  }
}

impl std::io::Seek for FsFileAdapter {
  fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
    self
      .0
      .clone()
      .seek_sync(pos)
      .map_err(|err| err.into_io_error())
  }
}

impl std::io::Write for FsFileAdapter {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self
      .0
      .clone()
      .write_sync(buf)
      .map_err(|err| err.into_io_error())
  }

  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    self
      .0
      .clone()
      .sync_sync()
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsOpen for FsSysTraitsAdapter {
  type File = FsFileAdapter;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &sys_traits::OpenOptions,
  ) -> std::io::Result<Self::File> {
    self
      .0
      .open_sync(
        path,
        OpenOptions {
          read: options.read,
          write: options.write,
          create: options.create,
          truncate: options.truncate,
          append: options.append,
          create_new: options.create_new,
          mode: options.mode,
        },
        None,
      )
      .map(FsFileAdapter)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::SystemRandom for FsSysTraitsAdapter {
  #[inline]
  fn sys_random(&self, buf: &mut [u8]) -> std::io::Result<()> {
    getrandom::getrandom(buf).map_err(|err| {
      std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
    })
  }
}

impl sys_traits::SystemTimeNow for FsSysTraitsAdapter {
  #[inline]
  fn sys_time_now(&self) -> SystemTime {
    SystemTime::now()
  }
}

impl sys_traits::ThreadSleep for FsSysTraitsAdapter {
  #[inline]
  fn thread_sleep(&self, dur: Duration) {
    std::thread::sleep(dur);
  }
}

impl sys_traits::BaseEnvVar for FsSysTraitsAdapter {
  fn base_env_var_os(
    &self,
    key: &std::ffi::OsStr,
  ) -> Option<std::ffi::OsString> {
    std::env::var_os(key)
  }
}
