// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::ResourceHandleFd;
use deno_lib::standalone::virtual_fs::FileSystemCaseSensitivity;
use deno_lib::standalone::virtual_fs::OffsetWithLength;
use deno_lib::standalone::virtual_fs::VfsEntry;
use deno_lib::standalone::virtual_fs::VfsEntryRef;
use deno_lib::standalone::virtual_fs::VirtualDirectory;
use deno_lib::standalone::virtual_fs::VirtualFile;
use deno_runtime::deno_fs::AccessCheckCb;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_fs::FsFileType;
use deno_runtime::deno_fs::OpenOptions;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io;
use deno_runtime::deno_io::fs::File as DenoFile;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;
use deno_runtime::deno_napi::DenoRtNativeAddonLoader;
use deno_runtime::deno_napi::DenoRtNativeAddonLoaderRc;
use sys_traits::boxed::BoxedFsDirEntry;
use sys_traits::boxed::BoxedFsMetadataValue;
use sys_traits::boxed::FsMetadataBoxed;
use sys_traits::boxed::FsReadDirBoxed;
use sys_traits::FsCopy;
use url::Url;

#[derive(Debug, Clone)]
pub struct DenoRtSys(Arc<FileBackedVfs>);

impl DenoRtSys {
  pub fn new(vfs: Arc<FileBackedVfs>) -> Self {
    Self(vfs)
  }

  pub fn as_deno_rt_native_addon_loader(&self) -> DenoRtNativeAddonLoaderRc {
    self.0.clone()
  }

  pub fn is_specifier_in_vfs(&self, specifier: &Url) -> bool {
    deno_path_util::url_to_file_path(specifier)
      .map(|p| self.is_in_vfs(&p))
      .unwrap_or(false)
  }

