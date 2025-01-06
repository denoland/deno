// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

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
use sys_traits::boxed::BoxedFsDirEntry;
use sys_traits::boxed::BoxedFsMetadataValue;
use sys_traits::boxed::FsMetadataBoxed;
use sys_traits::boxed::FsReadDirBoxed;
use sys_traits::FsCopy;
use sys_traits::FsMetadata;

use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::FileBackedVfsDirEntry;
use super::virtual_fs::FileBackedVfsFile;
use super::virtual_fs::FileBackedVfsMetadata;
use super::virtual_fs::VfsFileSubDataKind;

#[derive(Debug, Clone)]
pub struct DenoCompileFileSystem(Arc<FileBackedVfs>);

impl DenoCompileFileSystem {
  pub fn new(vfs: Arc<FileBackedVfs>) -> Self {
    Self(vfs)
  }

  fn error_if_in_vfs(&self, path: &Path) -> FsResult<()> {
    if self.0.is_path_within(path) {
      Err(FsError::NotSupported)
    } else {
      Ok(())
    }
  }

  fn copy_to_real_path(
    &self,
    oldpath: &Path,
    newpath: &Path,
  ) -> std::io::Result<u64> {
    let old_file = self.0.file_entry(oldpath)?;
    let old_file_bytes =
      self.0.read_file_all(old_file, VfsFileSubDataKind::Raw)?;
    let len = old_file_bytes.len() as u64;
    RealFs
      .write_file_sync(
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
      .map_err(|err| err.into_io_error())?;
    Ok(len)
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
      Ok(Rc::new(self.0.open_file(path)?))
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
      Ok(Rc::new(self.0.open_file(&path)?))
    } else {
      RealFs.open_async(path, options, access_check).await
    }
  }

