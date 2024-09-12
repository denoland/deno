// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::io;
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::error::custom_error;
use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::OpState;
use deno_core::ResourceHandleFd;
use deno_core::ResourceId;
use tokio::task::JoinError;

#[derive(Debug)]
pub enum FsError {
  Io(io::Error),
  FileBusy,
  NotSupported,
  NotCapable(&'static str),
}

impl FsError {
  pub fn kind(&self) -> io::ErrorKind {
    match self {
      Self::Io(err) => err.kind(),
      Self::FileBusy => io::ErrorKind::Other,
      Self::NotSupported => io::ErrorKind::Other,
      Self::NotCapable(_) => io::ErrorKind::Other,
    }
  }

  pub fn into_io_error(self) -> io::Error {
    match self {
      FsError::Io(err) => err,
      FsError::FileBusy => io::Error::new(self.kind(), "file busy"),
      FsError::NotSupported => io::Error::new(self.kind(), "not supported"),
      FsError::NotCapable(err) => {
        io::Error::new(self.kind(), format!("requires {err} access"))
      }
    }
  }
}

impl From<io::Error> for FsError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl From<io::ErrorKind> for FsError {
  fn from(err: io::ErrorKind) -> Self {
    Self::Io(err.into())
  }
}

impl From<FsError> for AnyError {
  fn from(err: FsError) -> Self {
    match err {
      FsError::Io(err) => AnyError::from(err),
      FsError::FileBusy => resource_unavailable(),
      FsError::NotSupported => not_supported(),
      FsError::NotCapable(err) => {
        custom_error("NotCapable", format!("permission denied: {err}"))
      }
    }
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

pub type FsResult<T> = Result<T, FsError>;

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
  pub is_block_device: bool,
  pub is_char_device: bool,
  pub is_fifo: bool,
  pub is_socket: bool,
}

impl FsStat {
  pub fn from_std(metadata: std::fs::Metadata) -> Self {
    macro_rules! unix_or_zero {
      ($member:ident) => {{
        #[cfg(unix)]
        {
          use std::os::unix::fs::MetadataExt;
          metadata.$member()
        }
        #[cfg(not(unix))]
        {
          0
        }
      }};
    }

    macro_rules! unix_or_false {
      ($member:ident) => {{
        #[cfg(unix)]
        {
          use std::os::unix::fs::FileTypeExt;
          metadata.file_type().$member()
        }
        #[cfg(not(unix))]
        {
          false
        }
      }};
    }

    #[inline(always)]
    fn to_msec(maybe_time: Result<SystemTime, io::Error>) -> Option<u64> {
      match maybe_time {
        Ok(time) => Some(
          time
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_millis() as u64)
            .unwrap_or_else(|err| err.duration().as_millis() as u64),
        ),
        Err(_) => None,
      }
    }

    Self {
      is_file: metadata.is_file(),
      is_directory: metadata.is_dir(),
      is_symlink: metadata.file_type().is_symlink(),
      size: metadata.len(),

      mtime: to_msec(metadata.modified()),
      atime: to_msec(metadata.accessed()),
      birthtime: to_msec(metadata.created()),

      dev: unix_or_zero!(dev),
      ino: unix_or_zero!(ino),
      mode: unix_or_zero!(mode),
      nlink: unix_or_zero!(nlink),
      uid: unix_or_zero!(uid),
      gid: unix_or_zero!(gid),
      rdev: unix_or_zero!(rdev),
      blksize: unix_or_zero!(blksize),
      blocks: unix_or_zero!(blocks),
      is_block_device: unix_or_false!(is_block_device),
      is_char_device: unix_or_false!(is_char_device),
      is_fifo: unix_or_false!(is_fifo),
      is_socket: unix_or_false!(is_socket),
    }
  }
}

#[async_trait::async_trait(?Send)]
pub trait File {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize>;
  async fn read(self: Rc<Self>, limit: usize) -> FsResult<BufView> {
    let buf = BufMutView::new(limit);
    let (nread, mut buf) = self.read_byob(buf).await?;
    buf.truncate(nread);
    Ok(buf.into_view())
  }
  async fn read_byob(
    self: Rc<Self>,
    buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)>;

  fn write_sync(self: Rc<Self>, buf: &[u8]) -> FsResult<usize>;
  async fn write(
    self: Rc<Self>,
    buf: BufView,
  ) -> FsResult<deno_core::WriteOutcome>;

  fn write_all_sync(self: Rc<Self>, buf: &[u8]) -> FsResult<()>;
  async fn write_all(self: Rc<Self>, buf: BufView) -> FsResult<()>;

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

  // lower level functionality
  fn as_stdio(self: Rc<Self>) -> FsResult<std::process::Stdio>;
  fn backing_fd(self: Rc<Self>) -> Option<ResourceHandleFd>;
  fn try_clone_inner(self: Rc<Self>) -> FsResult<Rc<dyn File>>;
}

pub struct FileResource {
  name: String,
  file: Rc<dyn File>,
}

impl FileResource {
  pub fn new(file: Rc<dyn File>, name: String) -> Self {
    Self { name, file }
  }

  fn with_resource<F, R>(
    state: &OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<FileResource>) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<FileResource>(rid)?;
    f(resource)
  }

  pub fn get_file(
    state: &OpState,
    rid: ResourceId,
  ) -> Result<Rc<dyn File>, AnyError> {
    let resource = state.resource_table.get::<FileResource>(rid)?;
    Ok(resource.file())
  }

  pub fn with_file<F, R>(
    state: &OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<dyn File>) -> Result<R, AnyError>,
  {
    Self::with_resource(state, rid, |r| f(r.file.clone()))
  }

  pub fn file(&self) -> Rc<dyn File> {
    self.file.clone()
  }
}

impl deno_core::Resource for FileResource {
  fn name(&self) -> Cow<str> {
    Cow::Borrowed(&self.name)
  }

  fn read(
    self: Rc<Self>,
    limit: usize,
  ) -> deno_core::AsyncResult<deno_core::BufView> {
    Box::pin(async move {
      self
        .file
        .clone()
        .read(limit)
        .await
        .map_err(|err| err.into())
    })
  }

  fn read_byob(
    self: Rc<Self>,
    buf: deno_core::BufMutView,
  ) -> deno_core::AsyncResult<(usize, deno_core::BufMutView)> {
    Box::pin(async move {
      self
        .file
        .clone()
        .read_byob(buf)
        .await
        .map_err(|err| err.into())
    })
  }

  fn write(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> deno_core::AsyncResult<deno_core::WriteOutcome> {
    Box::pin(async move {
      self.file.clone().write(buf).await.map_err(|err| err.into())
    })
  }

  fn write_all(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> deno_core::AsyncResult<()> {
    Box::pin(async move {
      self
        .file
        .clone()
        .write_all(buf)
        .await
        .map_err(|err| err.into())
    })
  }

  fn read_byob_sync(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, deno_core::anyhow::Error> {
    self.file.clone().read_sync(data).map_err(|err| err.into())
  }

  fn write_sync(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, deno_core::anyhow::Error> {
    self.file.clone().write_sync(data).map_err(|err| err.into())
  }

  fn backing_fd(self: Rc<Self>) -> Option<ResourceHandleFd> {
    self.file.clone().backing_fd()
  }
}