  pub fn is_in_vfs(&self, path: &Path) -> bool {
    self.0.is_path_within(path)
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
    let old_file_bytes = self.0.read_file_all(old_file)?;
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
impl FileSystem for DenoRtSys {
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
  ) -> FsResult<Rc<dyn DenoFile>> {
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
  ) -> FsResult<Rc<dyn DenoFile>> {
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

impl sys_traits::BaseFsHardLink for DenoRtSys {
  #[inline]
  fn base_fs_hard_link(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    self.link_sync(src, dst).map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRead for DenoRtSys {
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

impl sys_traits::BaseFsReadDir for DenoRtSys {
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

impl sys_traits::BaseFsCanonicalize for DenoRtSys {
  #[inline]
  fn base_fs_canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    self.realpath_sync(path).map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsMetadata for DenoRtSys {
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

impl sys_traits::BaseFsCopy for DenoRtSys {
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

impl sys_traits::BaseFsCloneFile for DenoRtSys {
  fn base_fs_clone_file(
    &self,
    _from: &Path,
    _to: &Path,
  ) -> std::io::Result<()> {
    // will cause a fallback in the code that uses this
    Err(not_supported("cloning files"))
  }
}

impl sys_traits::BaseFsCreateDir for DenoRtSys {
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

impl sys_traits::BaseFsRemoveFile for DenoRtSys {
  #[inline]
  fn base_fs_remove_file(&self, path: &Path) -> std::io::Result<()> {
    self
      .remove_sync(path, false)
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::BaseFsRename for DenoRtSys {
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

impl sys_traits::BaseFsOpen for DenoRtSys {
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

impl sys_traits::BaseFsSymlinkDir for DenoRtSys {
  fn base_fs_symlink_dir(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
    self
      .symlink_sync(src, dst, Some(FsFileType::Directory))
      .map_err(|err| err.into_io_error())
  }
}

impl sys_traits::SystemRandom for DenoRtSys {
  #[inline]
  fn sys_random(&self, buf: &mut [u8]) -> std::io::Result<()> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.sys_random(buf)
  }
}

impl sys_traits::SystemTimeNow for DenoRtSys {
  #[inline]
  fn sys_time_now(&self) -> SystemTime {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.sys_time_now()
  }
}

impl sys_traits::ThreadSleep for DenoRtSys {
  #[inline]
  fn thread_sleep(&self, dur: Duration) {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.thread_sleep(dur)
  }
}

impl sys_traits::EnvCurrentDir for DenoRtSys {
  fn env_current_dir(&self) -> std::io::Result<PathBuf> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.env_current_dir()
  }
}

impl sys_traits::BaseEnvVar for DenoRtSys {
  fn base_env_var_os(
    &self,
    key: &std::ffi::OsStr,
  ) -> Option<std::ffi::OsString> {
    #[allow(clippy::disallowed_types)] // ok because we're implementing the fs
    sys_traits::impls::RealSys.base_env_var_os(key)
  }
}

#[derive(Debug)]
pub struct VfsRoot {
  pub dir: VirtualDirectory,
  pub root_path: PathBuf,
  pub start_file_offset: u64,
}

impl VfsRoot {
  fn find_entry<'a>(
    &'a self,
    path: &Path,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    self.find_entry_inner(path, &mut HashSet::new(), case_sensitivity)
  }

  fn find_entry_inner<'a>(
    &'a self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    let mut path = Cow::Borrowed(path);
    loop {
      let (resolved_path, entry) =
        self.find_entry_no_follow_inner(&path, seen, case_sensitivity)?;
      match entry {
        VfsEntryRef::Symlink(symlink) => {
          if !seen.insert(path.to_path_buf()) {
            return Err(std::io::Error::new(
              std::io::ErrorKind::Other,
              "circular symlinks",
            ));
          }
          path = Cow::Owned(symlink.resolve_dest_from_root(&self.root_path));
        }
        _ => {
          return Ok((resolved_path, entry));
        }
      }
    }
  }

  fn find_entry_no_follow(
    &self,
    path: &Path,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> std::io::Result<(PathBuf, VfsEntryRef)> {
    self.find_entry_no_follow_inner(path, &mut HashSet::new(), case_sensitivity)
  }

  fn find_entry_no_follow_inner<'a>(
    &'a self,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> std::io::Result<(PathBuf, VfsEntryRef<'a>)> {
    let relative_path = match path.strip_prefix(&self.root_path) {
      Ok(p) => p,
      Err(_) => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "path not found",
        ));
      }
    };
    let mut final_path = self.root_path.clone();
    let mut current_entry = VfsEntryRef::Dir(&self.dir);
    for component in relative_path.components() {
      let component = component.as_os_str();
      let current_dir = match current_entry {
        VfsEntryRef::Dir(dir) => {
          final_path.push(component);
          dir
        }
        VfsEntryRef::Symlink(symlink) => {
          let dest = symlink.resolve_dest_from_root(&self.root_path);
          let (resolved_path, entry) =
            self.find_entry_inner(&dest, seen, case_sensitivity)?;
          final_path = resolved_path; // overwrite with the new resolved path
          match entry {
            VfsEntryRef::Dir(dir) => {
              final_path.push(component);
              dir
            }
            _ => {
              return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "path not found",
              ));
            }
          }
        }
        _ => {
          return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path not found",
          ));
        }
      };
      let component = component.to_string_lossy();
      current_entry = current_dir
        .entries
        .get_by_name(&component, case_sensitivity)
        .ok_or_else(|| {
          std::io::Error::new(std::io::ErrorKind::NotFound, "path not found")
        })?
        .as_ref();
    }

    Ok((final_path, current_entry))
  }
}

pub struct FileBackedVfsFile {
  file: VirtualFile,
  pos: RefCell<u64>,
  vfs: Arc<FileBackedVfs>,
}

impl FileBackedVfsFile {
  pub fn seek(&self, pos: SeekFrom) -> std::io::Result<u64> {
    match pos {
      SeekFrom::Start(pos) => {
        *self.pos.borrow_mut() = pos;
        Ok(pos)
      }
      SeekFrom::End(offset) => {
        if offset < 0 && -offset as u64 > self.file.offset.len {
          let msg = "An attempt was made to move the file pointer before the beginning of the file.";
          Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            msg,
          ))
        } else {
          let mut current_pos = self.pos.borrow_mut();
          *current_pos = if offset >= 0 {
            self.file.offset.len - (offset as u64)
          } else {
            self.file.offset.len + (-offset as u64)
          };
          Ok(*current_pos)
        }
      }
      SeekFrom::Current(offset) => {
        let mut current_pos = self.pos.borrow_mut();
        if offset >= 0 {
          *current_pos += offset as u64;
        } else if -offset as u64 > *current_pos {
          return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "An attempt was made to move the file pointer before the beginning of the file."));
        } else {
          *current_pos -= -offset as u64;
        }
        Ok(*current_pos)
      }
    }
  }

