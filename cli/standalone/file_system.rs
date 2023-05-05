// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::parking_lot::Mutex;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_fs::FsFileType;
use deno_runtime::deno_fs::OpenOptions;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io;
use deno_runtime::deno_io::fs::File;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;

use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::VirtualFile;

#[derive(Clone)]
struct DenoCompileFile {
  file: VirtualFile,
  pos: Arc<Mutex<u64>>,
  vfs: Arc<FileBackedVfs>,
}

impl DenoCompileFile {
  pub fn seek(&self, pos: SeekFrom) -> FsResult<u64> {
    match pos {
      SeekFrom::Start(pos) => {
        *self.pos.lock() = pos;
        Ok(pos)
      }
      SeekFrom::End(offset) => {
        if offset < 0 && -offset as u64 > self.file.len {
          Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "An attempt was made to move the file pointer before the beginning of the file.").into())
        } else {
          let mut current_pos = self.pos.lock();
          *current_pos = if offset >= 0 {
            self.file.len - (offset as u64)
          } else {
            self.file.len + (-offset as u64)
          };
          Ok(*current_pos)
        }
      }
      SeekFrom::Current(offset) => {
        let mut current_pos = self.pos.lock();
        if offset >= 0 {
          *current_pos += offset as u64;
        } else if -offset as u64 > *current_pos {
          return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "An attempt was made to move the file pointer before the beginning of the file.").into());
        } else {
          *current_pos -= offset as u64;
        }
        Ok(*current_pos)
      }
    }
  }

  pub fn pos(&self) -> u64 {
    *self.pos.lock()
  }
}

#[async_trait::async_trait(?Send)]
impl deno_io::fs::File for DenoCompileFile {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize> {
    self
      .vfs
      .read_file(&self.file, self.pos(), buf)
      .map_err(|err| err.into())
  }
  async fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)> {
    let inner = (*self).clone();
    tokio::task::spawn(async move {
      let nread = inner.vfs.read_file(&inner.file, inner.pos(), &mut buf)?;
      Ok((nread, buf))
    })
    .await?
  }

  fn write_sync(self: Rc<Self>, _buf: &[u8]) -> FsResult<usize> {
    Err(FsError::NotSupported)
  }
  async fn write(
    self: Rc<Self>,
    _buf: BufView,
  ) -> FsResult<deno_core::WriteOutcome> {
    Err(FsError::NotSupported)
  }

  fn write_all_sync(self: Rc<Self>, _buf: &[u8]) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn write_all(self: Rc<Self>, _buf: BufView) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn read_all_sync(self: Rc<Self>) -> FsResult<Vec<u8>> {
    Ok(self.vfs.read_file_all(&self.file)?)
  }
  async fn read_all_async(self: Rc<Self>) -> FsResult<Vec<u8>> {
    let inner = (*self).clone();
    tokio::task::spawn_blocking(move || inner.vfs.read_file_all(&inner.file))
      .await?
      .map_err(|err| err.into())
  }

  fn chmod_sync(self: Rc<Self>, _pathmode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chmod_async(self: Rc<Self>, _mode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn seek_sync(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos)
  }
  async fn seek_async(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos)
  }

  fn datasync_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn datasync_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn sync_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn sync_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn stat_sync(self: Rc<Self>) -> FsResult<FsStat> {
    Err(FsError::NotSupported)
  }
  async fn stat_async(self: Rc<Self>) -> FsResult<FsStat> {
    Err(FsError::NotSupported)
  }

  fn lock_sync(self: Rc<Self>, _exclusive: bool) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn lock_async(self: Rc<Self>, _exclusive: bool) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn unlock_sync(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn unlock_async(self: Rc<Self>) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn truncate_sync(self: Rc<Self>, _len: u64) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn truncate_async(self: Rc<Self>, _len: u64) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn utime_sync(
    self: Rc<Self>,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn utime_async(
    self: Rc<Self>,
    _atime_secs: i64,
    _atime_nanos: u32,
    _mtime_secs: i64,
    _mtime_nanos: u32,
  ) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  // lower level functionality
  fn as_stdio(self: Rc<Self>) -> FsResult<std::process::Stdio> {
    Err(FsError::NotSupported)
  }
  #[cfg(unix)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::unix::prelude::RawFd> {
    None
  }
  #[cfg(windows)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::windows::io::RawHandle> {
    None
  }
  fn try_clone_inner(self: Rc<Self>) -> FsResult<Rc<dyn File>> {
    Ok(self)
  }
}

