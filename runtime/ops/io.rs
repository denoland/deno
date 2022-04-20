// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::not_supported;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::File as StdFile;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::process;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use {
  std::os::windows::io::FromRawHandle,
  winapi::um::{processenv::GetStdHandle, winbase},
};

#[cfg(unix)]
static STDIN_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(0) });
#[cfg(unix)]
static STDOUT_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(1) });
#[cfg(unix)]
static STDERR_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(2) });

/// Due to portability issues on Windows handle to stdout is created from raw
/// file descriptor.  The caveat of that approach is fact that when this
/// handle is dropped underlying file descriptor is closed - that is highly
/// not desirable in case of stdout.  That's why we store this global handle
/// that is then cloned when obtaining stdio for process. In turn when
/// resource table is dropped storing reference to that handle, the handle
/// itself won't be closed (so Deno.core.print) will still work.
// TODO(ry) It should be possible to close stdout.
#[cfg(windows)]
static STDIN_HANDLE: Lazy<StdFile> = Lazy::new(|| unsafe {
  StdFile::from_raw_handle(GetStdHandle(winbase::STD_INPUT_HANDLE))
});
#[cfg(windows)]
static STDOUT_HANDLE: Lazy<StdFile> = Lazy::new(|| unsafe {
  StdFile::from_raw_handle(GetStdHandle(winbase::STD_OUTPUT_HANDLE))
});
#[cfg(windows)]
static STDERR_HANDLE: Lazy<StdFile> = Lazy::new(|| unsafe {
  StdFile::from_raw_handle(GetStdHandle(winbase::STD_ERROR_HANDLE))
});

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_read_sync::decl(), op_write_sync::decl()])
    .build()
}

pub fn init_stdio() -> Extension {
  Extension::builder()
    .state(|state| {
      let t = &mut state.resource_table;
      t.add(StdFileResource::stdio(&STDIN_HANDLE, "stdin"));
      t.add(StdFileResource::stdio(&STDOUT_HANDLE, "stdout"));
      t.add(StdFileResource::stdio(&STDERR_HANDLE, "stderr"));
      Ok(())
    })
    .build()
}

#[cfg(unix)]
use nix::sys::termios;

#[derive(Default)]
pub struct TtyMetadata {
  #[cfg(unix)]
  pub mode: Option<termios::Termios>,
}

#[derive(Default)]
pub struct FileMetadata {
  pub tty: TtyMetadata,
}

#[derive(Debug)]
pub struct WriteOnlyResource<S> {
  stream: AsyncRefCell<S>,
}

impl<S: 'static> From<S> for WriteOnlyResource<S> {
  fn from(stream: S) -> Self {
    Self {
      stream: stream.into(),
    }
  }
}

impl<S> WriteOnlyResource<S>
where
  S: AsyncWrite + Unpin + 'static,
{
  pub fn borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<S> {
    RcRef::map(self, |r| &r.stream).borrow_mut()
  }

  async fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> Result<usize, AnyError> {
    let mut stream = self.borrow_mut().await;
    let nwritten = stream.write(&buf).await?;
    Ok(nwritten)
  }

  async fn shutdown(self: Rc<Self>) -> Result<(), AnyError> {
    let mut stream = self.borrow_mut().await;
    stream.shutdown().await?;
    Ok(())
  }

  pub fn into_inner(self) -> S {
    self.stream.into_inner()
  }
}

#[derive(Debug)]
pub struct ReadOnlyResource<S> {
  stream: AsyncRefCell<S>,
  cancel_handle: CancelHandle,
}

impl<S: 'static> From<S> for ReadOnlyResource<S> {
  fn from(stream: S) -> Self {
    Self {
      stream: stream.into(),
      cancel_handle: Default::default(),
    }
  }
}

impl<S> ReadOnlyResource<S>
where
  S: AsyncRead + Unpin + 'static,
{
  pub fn borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<S> {
    RcRef::map(self, |r| &r.stream).borrow_mut()
  }

  pub fn cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.cancel_handle)
  }

  pub fn cancel_read_ops(&self) {
    self.cancel_handle.cancel()
  }

  async fn read(
    self: Rc<Self>,
    mut buf: ZeroCopyBuf,
  ) -> Result<(usize, ZeroCopyBuf), AnyError> {
    let mut rd = self.borrow_mut().await;
    let nread = rd
      .read(&mut buf)
      .try_or_cancel(self.cancel_handle())
      .await?;
    Ok((nread, buf))
  }

  pub fn into_inner(self) -> S {
    self.stream.into_inner()
  }
}

pub type ChildStdinResource = WriteOnlyResource<process::ChildStdin>;

impl Resource for ChildStdinResource {
  fn name(&self) -> Cow<str> {
    "childStdin".into()
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(self.write(buf))
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown())
  }
}

pub type ChildStdoutResource = ReadOnlyResource<process::ChildStdout>;