  pub fn read_to_buf(&self, buf: &mut [u8]) -> std::io::Result<usize> {
    let read_pos = {
      let mut pos = self.pos.borrow_mut();
      let read_pos = *pos;
      // advance the position due to the read
      *pos = std::cmp::min(self.file.offset.len, *pos + buf.len() as u64);
      read_pos
    };
    self.vfs.read_file(&self.file, read_pos, buf)
  }

  fn read_to_end(&self) -> FsResult<Cow<'static, [u8]>> {
    let read_pos = {
      let mut pos = self.pos.borrow_mut();
      let read_pos = *pos;
      // todo(dsherret): should this always set it to the end of the file?
      if *pos < self.file.offset.len {
        // advance the position due to the read
        *pos = self.file.offset.len;
      }
      read_pos
    };
    if read_pos > self.file.offset.len {
      return Ok(Cow::Borrowed(&[]));
    }
    if read_pos == 0 {
      Ok(self.vfs.read_file_all(&self.file)?)
    } else {
      let size = (self.file.offset.len - read_pos) as usize;
      let mut buf = vec![0; size];
      self.vfs.read_file(&self.file, read_pos, &mut buf)?;
      Ok(Cow::Owned(buf))
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_io::fs::File for FileBackedVfsFile {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize> {
    self.read_to_buf(buf).map_err(Into::into)
  }
  async fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)> {
    // this is fast, no need to spawn a task
    let nread = self.read_to_buf(&mut buf)?;
    Ok((nread, buf))
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

  fn read_all_sync(self: Rc<Self>) -> FsResult<Cow<'static, [u8]>> {
    self.read_to_end()
  }
  async fn read_all_async(self: Rc<Self>) -> FsResult<Cow<'static, [u8]>> {
    // this is fast, no need to spawn a task
    self.read_to_end()
  }

  fn chmod_sync(self: Rc<Self>, _pathmode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }
  async fn chmod_async(self: Rc<Self>, _mode: u32) -> FsResult<()> {
    Err(FsError::NotSupported)
  }

  fn seek_sync(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos).map_err(|err| err.into())
  }
  async fn seek_async(self: Rc<Self>, pos: SeekFrom) -> FsResult<u64> {
    self.seek(pos).map_err(|err| err.into())
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
  fn backing_fd(self: Rc<Self>) -> Option<ResourceHandleFd> {
    None
  }
  fn try_clone_inner(self: Rc<Self>) -> FsResult<Rc<dyn deno_io::fs::File>> {
    Ok(self)
  }
}

#[derive(Debug, Clone)]
pub struct FileBackedVfsDirEntry {
  pub parent_path: PathBuf,
  pub metadata: FileBackedVfsMetadata,
}

#[derive(Debug, Clone)]
pub struct FileBackedVfsMetadata {
  pub name: String,
  pub file_type: sys_traits::FileType,
  pub len: u64,
  pub mtime: Option<u128>,
}

