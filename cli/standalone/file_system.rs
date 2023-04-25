use std::path::Path;
use std::path::PathBuf;

use deno_runtime::deno_fs;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_fs::FsFileType;
use deno_runtime::deno_fs::FsResult;
use deno_runtime::deno_fs::FsStat;
use deno_runtime::deno_fs::OpenOptions;
use deno_runtime::deno_fs::StdFs;
use deno_runtime::deno_io;
use deno_runtime::deno_node::NodeFs;
use deno_runtime::deno_node::NodeFsMetadata;
use deno_runtime::deno_node::RealFs;

use crate::standalone::binary::NPM_VFS;

#[derive(Debug, Clone)]
pub struct DenoCompileFileSystem;

#[async_trait::async_trait(?Send)]
impl deno_fs::FileSystem for DenoCompileFileSystem {
  type File = deno_io::StdFileResource;

  fn cwd(&self) -> FsResult<PathBuf> {
    StdFs.cwd()
  }

  fn tmp_dir(&self) -> FsResult<PathBuf> {
    StdFs.tmp_dir()
  }

  fn chdir(&self, path: impl AsRef<Path>) -> FsResult<()> {
    StdFs.chdir(path)
  }

  fn umask(&self, mask: Option<u32>) -> FsResult<u32> {
    StdFs.umask(mask)
  }

  fn open_sync(
    &self,
    path: impl AsRef<Path>,
    options: OpenOptions,
  ) -> FsResult<Self::File> {
    StdFs.open_sync(path, options)
  }
  async fn open_async(
    &self,
    path: PathBuf,
    options: OpenOptions,
  ) -> FsResult<Self::File> {
    StdFs.open_async(path, options).await
  }

  fn mkdir_sync(
    &self,
    path: impl AsRef<Path>,
    recursive: bool,
    mode: u32,
  ) -> FsResult<()> {
    StdFs.mkdir_sync(path, recursive, mode)
  }
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recursive: bool,
    mode: u32,
  ) -> FsResult<()> {
    StdFs.mkdir_async(path, recursive, mode).await
  }

  fn chmod_sync(&self, path: impl AsRef<Path>, mode: u32) -> FsResult<()> {
    StdFs.chmod_sync(path, mode)
  }
  async fn chmod_async(&self, path: PathBuf, mode: u32) -> FsResult<()> {
    StdFs.chmod_async(path, mode).await
  }

  fn chown_sync(
    &self,
    path: impl AsRef<Path>,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    StdFs.chown_sync(path, uid, gid)
  }
  async fn chown_async(
    &self,
    path: PathBuf,
    uid: Option<u32>,
    gid: Option<u32>,
  ) -> FsResult<()> {
    StdFs.chown_async(path, uid, gid).await
  }

  fn remove_sync(
    &self,
    path: impl AsRef<Path>,
    recursive: bool,
  ) -> FsResult<()> {
    StdFs.remove_sync(path, recursive)
  }
  async fn remove_async(&self, path: PathBuf, recursive: bool) -> FsResult<()> {
    StdFs.remove_async(path, recursive).await
  }

  fn copy_file_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()> {
    StdFs.copy_file_sync(oldpath, newpath)
  }
  async fn copy_file_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    StdFs.copy_file_async(oldpath, newpath).await
  }

  fn stat_sync(&self, path: impl AsRef<Path>) -> FsResult<FsStat> {
    StdFs.stat_sync(path)
  }
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    StdFs.stat_async(path).await
  }

  fn lstat_sync(&self, path: impl AsRef<Path>) -> FsResult<FsStat> {
    StdFs.lstat_sync(path)
  }
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    StdFs.lstat_async(path).await
  }

  fn realpath_sync(&self, path: impl AsRef<Path>) -> FsResult<PathBuf> {
    StdFs.realpath_sync(path)
  }
  async fn realpath_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    StdFs.realpath_async(path).await
  }

  fn read_dir_sync(&self, path: impl AsRef<Path>) -> FsResult<Vec<FsDirEntry>> {
    StdFs.read_dir_sync(path)
  }
  async fn read_dir_async(&self, path: PathBuf) -> FsResult<Vec<FsDirEntry>> {
    StdFs.read_dir_async(path).await
  }

  fn rename_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()> {
    StdFs.rename_sync(oldpath, newpath)
  }
  async fn rename_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    StdFs.rename_async(oldpath, newpath).await
  }

  fn link_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) -> FsResult<()> {
    StdFs.link_sync(oldpath, newpath)
  }
  async fn link_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
  ) -> FsResult<()> {
    StdFs.link_async(oldpath, newpath).await
  }

  fn symlink_sync(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    StdFs.symlink_sync(oldpath, newpath, file_type)
  }
  async fn symlink_async(
    &self,
    oldpath: PathBuf,
    newpath: PathBuf,
    file_type: Option<FsFileType>,
  ) -> FsResult<()> {
    StdFs.symlink_async(oldpath, newpath, file_type).await
  }

  fn read_link_sync(&self, path: impl AsRef<Path>) -> FsResult<PathBuf> {
    StdFs.read_link_sync(path)
  }
  async fn read_link_async(&self, path: PathBuf) -> FsResult<PathBuf> {
    StdFs.read_link_async(path).await
  }

  fn truncate_sync(&self, path: impl AsRef<Path>, len: u64) -> FsResult<()> {
    StdFs.truncate_sync(path, len)
  }
  async fn truncate_async(&self, path: PathBuf, len: u64) -> FsResult<()> {
    StdFs.truncate_async(path, len).await
  }

  fn utime_sync(
    &self,
    path: impl AsRef<Path>,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    StdFs.utime_sync(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
  }
  async fn utime_async(
    &self,
    path: PathBuf,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()> {
    StdFs
      .utime_async(path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
  }
}

impl NodeFs for DenoCompileFileSystem {
  fn current_dir(&self) -> std::io::Result<PathBuf> {
    RealFs.current_dir()
  }

  fn metadata(&self, path: &Path) -> std::io::Result<NodeFsMetadata> {
    if NPM_VFS.is_path_within(path) {
      NPM_VFS.metadata(path).map(|metadata| NodeFsMetadata {
        is_file: metadata.is_file,
        is_dir: metadata.is_directory,
      })
    } else {
      RealFs.metadata(path)
    }
  }

  fn is_file(&self, path: &Path) -> bool {
    self.metadata(path).map(|m| m.is_file).unwrap_or(false)
  }

  fn is_dir(&self, path: &Path) -> bool {
    self.metadata(path).map(|m| m.is_dir).unwrap_or(false)
  }

  fn exists(&self, path: &Path) -> bool {
    self.metadata(path).is_ok()
  }

  fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
    if NPM_VFS.is_path_within(path) {
      NPM_VFS.read_to_string(path)
    } else {
      RealFs.read_to_string(path)
    }
  }

  fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    if NPM_VFS.is_path_within(path) {
      NPM_VFS.canonicalize(path)
    } else {
      RealFs.canonicalize(path)
    }
  }
}
