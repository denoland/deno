// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_runtime::deno_fs::AccessCheckCb;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_fs::FsFileType;
use deno_runtime::deno_fs::OpenOptions;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io::fs::File;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;

use super::virtual_fs::FileBackedVfs;

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
      None,
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
    access_check: Option<AccessCheckCb>,
  ) -> FsResult<Rc<dyn File>> {
    if self.0.is_path_within(path) {
      Ok(self.0.open_file(path)?)
    } else {
      RealFs.open_sync(path, options, access_check)
    }
  }
  async fn open_async<'a>(
    &'a self,
    path: PathBuf,
    options: OpenOptions,
    access_check: Option<AccessCheckCb<'a>>,
  ) -> FsResult<Rc<dyn File>> {
    if self.0.is_path_within(&path) {
      Ok(self.0.open_file(&path)?)
    } else {
      RealFs.open_async(path, options, access_check).await
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

  fn lchown_sync(
    &self,
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.lchown_sync(path, uid, gid)
  }

  async fn lchown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs.lchown_async(path, uid, gid).await
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

  fn cp_sync(&self, from: &Path, to: &Path) -> FsResult<()> {
    self.error_if_in_vfs(to)?;

    RealFs.cp_sync(from, to)
  }
  async fn cp_async(&self, from: PathBuf, to: PathBuf) -> FsResult<()> {
    self.error_if_in_vfs(&to)?;

    RealFs.cp_async(from, to).await
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

  fn read_dir_names_sync(&self, path: &Path) -> FsResult<Vec<String>> {
    if self.0.is_path_within(path) {
      Ok(self.0.read_dir_names(path)?)
    } else {
      RealFs.read_dir_names_sync(path)
    }
  }
  async fn read_dir_names_async(&self, path: PathBuf) -> FsResult<Vec<String>> {
    if self.0.is_path_within(&path) {
      Ok(self.0.read_dir_names(&path)?)
    } else {
      RealFs.read_dir_names_async(path).await
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
      Ok(self.0.read_link(path)?)
    } else {
      RealFs.read_link_sync(path)
    }
  }
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    if self.0.is_path_within(&path) {
      Ok(self.0.read_link(&path)?)
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

  fn lutime_sync(
    &self,
    path: &Path,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.lutime_sync(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
  }
  async fn lutime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    self.error_if_in_vfs(&path)?;
    RealFs
      .lutime_async(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
  }
}