impl FileBackedVfsMetadata {
  pub fn from_vfs_entry_ref(vfs_entry: VfsEntryRef) -> Self {
    FileBackedVfsMetadata {
      file_type: match vfs_entry {
        VfsEntryRef::Dir(_) => sys_traits::FileType::Dir,
        VfsEntryRef::File(_) => sys_traits::FileType::File,
        VfsEntryRef::Symlink(_) => sys_traits::FileType::Symlink,
      },
      name: vfs_entry.name().to_string(),
      len: match vfs_entry {
        VfsEntryRef::Dir(_) => 0,
        VfsEntryRef::File(file) => file.offset.len,
        VfsEntryRef::Symlink(_) => 0,
      },
      mtime: match vfs_entry {
        VfsEntryRef::Dir(_) => None,
        VfsEntryRef::File(file) => file.mtime,
        VfsEntryRef::Symlink(_) => None,
      },
    }
  }
  pub fn as_fs_stat(&self) -> FsStat {
    // to use lower overhead, use mtime instead of all time params
    FsStat {
      is_directory: self.file_type == sys_traits::FileType::Dir,
      is_file: self.file_type == sys_traits::FileType::File,
      is_symlink: self.file_type == sys_traits::FileType::Symlink,
      atime: Some(self.get_mtime()),
      birthtime: Some(self.get_mtime()),
      mtime: Some(self.get_mtime()),
      ctime: Some(self.get_mtime()),
      blksize: 0,
      size: self.len,
      dev: 0,
      ino: 0,
      mode: 0,
      nlink: 0,
      uid: 0,
      gid: 0,
      rdev: 0,
      blocks: 0,
      is_block_device: false,
      is_char_device: false,
      is_fifo: false,
      is_socket: false,
    }
  }

  /// if `mtime` is `None`, return `0`.
  ///
  /// if `mtime` is greater than `u64::MAX`, return `u64::MAX`.
  fn get_mtime(&self) -> u64 {
    self.mtime.unwrap_or(0).try_into().unwrap_or(u64::MAX)
  }
}

#[derive(Debug)]
pub struct FileBackedVfs {
  vfs_data: Cow<'static, [u8]>,
  fs_root: VfsRoot,
  case_sensitivity: FileSystemCaseSensitivity,
}

impl FileBackedVfs {
  pub fn new(
    data: Cow<'static, [u8]>,
    fs_root: VfsRoot,
    case_sensitivity: FileSystemCaseSensitivity,
  ) -> Self {
    Self {
      vfs_data: data,
      fs_root,
      case_sensitivity,
    }
  }

  pub fn root(&self) -> &Path {
    &self.fs_root.root_path
  }

  pub fn is_path_within(&self, path: &Path) -> bool {
    path.starts_with(&self.fs_root.root_path)
  }

  pub fn open_file(
    self: &Arc<Self>,
    path: &Path,
  ) -> std::io::Result<FileBackedVfsFile> {
    let file = self.file_entry(path)?;
    Ok(FileBackedVfsFile {
      file: file.clone(),
      vfs: self.clone(),
      pos: Default::default(),
    })
  }

  pub fn read_dir(&self, path: &Path) -> std::io::Result<Vec<FsDirEntry>> {
    let dir = self.dir_entry(path)?;
    Ok(
      dir
        .entries
        .iter()
        .map(|entry| FsDirEntry {
          name: entry.name().to_string(),
          is_file: matches!(entry, VfsEntry::File(_)),
          is_directory: matches!(entry, VfsEntry::Dir(_)),
          is_symlink: matches!(entry, VfsEntry::Symlink(_)),
        })
        .collect(),
    )
  }