#[derive(Debug, Clone)]
pub struct DenoCompileFileSystem(Arc<FileBackedVfs>);

impl DenoCompileFileSystem {
  pub fn new(vfs: FileBackedVfs) -> Self {
    Self(Arc::new(vfs))
  }

  fn error_if_in_vfs(&self, path: &Path) -> FsResult<()> {
    if self.0.is_path_within(path) {
      Err(FsError::NotSupported)
    } else {
      Ok(())
    }
  }

  fn copy_to_real_path(&self, oldpath: &Path, newpath: &Path) -> FsResult<()> {
    let old_file = self.0.file_entry(oldpath)?;
    let old_file_bytes = self.0.read_file_all(old_file)?;
    RealFs.write_file_sync(
      newpath,
      OpenOptions {
        read: false,
        write: true,
        create: true,
        truncate: true,
        append: false,
        create_new: false,
        mode: None,
      },
      &old_file_bytes,
    )
  }
}

#[async_trait::async_trait(?Send)]
impl FileSystem for DenoCompileFileSystem {
  fn cwd(&self) -> FsResult<PathBuf> {
    RealFs.cwd()
  }

  fn tmp_dir(&self) -> FsResult<PathBuf> {
    RealFs.tmp_dir()
  }

  fn chdir(&self, path: &Path) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.chdir(path)
  }

  fn umask(&self, mask: Option<u32>) -> FsResult<u32> {
    RealFs.umask(mask)
  }

  fn open_sync(
    &self,
    path: &Path,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>> {
    if self.0.is_path_within(path) {
      let file = self.0.file_entry(path)?;
      Ok(Rc::new(DenoCompileFile {
        file: file.clone(),
        vfs: self.0.clone(),
        pos: Default::default(),
      }))
    } else {
      RealFs.open_sync(path, options)
    }
  }
  async fn open_async(
    &self,
    path: PathBuf,
    options: OpenOptions,
  ) -> FsResult<Rc<dyn File>> {
    if self.0.is_path_within(&path) {
      let file = self.0.file_entry(&path)?;
      Ok(Rc::new(DenoCompileFile {
        file: file.clone(),
        vfs: self.0.clone(),
        pos: Default::default(),
      }))
    } else {
      RealFs.open_async(path, options).await
    }
  }

  fn mkdir_sync(
    &self,
    path: &Path,
    recursive: bool,
    mode: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.mkdir_sync(path, recursive, mode)
  }
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recursive: bool,
    mode: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.mkdir_async(path, recursive, mode).await
  }

  fn chmod_sync(&self, path: &Path, mode: u32) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.chmod_sync(path, mode)
  }
  async fn chmod_async(&self, path: PathBuf, mode: u32) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.chmod_async(path, mode).await
  }

  fn chown_sync(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.chown_sync(path, uid, gid)
  }
  async fn chown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.chown_async(path, uid, gid).await
  }

  fn remove_sync(&self, path: &Path, recursive: bool) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.remove_sync(path, recursive)
  }
  async fn remove_async(&self, path: PathBuf, recursive: bool) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.remove_async(path, recursive).await
  }

  fn copy_file_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()> {
    self.error_if_in_vfs(newpath)?;
    if self.0.is_path_within(oldpath) {
      self.copy_to_real_path(oldpath, newpath)
    } else {
      RealFs.copy_file_sync(oldpath, newpath)
    }
  }
  async fn copy_file_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&newpath)?;
    if self.0.is_path_within(&oldpath) {
      let fs = self.clone();
      tokio::task::spawn_blocking(move || {
        fs.copy_to_real_path(&oldpath, &newpath)
      })
      .await?
    } else {
      RealFs.copy_file_async(oldpath, newpath).await
    }
  }

  fn stat_sync(&self, path: &Path) -> FsResult<FsStat> {
    if self.0.is_path_within(path) {
      Ok(self.0.stat(path)?)
    } else {
      RealFs.stat_sync(path)
    }
  }
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    if self.0.is_path_within(&path) {
      Ok(self.0.stat(&path)?)
    } else {
      RealFs.stat_async(path).await
    }
  }

  fn lstat_sync(&self, path: &Path) -> FsResult<FsStat> {
    if self.0.is_path_within(path) {
      Ok(self.0.lstat(path)?)
    } else {
      RealFs.lstat_sync(path)
    }
  }
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    if self.0.is_path_within(&path) {
      Ok(self.0.lstat(&path)?)
    } else {
      RealFs.lstat_async(path).await
    }
  }

  fn realpath_sync(&self, path: &Path) -> FsResult<PathBuf> {
    if self.0.is_path_within(path) {
      Ok(self.0.canonicalize(path)?)
    } else {
      RealFs.realpath_sync(path)
    }
  }
  async fn realpath_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    if self.0.is_path_within(&path) {
      Ok(self.0.canonicalize(&path)?)
    } else {
      RealFs.realpath_async(path).await
    }
  }

  fn read_dir_sync(&self, path: &Path) -> FsResult<Vec<FsDirEntry>> {
    if self.0.is_path_within(path) {
      Ok(self.0.read_dir(path)?)
    } else {
      RealFs.read_dir_sync(path)
    }
  }
  async fn read_dir_async(&self, path: PathBuf) -> FsResult<Vec<FsDirEntry>> {
    if self.0.is_path_within(&path) {
      Ok(self.0.read_dir(&path)?)
    } else {
      RealFs.read_dir_async(path).await
    }
  }

  fn rename_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()> {
    self.error_if_in_vfs(oldpath)?;
    self.error_if_in_vfs(newpath)?;
    RealFs.rename_sync(oldpath, newpath)
  }
  async fn rename_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&oldpath)?;
    self.error_if_in_vfs(&newpath)?;
    RealFs.rename_async(oldpath, newpath).await
  }

  fn link_sync(&self, oldpath: &Path, newpath: &Path) -> FsResult<()> {
    self.error_if_in_vfs(oldpath)?;
    self.error_if_in_vfs(newpath)?;
    RealFs.link_sync(oldpath, newpath)
  }
  async fn link_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&oldpath)?;
    self.error_if_in_vfs(&newpath)?;
    RealFs.link_async(oldpath, newpath).await
  }

  fn symlink_sync(
    &self,
    oldpath: &Path,
    newpath: &Path,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(oldpath)?;
    self.error_if_in_vfs(newpath)?;
    RealFs.symlink_sync(oldpath, newpath, file_type)
  }
  async fn symlink_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&oldpath)?;
    self.error_if_in_vfs(&newpath)?;
    RealFs.symlink_async(oldpath, newpath, file_type).await
  }

  fn read_link_sync(&self, path: &Path) -> FsResult<PathBuf> {
    if self.0.is_path_within(path) {
      todo!() // todo
    } else {
      RealFs.read_link_sync(path)
    }
  }
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    if self.0.is_path_within(&path) {
      todo!() // todo
    } else {
      RealFs.read_link_async(path).await
    }
  }

  fn truncate_sync(&self, path: &Path, len: u64) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.truncate_sync(path, len)
  }
  async fn truncate_async(&self, path: PathBuf, len: u64) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.truncate_async(path, len).await
  }

  fn utime_sync(
    &self,
    path: &Path,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.utime_sync(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
  }
  async fn utime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs
      .utime_async(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
  }
}
