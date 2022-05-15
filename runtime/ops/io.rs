// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::parking_lot::Mutex;
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
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
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

// Store the stdio fd/handles in global statics in order to keep them
// alive for the duration of the application since the last handle/fd
// being dropped will close the corresponding pipe.
#[cfg(unix)]
static STDIN_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(0) });
#[cfg(unix)]
static STDOUT_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(1) });
#[cfg(unix)]
static STDERR_HANDLE: Lazy<StdFile> =
  Lazy::new(|| unsafe { StdFile::from_raw_fd(2) });

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

pub enum StdioPipe {
  Inherit,
  File(StdFile),
}

impl Default for StdioPipe {
  fn default() -> Self {
    Self::Inherit
  }
}

impl Clone for StdioPipe {
  fn clone(&self) -> Self {
    match self {
      StdioPipe::Inherit => StdioPipe::Inherit,
      StdioPipe::File(pipe) => StdioPipe::File(pipe.try_clone().unwrap()),
    }
  }
}

/// Specify how stdin, stdout, and stderr are piped.
/// By default, inherits from the process.
#[derive(Clone, Default)]
pub struct Stdio {
  pub stdin: StdioPipe,
  pub stdout: StdioPipe,
  pub stderr: StdioPipe,
}

pub fn init_stdio(stdio: Stdio) -> Extension {
  // todo(dsheret): don't do this? Taking out the writers was necessary to prevent invalid handle panics
  let stdio = Rc::new(RefCell::new(Some(stdio)));

  Extension::builder()
    .middleware(|op| match op.name {
      "op_print" => op_print::decl(),
      _ => op,
    })
    .state(move |state| {
      let stdio = stdio
        .borrow_mut()
        .take()
        .expect("Extension only supports being used once.");
      let t = &mut state.resource_table;
      t.add(StdFileResource::stdio(
        match stdio.stdin {
          StdioPipe::Inherit => StdFileResourceInner::Stdin,
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stdin",
      ));
      t.add(StdFileResource::stdio(
        match stdio.stdout {
          StdioPipe::Inherit => StdFileResourceInner::Stdout,
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stdout",
      ));
      t.add(StdFileResource::stdio(
        match stdio.stderr {
          StdioPipe::Inherit => StdFileResourceInner::Stderr,
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stderr",
      ));
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

#[derive(Clone)]
enum StdFileResourceInner {
  // Ideally we would store stdio as an StdFile, but we get some Windows
  // specific functionality for free by using Rust std's wrappers. So we
  // take a bit of a complexity hit here in order to not have to duplicate
  // the functionality in Rust's std/src/sys/windows/stdio.rs
  Stdin,
  Stdout,
  Stderr,
  File(Arc<Mutex<StdFile>>),
}

impl StdFileResourceInner {
  pub fn file(fs_file: StdFile) -> Self {
    StdFileResourceInner::File(Arc::new(Mutex::new(fs_file)))
  }

  pub fn with_file<R>(&self, mut f: impl FnMut(&mut StdFile) -> R) -> R {
    match self {
      Self::Stdin => f(&mut STDIN_HANDLE.try_clone().unwrap()),
      Self::Stdout => f(&mut STDOUT_HANDLE.try_clone().unwrap()),
      Self::Stderr => f(&mut STDERR_HANDLE.try_clone().unwrap()),
      Self::File(file) => {
        let mut file = file.lock();
        f(&mut file)
      }
    }
  }

  pub fn write_and_maybe_flush(
    &mut self,
    buf: &[u8],
  ) -> Result<usize, AnyError> {
    let nwritten = self.write(buf)?;
    if !matches!(self, StdFileResourceInner::File(_)) {
      // Rust will line buffer and we don't want that behavior
      // (see https://github.com/denoland/deno/issues/948), so flush.
      // Although an alternative solution could be to bypass Rust's std by
      // using the raw fds/handles, it will cause encoding issues on Windows
      // that we get solved for free by using Rust's stdio wrappers (see
      // std/src/sys/windows/stdio.rs in Rust's source code).
      self.flush()?;
    }
    Ok(nwritten)
  }
}

impl Read for StdFileResourceInner {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self {
      Self::Stdout => Err(ErrorKind::Unsupported.into()),
      Self::Stderr => Err(ErrorKind::Unsupported.into()),
      Self::Stdin => std::io::stdin().read(buf),
      Self::File(file) => file.lock().read(buf),
    }
  }
}

impl Write for StdFileResourceInner {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self {
      Self::Stdout => std::io::stdout().write(buf),
      Self::Stderr => std::io::stderr().write(buf),
      Self::Stdin => Err(ErrorKind::Unsupported.into()),
      Self::File(file) => file.lock().write(buf),
    }
  }

  fn flush(&mut self) -> std::io::Result<()> {
    match self {
      Self::Stdout => std::io::stdout().flush(),
      Self::Stderr => std::io::stderr().flush(),
      Self::Stdin => Err(ErrorKind::Unsupported.into()),
      Self::File(file) => file.lock().flush(),
    }
  }
}

pub struct StdFileResource {
  inner: StdFileResourceInner,
  metadata: RefCell<FileMetadata>,
  name: String,
}

impl StdFileResource {
  fn stdio(inner: StdFileResourceInner, name: &str) -> Self {
    Self {
      inner,
      metadata: Default::default(),
      name: name.to_string(),
    }
  }

  pub fn fs_file(fs_file: StdFile) -> Self {
    Self {
      inner: StdFileResourceInner::file(fs_file),
      metadata: Default::default(),
      name: "fsFile".to_string(),
    }
  }

  pub fn std_file(&self) -> Arc<Mutex<StdFile>> {
    match &self.inner {
      StdFileResourceInner::File(fs_file) => fs_file.clone(),
      StdFileResourceInner::Stdin => {
        Arc::new(Mutex::new(STDIN_HANDLE.try_clone().unwrap()))
      }
      StdFileResourceInner::Stdout => {
        Arc::new(Mutex::new(STDOUT_HANDLE.try_clone().unwrap()))
      }
      StdFileResourceInner::Stderr => {
        Arc::new(Mutex::new(STDERR_HANDLE.try_clone().unwrap()))
      }
    }
  }

  pub fn metadata_mut(&self) -> std::cell::RefMut<FileMetadata> {
    self.metadata.borrow_mut()
  }

  async fn read(
    self: Rc<Self>,
    mut buf: ZeroCopyBuf,
  ) -> Result<(usize, ZeroCopyBuf), AnyError> {
    let mut inner = self.inner.clone();
    tokio::task::spawn_blocking(
      move || -> Result<(usize, ZeroCopyBuf), AnyError> {
        Ok((inner.read(&mut buf)?, buf))
      },
    )
    .await?
  }

  async fn write(self: Rc<Self>, buf: ZeroCopyBuf) -> Result<usize, AnyError> {
    let mut inner = self.inner.clone();
    tokio::task::spawn_blocking(move || inner.write_and_maybe_flush(&buf))
      .await?
      .map_err(AnyError::from)
  }

  fn with_inner<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    mut f: F,
  ) -> Result<R, AnyError>
  where
    F: FnMut(StdFileResourceInner) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<StdFileResource>(rid)?;
    f(resource.inner.clone())
  }

  pub fn with_file<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnMut(&mut StdFile) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<StdFileResource>(rid)?;
    resource.inner.with_file(f)
  }

  pub fn clone_file(
    state: &mut OpState,
    rid: ResourceId,
  ) -> Result<StdFile, AnyError> {
    Self::with_file(state, rid, move |std_file| {
      std_file.try_clone().map_err(AnyError::from)
    })
  }

  pub fn as_stdio(
    state: &mut OpState,
    rid: u32,
  ) -> Result<std::process::Stdio, AnyError> {
    Self::with_inner(state, rid, |inner| match inner {
      StdFileResourceInner::File(file) => {
        let file = file.lock().try_clone()?;
        Ok(file.into())
      }
      _ => Ok(std::process::Stdio::inherit()),
    })
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
}

// override op_print to use the stdout and stderr in the resource table
#[op]
pub fn op_print(
  state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  let rid = if is_err { 2 } else { 1 };
  StdFileResource::with_inner(state, rid, move |mut inner| {
    inner.write_all(msg.as_bytes())?;
    inner.flush().unwrap();
    Ok(())
  })
}

#[op]
fn op_read_sync(
  state: &mut OpState,
  rid: ResourceId,
  mut buf: ZeroCopyBuf,
) -> Result<u32, AnyError> {
  StdFileResource::with_inner(state, rid, move |mut inner| {
    inner
      .read(&mut buf)
      .map(|n: usize| n as u32)
      .map_err(AnyError::from)
  })
}

#[op]
fn op_write_sync(
  state: &mut OpState,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<u32, AnyError> {
  StdFileResource::with_inner(state, rid, move |mut inner| {
    inner
      .write_and_maybe_flush(&buf)
      .map(|nwritten: usize| nwritten as u32)
      .map_err(AnyError::from)
  })
}