  pub fn read_dir_with_metadata<'a>(
    &'a self,
    path: &Path,
  ) -> std::io::Result<impl Iterator<Item = FileBackedVfsDirEntry> + 'a> {
    let dir = self.dir_entry(path)?;
    let path = path.to_path_buf();
    Ok(dir.entries.iter().map(move |entry| FileBackedVfsDirEntry {
      parent_path: path.to_path_buf(),
      metadata: FileBackedVfsMetadata::from_vfs_entry_ref(entry.as_ref()),
    }))
  }

  pub fn read_link(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (_, entry) = self
      .fs_root
      .find_entry_no_follow(path, self.case_sensitivity)?;
    match entry {
      VfsEntryRef::Symlink(symlink) => {
        Ok(symlink.resolve_dest_from_root(&self.fs_root.root_path))
      }
      VfsEntryRef::Dir(_) | VfsEntryRef::File(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "not a symlink",
      )),
    }
  }

  pub fn lstat(&self, path: &Path) -> std::io::Result<FileBackedVfsMetadata> {
    let (_, entry) = self
      .fs_root
      .find_entry_no_follow(path, self.case_sensitivity)?;
    Ok(FileBackedVfsMetadata::from_vfs_entry_ref(entry))
  }

  pub fn stat(&self, path: &Path) -> std::io::Result<FileBackedVfsMetadata> {
    let (_, entry) = self.fs_root.find_entry(path, self.case_sensitivity)?;
    Ok(FileBackedVfsMetadata::from_vfs_entry_ref(entry))
  }

  pub fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
    let (path, _) = self.fs_root.find_entry(path, self.case_sensitivity)?;
    Ok(path)
  }

  pub fn read_file_all(
    &self,
    file: &VirtualFile,
  ) -> std::io::Result<Cow<'static, [u8]>> {
    self.read_file_offset_with_len(file.offset)
  }

  pub fn read_file_offset_with_len(
    &self,
    offset_with_len: OffsetWithLength,
  ) -> std::io::Result<Cow<'static, [u8]>> {
    let read_range =
      self.get_read_range(offset_with_len, 0, offset_with_len.len)?;
    match &self.vfs_data {
      Cow::Borrowed(data) => Ok(Cow::Borrowed(&data[read_range])),
      Cow::Owned(data) => Ok(Cow::Owned(data[read_range].to_vec())),
    }
  }

  pub fn read_file(
    &self,
    file: &VirtualFile,
    pos: u64,
    buf: &mut [u8],
  ) -> std::io::Result<usize> {
    let read_range = self.get_read_range(file.offset, pos, buf.len() as u64)?;
    let read_len = read_range.len();
    buf[..read_len].copy_from_slice(&self.vfs_data[read_range]);
    Ok(read_len)
  }

  fn get_read_range(
    &self,
    file_offset_and_len: OffsetWithLength,
    pos: u64,
    len: u64,
  ) -> std::io::Result<Range<usize>> {
    if pos > file_offset_and_len.len {
      return Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "unexpected EOF",
      ));
    }
    let file_offset =
      self.fs_root.start_file_offset + file_offset_and_len.offset;
    let start = file_offset + pos;
    let end = file_offset + std::cmp::min(pos + len, file_offset_and_len.len);
    Ok(start as usize..end as usize)
  }

  pub fn dir_entry(&self, path: &Path) -> std::io::Result<&VirtualDirectory> {
    let (_, entry) = self.fs_root.find_entry(path, self.case_sensitivity)?;
    match entry {
      VfsEntryRef::Dir(dir) => Ok(dir),
      VfsEntryRef::Symlink(_) => unreachable!(),
      VfsEntryRef::File(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "path is a file",
      )),
    }
  }

  pub fn file_entry(&self, path: &Path) -> std::io::Result<&VirtualFile> {
    let (_, entry) = self.fs_root.find_entry(path, self.case_sensitivity)?;
    match entry {
      VfsEntryRef::Dir(_) => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "path is a directory",
      )),
      VfsEntryRef::Symlink(_) => unreachable!(),
      VfsEntryRef::File(file) => Ok(file),
    }
  }
}

impl DenoRtNativeAddonLoader for FileBackedVfs {
  fn load_if_in_vfs(&self, path: &Path) -> Option<Cow<'static, [u8]>> {
    if !self.is_path_within(path) {
      return None;
    }
    let file = self.file_entry(path).ok()?;
    self.read_file_offset_with_len(file.offset).ok()
  }
}

#[cfg(test)]
mod test {
  use std::io::Write;

  use deno_lib::standalone::virtual_fs::VfsBuilder;
  use test_util::assert_contains;
  use test_util::TempDir;

  use super::*;

