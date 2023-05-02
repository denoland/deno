// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::File as StdFile;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::FutureExt;
use deno_core::AsyncResult;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::TaskQueue;

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

pub trait FileSync {
  fn write_all_sync(self: Rc<Self>, buf: &[u8]) -> FsResult<()>;
  fn write_sync(self: Rc<Self>, buf: &[u8]) -> FsResult<usize>;
  fn read_all_sync(self: Rc<Self>) -> FsResult<Vec<u8>>;
  fn read_sync(self: Rc<Self>, buf: &mut [u8]) -> FsResult<usize>;
  fn chmod_sync(self: Rc<Self>, pathmode: u32) -> FsResult<()>;
  fn seek_sync(self: Rc<Self>, pos: io::SeekFrom) -> FsResult<u64>;
  fn datasync_sync(self: Rc<Self>) -> FsResult<()>;
  fn sync_sync(self: Rc<Self>) -> FsResult<()>;
  fn stat_sync(self: Rc<Self>) -> FsResult<FsStat>;
  fn lock_sync(self: Rc<Self>, exclusive: bool) -> FsResult<()>;
  fn unlock_sync(self: Rc<Self>) -> FsResult<()>;
  fn truncate_sync(self: Rc<Self>, len: u64) -> FsResult<()>;
  fn utime_sync(
    self: Rc<Self>,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  // lower level functionality
  fn as_stdio(self: Rc<Self>) -> FsResult<std::process::Stdio>;
  #[cfg(unix)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::unix::prelude::RawFd>;
  #[cfg(windows)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::windows::io::RawHandle>;
}

#[async_trait::async_trait(?Send)]
pub trait FileAsync {
  async fn write_all_async(self: Rc<Self>, buf: Vec<u8>) -> FsResult<()>;
  async fn read_all_async(self: Rc<Self>) -> FsResult<Vec<u8>>;
  async fn chmod_async(self: Rc<Self>, mode: u32) -> FsResult<()>;
  async fn seek_async(self: Rc<Self>, pos: io::SeekFrom) -> FsResult<u64>;
  async fn datasync_async(self: Rc<Self>) -> FsResult<()>;
  async fn sync_async(self: Rc<Self>) -> FsResult<()>;
  async fn stat_async(self: Rc<Self>) -> FsResult<FsStat>;
  async fn lock_async(self: Rc<Self>, exclusive: bool) -> FsResult<()>;
  async fn unlock_async(self: Rc<Self>) -> FsResult<()>;
  async fn truncate_async(self: Rc<Self>, len: u64) -> FsResult<()>;
  async fn utime_async(
    self: Rc<Self>,
    atime_secs: i64,
    atime_nanos: u32,
    mtime_secs: i64,
    mtime_nanos: u32,
  ) -> FsResult<()>;

  async fn read_byob(
    self: Rc<Self>,
    buf: BufMutView,
  ) -> FsResult<(usize, BufMutView)>;
  async fn write_byob(
    self: Rc<Self>,
    buf: BufView,
  ) -> FsResult<deno_core::WriteOutcome>;
  async fn write_all_byob(self: Rc<Self>, buf: BufView) -> FsResult<()>;
}

pub trait File: FileSync + FileAsync {
  fn try_clone_inner(self: Rc<Self>) -> Option<Rc<dyn File>>;
  fn as_sync(self: Rc<Self>) -> Rc<dyn FileSync>;
  fn as_async(self: Rc<Self>) -> Rc<dyn FileAsync>;
}

pub struct FileResource {
  name: String,
  // We can't use an AsyncRefCell here because we need to allow
  // access to the resource synchronously at any time and
  // asynchronously one at a time in order
  cell: RefCell<Option<Rc<dyn File>>>,
  // Used to keep async actions in order and only allow one
  // to occur at a time
  cell_async_task_queue: TaskQueue,
}

impl FileResource {
  pub fn new(file: Rc<dyn File>, name: String) -> Self {
    Self {
      name,
      cell: RefCell::new(Some(file)),
      cell_async_task_queue: Default::default(),
    }
  }