impl Resource for ChildStdoutResource {
  fn name(&self) -> Cow<str> {
    "childStdout".into()
  }

  fn read_return(
    self: Rc<Self>,
    buf: ZeroCopyBuf,
  ) -> AsyncResult<(usize, ZeroCopyBuf)> {
    Box::pin(self.read(buf))
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

pub type ChildStderrResource = ReadOnlyResource<process::ChildStderr>;

impl Resource for ChildStderrResource {
  fn name(&self) -> Cow<str> {
    "childStderr".into()
  }

  fn read_return(
    self: Rc<Self>,
    buf: ZeroCopyBuf,
  ) -> AsyncResult<(usize, ZeroCopyBuf)> {
    Box::pin(self.read(buf))
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

type MaybeSharedStdFile = Option<Arc<Mutex<StdFile>>>;

#[derive(Default)]
pub struct StdFileResource {
  pub fs_file: Option<(MaybeSharedStdFile, Option<RefCell<FileMetadata>>)>,
  cancel: CancelHandle,
  name: String,
}

impl StdFileResource {
  pub fn stdio(std_file: &StdFile, name: &str) -> Self {
    Self {
      fs_file: Some((
        std_file.try_clone().map(|s| Arc::new(Mutex::new(s))).ok(),
        Some(RefCell::new(FileMetadata::default())),
      )),
      name: name.to_string(),
      ..Default::default()
    }
  }

  pub fn fs_file(fs_file: StdFile) -> Self {
    Self {
      fs_file: Some((
        Some(Arc::new(Mutex::new(fs_file))),
        Some(RefCell::new(FileMetadata::default())),
      )),
      name: "fsFile".to_string(),
      ..Default::default()
    }
  }

  async fn read(
    self: Rc<Self>,
    mut buf: ZeroCopyBuf,
  ) -> Result<(usize, ZeroCopyBuf), AnyError> {
    if self.fs_file.is_some() {
      let fs_file = self.fs_file.as_ref().unwrap();
      let std_file = fs_file.0.as_ref().unwrap().clone();
      tokio::task::spawn_blocking(
        move || -> Result<(usize, ZeroCopyBuf), AnyError> {
          let mut std_file = std_file.lock().unwrap();
          Ok((std_file.read(&mut buf)?, buf))
        },
      )
      .await?
    } else {
      Err(resource_unavailable())
    }
  }

  async fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> Result<usize, AnyError> {
    if self.fs_file.is_some() {
      let fs_file = self.fs_file.as_ref().unwrap();
      let std_file = fs_file.0.as_ref().unwrap().clone();
      tokio::task::spawn_blocking(move || {
        let mut std_file = std_file.lock().unwrap();
        std_file.write(&buf)
      })
      .await?
      .map_err(AnyError::from)
    } else {
      Err(resource_unavailable())
    }
  }

  pub fn with<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    mut f: F,
  ) -> Result<R, AnyError>
  where
    F: FnMut(Result<&mut std::fs::File, ()>) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<StdFileResource>(rid)?;
    // TODO(@AaronO): does this make sense ?
    // Sync write only works for FsFile. It doesn't make sense to do this
    // for non-blocking sockets. So we error out if not FsFile.
    if resource.fs_file.is_none() {
      return f(Err(()));
    }

    let (r, _) = resource.fs_file.as_ref().unwrap();
    match r {
      Some(r) => f(Ok(&mut r.as_ref().lock().unwrap())),
      None => Err(resource_unavailable()),
    }
  }
}

impl Resource for StdFileResource {
  fn name(&self) -> Cow<str> {
    self.name.as_str().into()
  }

  fn read_return(
    self: Rc<Self>,
    buf: ZeroCopyBuf,
  ) -> AsyncResult<(usize, ZeroCopyBuf)> {
    Box::pin(self.read(buf))
  }

  fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> AsyncResult<usize> {
    Box::pin(self.write(buf))
  }

  fn close(self: Rc<Self>) {
    // TODO: do not cancel file I/O when file is writable.
    self.cancel.cancel()
  }
}

#[op]
fn op_read_sync(
  state: &mut OpState,
  rid: ResourceId,
  mut buf: ZeroCopyBuf,
) -> Result<u32, AnyError> {
  StdFileResource::with(state, rid, move |r| match r {
    Ok(std_file) => std_file
      .read(&mut buf)
      .map(|n: usize| n as u32)
      .map_err(AnyError::from),
    Err(_) => Err(not_supported()),
  })
}

#[op]
fn op_write_sync(
  state: &mut OpState,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<u32, AnyError> {
  StdFileResource::with(state, rid, move |r| match r {
    Ok(std_file) => std_file
      .write(&buf)
      .map(|nwritten: usize| nwritten as u32)
      .map_err(AnyError::from),
    Err(_) => Err(not_supported()),
  })
}
