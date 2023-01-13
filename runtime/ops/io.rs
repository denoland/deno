// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::parking_lot::Mutex;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
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
pub static STDIN_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stdin
  unsafe { StdFile::from_raw_fd(0) }
});
#[cfg(unix)]
pub static STDOUT_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stdout
  unsafe { StdFile::from_raw_fd(1) }
});
#[cfg(unix)]
pub static STDERR_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stderr
  unsafe { StdFile::from_raw_fd(2) }
});

#[cfg(windows)]
pub static STDIN_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stdin
  unsafe { StdFile::from_raw_handle(GetStdHandle(winbase::STD_INPUT_HANDLE)) }
});
#[cfg(windows)]
pub static STDOUT_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stdout
  unsafe { StdFile::from_raw_handle(GetStdHandle(winbase::STD_OUTPUT_HANDLE)) }
});
#[cfg(windows)]
pub static STDERR_HANDLE: Lazy<StdFile> = Lazy::new(|| {
  // SAFETY: corresponds to OS stderr
  unsafe { StdFile::from_raw_handle(GetStdHandle(winbase::STD_ERROR_HANDLE)) }
});

pub fn init() -> Extension {
  Extension::builder("deno_io")
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

  Extension::builder("deno_stdio")
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

      let rid = t.add(StdFileResource::stdio(
        match stdio.stdin {
          StdioPipe::Inherit => StdFileResourceInner {
            kind: StdFileResourceKind::Stdin,
            file: STDIN_HANDLE.try_clone().unwrap(),
          },
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stdin",
      ));
      assert_eq!(rid, 0, "stdin must have ResourceId 0");

      let rid = t.add(StdFileResource::stdio(
        match stdio.stdout {
          StdioPipe::Inherit => StdFileResourceInner {
            kind: StdFileResourceKind::Stdout,
            file: STDOUT_HANDLE.try_clone().unwrap(),
          },
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stdout",
      ));
      assert_eq!(rid, 1, "stdout must have ResourceId 1");

      let rid = t.add(StdFileResource::stdio(
        match stdio.stderr {
          StdioPipe::Inherit => StdFileResourceInner {
            kind: StdFileResourceKind::Stderr,
            file: STDERR_HANDLE.try_clone().unwrap(),
          },
          StdioPipe::File(pipe) => StdFileResourceInner::file(pipe),
        },
        "stderr",
      ));
      assert_eq!(rid, 2, "stderr must have ResourceId 2");
      Ok(())
    })
    .build()
}

#[cfg(unix)]
use nix::sys::termios;

use super::utils::TaskQueue;

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

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    let mut stream = self.borrow_mut().await;
    let nwritten = stream.write(data).await?;
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

  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, AnyError> {
    let mut rd = self.borrow_mut().await;
    let nread = rd.read(data).try_or_cancel(self.cancel_handle()).await?;
    Ok(nread)
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

  deno_core::impl_writable!();

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown())
  }
}

pub type ChildStdoutResource = ReadOnlyResource<process::ChildStdout>;

impl Resource for ChildStdoutResource {
  deno_core::impl_readable_byob!();