  pub fn with_resource<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<FileResource>) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<FileResource>(rid)?;
    f(resource)
  }

  pub fn with_sync_file<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<dyn FileSync>) -> Result<R, AnyError>,
  {
    Self::with_resource(state, rid, |r| {
      r.with_sync(|file| f(file))
        .unwrap_or_else(|| Err(FsError::FileBusy.into()))
    })
  }

  fn with_sync<F, R>(&self, action: F) -> Option<R>
  where
    F: FnOnce(Rc<dyn FileSync>) -> R,
  {
    match self.cell.try_borrow() {
      Ok(cell) if cell.is_some() => {
        Some(action(cell.as_ref().unwrap().clone().as_sync()))
      }
      _ => None,
    }
  }

  async fn with_async<F, R: 'static>(&self, action: F) -> R
  where
    F: FnOnce(Rc<dyn File>) -> LocalBoxFuture<'static, (Rc<dyn File>, R)>,
  {
    // we want to restrict this to one async action at a time
    let _permit = self.cell_async_task_queue.acquire().await;
    // we take the value out of the cell, use it on a blocking task,
    // then put it back into the cell when we're done
    let mut did_take = false;
    let cell_value = {
      let mut cell = self.cell.borrow_mut();
      match cell.as_mut().unwrap().clone().try_clone_inner() {
        Some(value) => value,
        None => {
          did_take = true;
          cell.take().unwrap()
        }
      }
    };
    let (cell_value, result) = action(cell_value).await;

    if did_take {
      // put it back
      self.cell.borrow_mut().replace(cell_value);
    }

    result
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
      let vec = vec![0; limit];
      let buf = BufMutView::from(vec);
      let (nread, buf) = self
        .with_async(|file| {
          async {
            let result = file.clone().read_byob(buf).await;
            (file, result)
          }
          .boxed_local()
        })
        .await?;
      let mut vec = buf.unwrap_vec();
      if vec.len() != nread {
        vec.truncate(nread);
      }
      Ok(BufView::from(vec))
    })
  }

  fn read_byob(
    self: Rc<Self>,
    buf: deno_core::BufMutView,
  ) -> deno_core::AsyncResult<(usize, deno_core::BufMutView)> {
    Box::pin(async move {
      self
        .with_async(|file| {
          async {
            let result = file.clone().read_byob(buf).await;
            (file, result)
          }
          .boxed_local()
        })
        .await
        .map_err(|err| err.into())
    })
  }

  fn write(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> deno_core::AsyncResult<deno_core::WriteOutcome> {
    Box::pin(async move {
      self
        .with_async(|file| {
          async {
            let result = file.clone().write_byob(buf).await;
            (file, result)
          }
          .boxed_local()
        })
        .await
        .map_err(|err| err.into())
    })
  }

  fn write_all(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> deno_core::AsyncResult<()> {
    Box::pin(async move {
      self
        .with_async(|file| {
          async {
            let result = file.clone().write_all_byob(buf).await;
            (file, result)
          }
          .boxed_local()
        })
        .await
        .map_err(|err| err.into())
    })
  }

  fn read_byob_sync(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, deno_core::anyhow::Error> {
    self
      .with_sync(|file| file.read_sync(data))
      .unwrap_or_else(|| Err(FsError::FileBusy))
      .map_err(|err| err.into())
  }

  fn write_sync(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, deno_core::anyhow::Error> {
    self
      .with_sync(|file| file.write_sync(data))
      .unwrap_or_else(|| Err(FsError::FileBusy))
      .map_err(|err| err.into())
  }

  #[cfg(unix)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::unix::prelude::RawFd> {
    self.with_sync(|file| file.backing_fd()).flatten()
  }

  #[cfg(windows)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::windows::io::RawHandle> {
    self.with_sync(|file| file.backing_fd()).flatten()
  }
}