  fn mkdir_sync(
    &self,
    path: &Path,
    recursive: bool,
    mode: Option<u32>,
  ) -> FsResult<()> {
    self.error_if_in_vfs(path)?;
    RealFs.mkdir_sync(path, recursive, mode)
  }
  async fn mkdir_async(
    &self,
    path: PathBuf,
    recursive: bool,
    mode: Option<u32>,
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
      self
        .copy_to_real_path(oldpath, newpath)
        .map(|_| ())
        .map_err(FsError::Io)
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
          .map(|_| ())
          .map_err(FsError::Io)
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
      Ok(self.0.stat(path)?.as_fs_stat())
    } else {
      RealFs.stat_sync(path)
    }
  }
  async fn stat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    if self.0.is_path_within(&path) {
      Ok(self.0.stat(&path)?.as_fs_stat())
    } else {
      RealFs.stat_async(path).await
    }
  }

  fn lstat_sync(&self, path: &Path) -> FsResult<FsStat> {
    if self.0.is_path_within(path) {
      Ok(self.0.lstat(path)?.as_fs_stat())
    } else {
      RealFs.lstat_sync(path)
    }
  }
  async fn lstat_async(&self, path: PathBuf) -> FsResult<FsStat> {
    if self.0.is_path_within(&path) {
      Ok(self.0.lstat(&path)?.as_fs_stat())
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

impl sys_traits::BaseFsHardLink for DenoCompileFileSystem {
  #[inline]
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    self.link_sync(src, dst).map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRead for DenoCompileFileSystem {
  #[inline]
  fn base_fs_read(&self, path: &Path) -> std::io::Result<Cow<'static, [u8]>> {
    self
      .read_file_sync(path, None)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::FsMetadataValue for FileBackedVfsMetadata {
  fn file_type(&self) -> sys_traits::FileType {
    self.file_type
  }

  fn len(&self) -> u64 {
    self.len
  }

  fn accessed(&self) -> std::io::Result<SystemTime> {
    Err(not_supported("accessed time"))
  }

  fn created(&self) -> std::io::Result<SystemTime> {
    Err(not_supported("created time"))
  }

  fn changed(&self) -> std::io::Result<SystemTime> {
    Err(not_supported("changed time"))
  }

  fn modified(&self) -> std::io::Result<SystemTime> {
    Err(not_supported("modified time"))
  }

  fn dev(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn ino(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn mode(&self) -> std::io::Result<u32> {
    Ok(0)
  }

  fn nlink(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn uid(&self) -> std::io::Result<u32> {
    Ok(0)
  }

  fn gid(&self) -> std::io::Result<u32> {
    Ok(0)
  }

  fn rdev(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn blksize(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn blocks(&self) -> std::io::Result<u64> {
    Ok(0)
  }

  fn is_block_device(&self) -> std::io::Result<bool> {
    Ok(false)
  }

  fn is_char_device(&self) -> std::io::Result<bool> {
    Ok(false)
  }

  fn is_fifo(&self) -> std::io::Result<bool> {
    Ok(false)
  }

  fn is_socket(&self) -> std::io::Result<bool> {
    Ok(false)
  }

  fn file_attributes(&self) -> std::io::Result<u32> {
    Ok(0)
  }
}

fn not_supported(name: &str) -> std::io::Error {
  std::io::Error::new(
    ErrorKind::Unsupported,
    format!(
      "{} is not supported for an embedded deno compile file",
      name
    ),
  )
}

impl sys_traits::FsDirEntry for FileBackedVfsDirEntry {
  type Metadata = BoxedFsMetadataValue;

  fn file_name(&self) -> Cow<std::ffi::OsStr> {
    Cow::Borrowed(self.metadata.name.as_ref())
  }

  fn file_type(&self) -> std::io::Result<sys_traits::FileType> {
    Ok(self.metadata.file_type)
  }

  fn metadata(&self) -> std::io::Result<Self::Metadata> {
    Ok(BoxedFsMetadataValue(Box::new(self.metadata.clone())))
  }

  fn path(&self) -> Cow<Path> {
    Cow::Owned(self.parent_path.join(&self.metadata.name))
  }
}

impl sys_traits::BaseFsReadDir for DenoCompileFileSystem {
  type ReadDirEntry = BoxedFsDirEntry;

  fn base_fs_read_dir(
    &self,
    path: &Path,
  ) -> std::io::Result<
    Box<dyn Iterator<Item = std::io::Result<Self::ReadDirEntry>> + '_>,
  > {
    if self.0.is_path_within(path) {
      let entries = self.0.read_dir_with_metadata(path)?;
      Ok(Box::new(
        entries.map(|entry| Ok(BoxedFsDirEntry::new(entry))),
      ))
    } else {
      #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
      sys_traits::impls::RealSys.fs_read_dir_boxed(path)
    }
  }
}

impl sys_traits::BaseFsCanonicalize for DenoCompileFileSystem {
  #[inline]
  fn base_fs_canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    self.realpath_sync(path).map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsMetadata for DenoCompileFileSystem {
  type Metadata = BoxedFsMetadataValue;

  #[inline]
  fn base_fs_metadata(&self, path: &Path) -> std::io::Result<Self::Metadata> {
    if self.0.is_path_within(path) {
      Ok(BoxedFsMetadataValue::new(self.0.stat(path)?))
    } else {
      #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
      sys_traits::impls::RealSys.fs_metadata_boxed(path)
    }
  }

  #[inline]
  fn base_fs_symlink_metadata(
    &self,
    path: &Path,
  ) -> std::io::Result<Self::Metadata> {
    if self.0.is_path_within(path) {
      Ok(BoxedFsMetadataValue::new(self.0.lstat(path)?))
    } else {
      #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
      sys_traits::impls::RealSys.fs_symlink_metadata_boxed(path)
    }
  }
}

impl sys_traits::BaseFsCopy for DenoCompileFileSystem {
  #[inline]
  fn base_fs_copy(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
    self
      .error_if_in_vfs(to)
      .map_err(|err| err.into_io_error())?;
    if self.0.is_path_within(from) {
      self.copy_to_real_path(from, to)
    } else {
      #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
      sys_traits::impls::RealSys.fs_copy(from, to)
    }
  }
}

impl sys_traits::BaseFsCloneFile for DenoCompileFileSystem {
  fn base_fs_clone_file(
    &self,
    _from: &Path,
    _to: &Path,
  ) -> std::io::Result<()> {
    // will cause a fallback in the code that uses this
    Err(not_supported("cloning files"))
  }
}

impl sys_traits::BaseFsCreateDir for DenoCompileFileSystem {
  #[inline]
  fn base_fs_create_dir(
    &self,
    path: &Path,
    options: &sys_traits::CreateDirOptions,
  ) -> std::io::Result<()> {
    self
      .mkdir_sync(path, options.recursive, options.mode)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRemoveFile for DenoCompileFileSystem {
  #[inline]
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    self
      .remove_sync(path, false)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRename for DenoCompileFileSystem {
  #[inline]
  fn base_fs_rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
    self
      .rename_sync(from, to)
      .map_err(|err| err.into_io_error())
  }
}

pub enum FsFileAdapter {
  Real(sys_traits::impls::RealFsFile),
  Vfs(FileBackedVfsFile),
}

impl sys_traits::FsFile for FsFileAdapter {}

impl sys_traits::FsFileAsRaw for FsFileAdapter {
  #[cfg(windows)]
  fn fs_file_as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
    match self {
      Self::Real(file) => file.fs_file_as_raw_handle(),
      Self::Vfs(_) => None,
    }
  }

  #[cfg(unix)]
  fn fs_file_as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
    match self {
      Self::Real(file) => file.fs_file_as_raw_fd(),
      Self::Vfs(_) => None,
    }
  }
}

impl sys_traits::FsFileSyncData for FsFileAdapter {
  fn fs_file_sync_data(&mut self) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_sync_data(),
      Self::Vfs(_) => Ok(()),
    }
  }
}

impl sys_traits::FsFileSyncAll for FsFileAdapter {
  fn fs_file_sync_all(&mut self) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_sync_all(),
      Self::Vfs(_) => Ok(()),
    }
  }
}

impl sys_traits::FsFileSetPermissions for FsFileAdapter {
  #[inline]
  fn fs_file_set_permissions(&mut self, mode: u32) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_set_permissions(mode),
      Self::Vfs(_) => Ok(()),
    }
  }
}

impl std::io::Read for FsFileAdapter {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self {
      Self::Real(file) => file.read(buf),
      Self::Vfs(file) => file.read_to_buf(buf),
    }
  }
}

impl std::io::Seek for FsFileAdapter {
  fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
    match self {
      Self::Real(file) => file.seek(pos),
      Self::Vfs(file) => file.seek(pos),
    }
  }
}

impl std::io::Write for FsFileAdapter {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self {
      Self::Real(file) => file.write(buf),
      Self::Vfs(_) => Err(not_supported("writing files")),
    }
  }

  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.flush(),
      Self::Vfs(_) => Err(not_supported("writing files")),
    }
  }
}