  fn name(&self) -> Cow<str> {
    "childStdout".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

pub type ChildStderrResource = ReadOnlyResource<process::ChildStderr>;

impl Resource for ChildStderrResource {
  deno_core::impl_readable_byob!();

  fn name(&self) -> Cow<str> {
    "childStderr".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

#[derive(Clone, Copy)]
enum StdFileResourceKind {
  File,
  // For stdout and stderr, we sometimes instead use std::io::stdout() directly,
  // because we get some Windows specific functionality for free by using Rust
  // std's wrappers. So we take a bit of a complexity hit in order to not
  // have to duplicate the functionality in Rust's std/src/sys/windows/stdio.rs
  Stdin,
  Stdout,
  Stderr,
}

struct StdFileResourceInner {
  kind: StdFileResourceKind,
  file: StdFile,
}

impl StdFileResourceInner {
  pub fn file(fs_file: StdFile) -> Self {
    StdFileResourceInner {
      kind: StdFileResourceKind::File,
      file: fs_file,
    }
  }

  pub fn with_file<R>(&mut self, f: impl FnOnce(&mut StdFile) -> R) -> R {
    f(&mut self.file)
  }

  pub fn try_clone(&self) -> Result<Self, std::io::Error> {
    Ok(Self {
      kind: self.kind,
      file: self.file.try_clone()?,
    })
  }

  pub fn write_and_maybe_flush(
    &mut self,
    buf: &[u8],
  ) -> Result<usize, AnyError> {
    // Rust will line buffer and we don't want that behavior
    // (see https://github.com/denoland/deno/issues/948), so flush stdout and stderr.
    // Although an alternative solution could be to bypass Rust's std by
    // using the raw fds/handles, it will cause encoding issues on Windows
    // that we get solved for free by using Rust's stdio wrappers (see
    // std/src/sys/windows/stdio.rs in Rust's source code).
    match self.kind {
      StdFileResourceKind::File => Ok(self.file.write(buf)?),
      StdFileResourceKind::Stdin => {
        Err(Into::<std::io::Error>::into(ErrorKind::Unsupported).into())
      }
      StdFileResourceKind::Stdout => {
        // bypass the file and use std::io::stdout()
        let mut stdout = std::io::stdout().lock();
        let nwritten = stdout.write(buf)?;
        stdout.flush()?;
        Ok(nwritten)
      }
      StdFileResourceKind::Stderr => {
        // bypass the file and use std::io::stderr()
        let mut stderr = std::io::stderr().lock();
        let nwritten = stderr.write(buf)?;
        stderr.flush()?;
        Ok(nwritten)
      }
    }
  }

  pub fn write_all_and_maybe_flush(
    &mut self,
    buf: &[u8],
  ) -> Result<(), AnyError> {
    // this method exists instead of using a `Write` implementation
    // so that we can acquire the locks once and do both actions
    match self.kind {
      StdFileResourceKind::File => Ok(self.file.write_all(buf)?),
      StdFileResourceKind::Stdin => {
        Err(Into::<std::io::Error>::into(ErrorKind::Unsupported).into())
      }
      StdFileResourceKind::Stdout => {
        // bypass the file and use std::io::stdout()
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(buf)?;
        stdout.flush()?;
        Ok(())
      }
      StdFileResourceKind::Stderr => {
        // bypass the file and use std::io::stderr()
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(buf)?;
        stderr.flush()?;
        Ok(())
      }
    }
  }
}

impl Read for StdFileResourceInner {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self.kind {
      StdFileResourceKind::File | StdFileResourceKind::Stdin => {
        self.file.read(buf)
      }
      StdFileResourceKind::Stdout | StdFileResourceKind::Stderr => {
        Err(ErrorKind::Unsupported.into())
      }
    }
  }
}

struct StdFileResourceCellValue {
  inner: StdFileResourceInner,
  meta_data: Arc<Mutex<FileMetadata>>,
}

impl StdFileResourceCellValue {
  pub fn try_clone(&self) -> Result<Self, std::io::Error> {
    Ok(Self {
      inner: self.inner.try_clone()?,
      meta_data: self.meta_data.clone(),
    })
  }
}

pub struct StdFileResource {
  name: String,
  // We can't use an AsyncRefCell here because we need to allow
  // access to the resource synchronously at any time and
  // asynchronously one at a time in order
  cell: RefCell<Option<StdFileResourceCellValue>>,
  // Used to keep async actions in order and only allow one
  // to occur at a time
  cell_async_task_queue: TaskQueue,
}

impl StdFileResource {
  fn stdio(inner: StdFileResourceInner, name: &str) -> Self {
    Self {
      cell: RefCell::new(Some(StdFileResourceCellValue {
        inner,
        meta_data: Default::default(),
      })),
      cell_async_task_queue: Default::default(),
      name: name.to_string(),
    }
  }

  pub fn fs_file(fs_file: StdFile) -> Self {
    Self {
      cell: RefCell::new(Some(StdFileResourceCellValue {
        inner: StdFileResourceInner::file(fs_file),
        meta_data: Default::default(),
      })),
      cell_async_task_queue: Default::default(),
      name: "fsFile".to_string(),
    }
  }

  fn with_inner_and_metadata<TResult>(
    self: Rc<Self>,
    action: impl FnOnce(
      &mut StdFileResourceInner,
      &Arc<Mutex<FileMetadata>>,
    ) -> Result<TResult, AnyError>,
  ) -> Result<TResult, AnyError> {
    match self.cell.try_borrow_mut() {
      Ok(mut cell) => {
        let mut file = cell.take().unwrap();
        let result = action(&mut file.inner, &file.meta_data);
        cell.replace(file);
        result
      }
      Err(_) => Err(resource_unavailable()),
    }
  }

  async fn with_inner_blocking_task<F, R: Send + 'static>(
    self: Rc<Self>,
    action: F,
  ) -> R
  where
    F: FnOnce(&mut StdFileResourceInner) -> R + Send + 'static,
  {
    // we want to restrict this to one async action at a time
    let _permit = self.cell_async_task_queue.acquire().await;
    // we take the value out of the cell, use it on a blocking task,
    // then put it back into the cell when we're done
    let mut did_take = false;
    let mut cell_value = {
      let mut cell = self.cell.borrow_mut();
      match cell.as_mut().unwrap().try_clone() {
        Ok(value) => value,
        Err(_) => {
          did_take = true;
          cell.take().unwrap()
        }
      }
    };
    let (cell_value, result) = tokio::task::spawn_blocking(move || {
      let result = action(&mut cell_value.inner);
      (cell_value, result)
    })
    .await
    .unwrap();

    if did_take {
      // put it back
      self.cell.borrow_mut().replace(cell_value);
    }

    result
  }

  async fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> Result<(usize, BufMutView), AnyError> {
    self
      .with_inner_blocking_task(move |inner| {
        let nread = inner.read(&mut buf)?;
        Ok((nread, buf))
      })
      .await
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    let buf = data.to_owned();
    self
      .with_inner_blocking_task(move |inner| inner.write_and_maybe_flush(&buf))
      .await
  }

  async fn write_all(self: Rc<Self>, data: &[u8]) -> Result<(), AnyError> {
    let buf = data.to_owned();
    self
      .with_inner_blocking_task(move |inner| {
        inner.write_all_and_maybe_flush(&buf)
      })
      .await
  }

  fn with_resource<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(Rc<StdFileResource>) -> Result<R, AnyError>,
  {
    let resource = state.resource_table.get::<StdFileResource>(rid)?;
    f(resource)
  }

  pub fn with_file<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(&mut StdFile) -> Result<R, AnyError>,
  {
    Self::with_resource(state, rid, move |resource| {
      resource.with_inner_and_metadata(move |inner, _| inner.with_file(f))
    })
  }

  pub fn with_file_and_metadata<F, R>(
    state: &mut OpState,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: FnOnce(&mut StdFile, &Arc<Mutex<FileMetadata>>) -> Result<R, AnyError>,
  {
    Self::with_resource(state, rid, move |resource| {
      resource.with_inner_and_metadata(move |inner, metadata| {
        inner.with_file(move |file| f(file, metadata))
      })
    })
  }

  pub async fn with_file_blocking_task<F, R: Send + 'static>(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
    f: F,
  ) -> Result<R, AnyError>
  where
    F: (FnOnce(&mut StdFile) -> Result<R, AnyError>) + Send + 'static,
  {
    let resource = state
      .borrow_mut()
      .resource_table
      .get::<StdFileResource>(rid)?;

    resource
      .with_inner_blocking_task(move |inner| inner.with_file(f))
      .await
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
    Self::with_resource(state, rid, |resource| {
      resource.with_inner_and_metadata(|inner, _| match inner.kind {
        StdFileResourceKind::File => {
          let file = inner.file.try_clone()?;
          Ok(file.into())
        }
        _ => Ok(std::process::Stdio::inherit()),
      })
    })
  }
}

impl Resource for StdFileResource {
  fn name(&self) -> Cow<str> {
    self.name.as_str().into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<deno_core::BufView> {
    Box::pin(async move {
      let vec = vec![0; limit];
      let buf = BufMutView::from(vec);
      let (nread, buf) = self.read_byob(buf).await?;
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
  ) -> AsyncResult<(usize, deno_core::BufMutView)> {
    Box::pin(self.read_byob(buf))
  }

  deno_core::impl_writable!(with_all);

  #[cfg(unix)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::unix::prelude::RawFd> {
    use std::os::unix::io::AsRawFd;
    self
      .with_inner_and_metadata(move |std_file, _| {
        Ok(std_file.with_file(|f| f.as_raw_fd()))
      })
      .ok()
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
  StdFileResource::with_resource(state, rid, move |resource| {
    resource.with_inner_and_metadata(|inner, _| {
      inner.write_all_and_maybe_flush(msg.as_bytes())?;
      Ok(())
    })
  })
}

#[op(fast)]
fn op_read_sync(
  state: &mut OpState,
  rid: u32,
  buf: &mut [u8],
) -> Result<u32, AnyError> {
  StdFileResource::with_resource(state, rid, move |resource| {
    resource.with_inner_and_metadata(|inner, _| {
      inner
        .read(buf)
        .map(|n: usize| n as u32)
        .map_err(AnyError::from)
    })
  })
}

#[op(fast)]
fn op_write_sync(
  state: &mut OpState,
  rid: u32,
  buf: &mut [u8],
) -> Result<u32, AnyError> {
  StdFileResource::with_resource(state, rid, move |resource| {
    resource.with_inner_and_metadata(|inner, _| {
      inner
        .write_and_maybe_flush(buf)
        .map(|nwritten: usize| nwritten as u32)
        .map_err(AnyError::from)
    })
  })
}
