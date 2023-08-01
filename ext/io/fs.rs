// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::io;
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::OpState;
use deno_core::ResourceBuilder;
use deno_core::ResourceBuilderImpl;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;
use deno_core::ResourceId;
use deno_core::ResourceStream;
use deno_core::ResourceStreamRead;
use deno_core::ResourceStreamWrite;
use tokio::task::JoinError;

use crate::StdFileResourceInner;

#[derive(Debug)]
pub enum FsError {
  Io(io::Error),
  FileBusy,
  NotSupported,
}

impl FsError {
  pub fn kind(&self) -> io::ErrorKind {
    match self {
      Self::Io(err) => err.kind(),
      Self::FileBusy => io::ErrorKind::Other,
      Self::NotSupported => io::ErrorKind::Other,
    }
  }

  pub fn into_io_error(self) -> io::Error {
    match self {
      FsError::Io(err) => err,
      FsError::FileBusy => io::Error::new(self.kind(), "file busy"),
      FsError::NotSupported => io::Error::new(self.kind(), "not supported"),
    }
  }
}

impl From<io::Error> for FsError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
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

// TODO(mmastrac): Restore dynamic name
pub static FILE_RESOURCE: ResourceBuilder<Rc<dyn File>> =
  ResourceBuilderImpl::new("file")
    .with_read_write_resource_stream::<dyn File>()
    .build();
pub static FILE_STDIN_RESOURCE: ResourceBuilder<Rc<dyn File>> =
  ResourceBuilderImpl::new("stdin")
    .with_read_write_resource_stream::<dyn File>()
    .build();
pub static FILE_STDERR_RESOURCE: ResourceBuilder<Rc<dyn File>> =
  ResourceBuilderImpl::new("stderr")
    .with_read_write_resource_stream::<dyn File>()
    .build();
pub static FILE_STDOUT_RESOURCE: ResourceBuilder<Rc<dyn File>> =
  ResourceBuilderImpl::new("stdout")
    .with_read_write_resource_stream::<dyn File>()
    .build();

#[async_trait::async_trait(?Send)]
pub trait File: ResourceStreamRead + ResourceStreamWrite {
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize>;
  async fn read(self: Rc<Self>, limit: usize) -> FsResult<BufView> {
    let buf = BufMutView::new(limit);
    let (nread, mut buf) = File::read_byob(self, buf).await?;
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
  fn backing_handle(self: Rc<Self>) -> Option<ResourceHandle>;
  fn try_clone_inner(self: Rc<Self>) -> FsResult<Rc<dyn File>>;
}

/// Implement [`ResourceStream`], [`ResourceStreamRead`] and [`ResourceStreamWrite`] for a given [`File`] type
/// by delegating to the appropriate file methods.
#[macro_export]
macro_rules! resource_stream_for_file {
  ($file:ident) => {
    impl ::deno_core::ResourceStream for $file {
      fn backing_handle(
        self: ::std::rc::Rc<Self>,
      ) -> Option<::deno_core::ResourceHandle> {
        <Self as $crate::fs::File>::backing_handle(self)
      }
    }

    impl ::deno_core::ResourceStreamWrite for $file {
      fn write(
        self: ::std::rc::Rc<Self>,
        buf: BufView,
      ) -> ::deno_core::AsyncResult<::deno_core::WriteOutcome> {
        use deno_core::futures::TryFutureExt;
        Box::pin(
          <Self as $crate::fs::File>::write(self, buf).map_err(|e| e.into()),
        )
      }

      fn write_all(
        self: Rc<Self>,
        view: ::deno_core::BufView,
      ) -> ::deno_core::AsyncResult<()> {
        use deno_core::futures::TryFutureExt;
        Box::pin(
          <Self as $crate::fs::File>::write_all(self, view)
            .map_err(|e| e.into()),
        )
      }

      fn write_sync(
        self: ::std::rc::Rc<Self>,
        data: &[u8],
      ) -> ::std::result::Result<usize, deno_core::anyhow::Error> {
        <Self as $crate::fs::File>::write_sync(self, data).map_err(|e| e.into())
      }
    }

    impl ::deno_core::ResourceStreamRead for $file {
      fn read(
        self: ::std::rc::Rc<Self>,
        limit: usize,
      ) -> ::deno_core::AsyncResult<::deno_core::BufView> {
        use deno_core::futures::TryFutureExt;
        Box::pin(
          <Self as $crate::fs::File>::read(self, limit).map_err(|e| e.into()),
        )
      }

      fn read_byob(
        self: ::std::rc::Rc<Self>,
        buf: ::deno_core::BufMutView,
      ) -> ::deno_core::AsyncResult<(usize, ::deno_core::BufMutView)> {
        use deno_core::futures::TryFutureExt;
        Box::pin(
          <Self as $crate::fs::File>::read_byob(self, buf)
            .map_err(|e| e.into()),
        )
      }

      fn read_byob_sync(
        self: ::std::rc::Rc<Self>,
        data: &mut [u8],
      ) -> ::std::result::Result<usize, deno_core::anyhow::Error> {
        <Self as $crate::fs::File>::read_sync(self, data).map_err(|e| e.into())
      }
    }
  };
}

pub struct FileResource {
  name: String,
  file: Rc<dyn File>,
}

impl FileResource {
  pub fn new(file: Rc<dyn File>, name: String) -> Self {
    Self { name, file }
  }

  pub fn get_file(
    state: &OpState,
    rid: ResourceId,
  ) -> Result<Rc<dyn File>, AnyError> {
    let res = state.resource_table.get_any(rid)?;
    Ok(
      FILE_RESOURCE
        .reader::<dyn File>(&res)
        .ok_or_else(bad_resource_id)?
        .clone(),
    )
  }

  pub fn with_file<F, R>(
    state: &OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<dyn File>) -> Result<R, AnyError>,
  {
    f(Self::get_file(state, rid)?)
  }

  pub fn file(&self) -> Rc<dyn File> {
    self.file.clone()
  }
}