  #[track_caller]
  fn read_file(vfs: &FileBackedVfs, path: &Path) -> String {
    let file = vfs.file_entry(path).unwrap();
    String::from_utf8(vfs.read_file_all(file).unwrap().into_owned()).unwrap()
  }

  #[test]
  fn builds_and_uses_virtual_fs() {
    let temp_dir = TempDir::new();
    // we canonicalize the temp directory because the vfs builder
    // will canonicalize the root path
    let src_path = temp_dir.path().canonicalize().join("src");
    src_path.create_dir_all();
    src_path.join("sub_dir").create_dir_all();
    src_path.join("e.txt").write("e");
    src_path.symlink_file("e.txt", "sub_dir/e.txt");
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new();
    builder
      .add_file_with_data_raw(&src_path.join("a.txt"), "data".into(), None)
      .unwrap();
    builder
      .add_file_with_data_raw(
        &src_path.join("b.txt"),
        "data".into(),
        Some(
          SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(2))
            .unwrap(),
        ),
      )
      .unwrap();
    assert_eq!(builder.files_len(), 1); // because duplicate data
    builder
      .add_file_with_data_raw(&src_path.join("c.txt"), "c".into(), None)
      .unwrap();
    builder
      .add_file_with_data_raw(
        &src_path.join("sub_dir").join("d.txt"),
        "d".into(),
        None,
      )
      .unwrap();
    builder.add_file_at_path(&src_path.join("e.txt")).unwrap();
    builder
      .add_symlink(&src_path.join("sub_dir").join("e.txt"))
      .unwrap();

    // get the virtual fs
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(read_file(&virtual_fs, &dest_path.join("a.txt")), "data");
    assert_eq!(read_file(&virtual_fs, &dest_path.join("b.txt")), "data");

    // attempt reading a symlink
    assert_eq!(
      read_file(&virtual_fs, &dest_path.join("sub_dir").join("e.txt")),
      "e",
    );

    // canonicalize symlink
    assert_eq!(
      virtual_fs
        .canonicalize(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap(),
      dest_path.join("e.txt"),
    );

    // metadata
    assert_eq!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Symlink,
    );
    assert_eq!(
      virtual_fs.lstat(&dest_path.join("b.txt")).unwrap().mtime,
      Some(2_000),
    );
    assert_eq!(
      virtual_fs
        .stat(&dest_path.join("sub_dir").join("e.txt"))
        .unwrap()
        .file_type,
      sys_traits::FileType::File,
    );
    assert_eq!(
      virtual_fs
        .stat(&dest_path.join("sub_dir"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Dir,
    );
    assert_eq!(
      virtual_fs.stat(&dest_path.join("e.txt")).unwrap().file_type,
      sys_traits::FileType::File
    );
  }

  #[test]
  fn test_include_dir_recursive() {
    let temp_dir = TempDir::new();
    let temp_dir_path = temp_dir.path().canonicalize();
    temp_dir.create_dir_all("src/nested/sub_dir");
    temp_dir.write("src/a.txt", "data");
    temp_dir.write("src/b.txt", "data");
    temp_dir.path().symlink_dir(
      temp_dir_path.join("src/nested/sub_dir"),
      temp_dir_path.join("src/sub_dir_link"),
    );
    temp_dir.write("src/nested/sub_dir/c.txt", "c");

    // build and create the virtual fs
    let src_path = temp_dir_path.join("src").to_path_buf();
    let mut builder = VfsBuilder::new();
    builder.add_dir_recursive(&src_path).unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);

    assert_eq!(read_file(&virtual_fs, &dest_path.join("a.txt")), "data",);
    assert_eq!(read_file(&virtual_fs, &dest_path.join("b.txt")), "data",);

    assert_eq!(
      read_file(
        &virtual_fs,
        &dest_path.join("nested").join("sub_dir").join("c.txt")
      ),
      "c",
    );
    assert_eq!(
      read_file(&virtual_fs, &dest_path.join("sub_dir_link").join("c.txt")),
      "c",
    );
    assert_eq!(
      virtual_fs
        .lstat(&dest_path.join("sub_dir_link"))
        .unwrap()
        .file_type,
      sys_traits::FileType::Symlink,
    );

    assert_eq!(
      virtual_fs
        .canonicalize(&dest_path.join("sub_dir_link").join("c.txt"))
        .unwrap(),
      dest_path.join("nested").join("sub_dir").join("c.txt"),
    );
  }