impl sys_traits::FsFileSetLen for FsFileAdapter {
  #[inline]
  fn fs_file_set_len(&mut self, len: u64) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_set_len(len),
      Self::Vfs(_) => Err(not_supported("setting file length")),
    }
  }
}

impl sys_traits::FsFileSetTimes for FsFileAdapter {
  fn fs_file_set_times(
    &mut self,
    times: sys_traits::FsFileTimes,
  ) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_set_times(times),
      Self::Vfs(_) => Err(not_supported("setting file times")),
    }
  }
}

impl sys_traits::FsFileLock for FsFileAdapter {
  fn fs_file_lock(
    &mut self,
    mode: sys_traits::FsFileLockMode,
  ) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_lock(mode),
      Self::Vfs(_) => Err(not_supported("locking files")),
    }
  }

  fn fs_file_try_lock(
    &mut self,
    mode: sys_traits::FsFileLockMode,
  ) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_try_lock(mode),
      Self::Vfs(_) => Err(not_supported("locking files")),
    }
  }

  fn fs_file_unlock(&mut self) -> std::io::Result<()> {
    match self {
      Self::Real(file) => file.fs_file_unlock(),
      Self::Vfs(_) => Err(not_supported("unlocking files")),
    }
  }
}

impl sys_traits::FsFileIsTerminal for FsFileAdapter {
  #[inline]
  fn fs_file_is_terminal(&self) -> bool {
    match self {
      Self::Real(file) => file.fs_file_is_terminal(),
      Self::Vfs(_) => false,
    }
  }
}

impl sys_traits::BaseFsOpen for DenoCompileFileSystem {
  type File = FsFileAdapter;

  fn base_fs_open(
    &self,
    path: &Path,
    options: &sys_traits::OpenOptions,
  ) -> std::io::Result<Self::File> {
    if self.0.is_path_within(path) {
      Ok(FsFileAdapter::Vfs(self.0.open_file(path)?))
    } else {
      #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
      Ok(FsFileAdapter::Real(
        sys_traits::impls::RealSys.base_fs_open(path, options)?,
      ))
    }
  }
}

impl sys_traits::BaseFsSymlinkDir for DenoCompileFileSystem {
  fn base_fs_symlink_dir(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    self
      .symlink_sync(src, dst, Some(FsFileType::Directory))
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::SystemRandom for DenoCompileFileSystem {
  #[inline]
  fn sys_random(&self, buf: &mut [u8]) -> std::io::Result<()> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.sys_random(buf)
  }
}

impl sys_traits::SystemTimeNow for DenoCompileFileSystem {
  #[inline]
  fn sys_time_now(&self) -> SystemTime {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.sys_time_now()
  }
}

impl sys_traits::ThreadSleep for DenoCompileFileSystem {
  #[inline]
  fn thread_sleep(&self, dur: Duration) {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.thread_sleep(dur)
  }
}

impl sys_traits::EnvCurrentDir for DenoCompileFileSystem {
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.env_current_dir()
  }
}

impl sys_traits::BaseEnvVar for DenoCompileFileSystem {
  fn base_env_var_os(
    &self,
    key: &std::ffi::OsStr,
  ) -> Option<std::ffi::OsString> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.base_env_var_os(key)
  }
}