  fn into_virtual_fs(
    builder: VfsBuilder,
    temp_dir: &TempDir,
  ) -> (PathBuf, FileBackedVfs) {
    let virtual_fs_file = temp_dir.path().join("virtual_fs");
    let vfs = builder.build();
    {
      let mut file = std::fs::File::create(&virtual_fs_file).unwrap();
      for file_data in &vfs.files {
        file.write_all(file_data).unwrap();
      }
    }
    let dest_path = temp_dir.path().join("dest");
    let data = std::fs::read(&virtual_fs_file).unwrap();
    (
      dest_path.to_path_buf(),
      FileBackedVfs::new(
        Cow::Owned(data),
        VfsRoot {
          dir: VirtualDirectory {
            name: "".to_string(),
            entries: vfs.entries,
          },
          root_path: dest_path.to_path_buf(),
          start_file_offset: 0,
        },
        FileSystemCaseSensitivity::Sensitive,
      ),
    )
  }

  #[test]
  fn circular_symlink() {
    let temp_dir = TempDir::new();
    let src_path = temp_dir.path().canonicalize().join("src");
    src_path.create_dir_all();
    src_path.symlink_file("a.txt", "b.txt");
    src_path.symlink_file("b.txt", "c.txt");
    src_path.symlink_file("c.txt", "a.txt");
    let src_path = src_path.to_path_buf();
    let mut builder = VfsBuilder::new();
    let err = builder
      .add_symlink(src_path.join("a.txt").as_path())
      .unwrap_err();
    assert_contains!(err.to_string(), "Circular symlink detected",);
  }

  #[tokio::test]
  async fn test_open_file() {
    let temp_dir = TempDir::new();
    let temp_path = temp_dir.path().canonicalize();
    let mut builder = VfsBuilder::new();
    builder
      .add_file_with_data_raw(
        temp_path.join("a.txt").as_path(),
        "0123456789".to_string().into_bytes(),
        None,
      )
      .unwrap();
    let (dest_path, virtual_fs) = into_virtual_fs(builder, &temp_dir);
    let virtual_fs = Arc::new(virtual_fs);
    let file = virtual_fs.open_file(&dest_path.join("a.txt")).unwrap();
    file.seek(SeekFrom::Current(2)).unwrap();
    let mut buf = vec![0; 2];
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"45");
    file.seek(SeekFrom::Current(-4)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.seek(SeekFrom::Start(2)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    file.seek(SeekFrom::End(2)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"89");
    file.seek(SeekFrom::Current(-8)).unwrap();
    file.read_to_buf(&mut buf).unwrap();
    assert_eq!(buf, b"23");
    assert_eq!(
      file
        .seek(SeekFrom::Current(-5))
        .unwrap_err()
        .to_string(),
      "An attempt was made to move the file pointer before the beginning of the file."
    );
    // go beyond the file length, then back
    file.seek(SeekFrom::Current(40)).unwrap();
    file.seek(SeekFrom::Current(-38)).unwrap();
    let file = Rc::new(file);
    let read_buf = file.clone().read(2).await.unwrap();
    assert_eq!(read_buf.to_vec(), b"67");
    file.clone().seek_sync(SeekFrom::Current(-2)).unwrap();

    // read to the end of the file
    let all_buf = file.clone().read_all_sync().unwrap();
    assert_eq!(all_buf.to_vec(), b"6789");
    file.clone().seek_sync(SeekFrom::Current(-9)).unwrap();

    // try try_clone_inner and read_all_async
    let all_buf = file
      .try_clone_inner()
      .unwrap()
      .read_all_async()
      .await
      .unwrap();
    assert_eq!(all_buf.to_vec(), b"123456789");
  }
}
