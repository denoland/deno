// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::not_supported;
use deno_core::error::null_opbuf;
use deno_core::error::resource_unavailable;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_net::io::TcpStreamResource;
use deno_net::io::TlsStreamResource;
use deno_net::io::UnixStreamResource;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::process;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

lazy_static::lazy_static! {
  /// Due to portability issues on Windows handle to stdout is created from raw
  /// file descriptor.  The caveat of that approach is fact that when this
  /// handle is dropped underlying file descriptor is closed - that is highly
  /// not desirable in case of stdout.  That's why we store this global handle
  /// that is then cloned when obtaining stdio for process. In turn when
  /// resource table is dropped storing reference to that handle, the handle
  /// itself won't be closed (so Deno.core.print) will still work.
  // TODO(ry) It should be possible to close stdout.
  static ref STDIN_HANDLE: Option<std::fs::File> = {
    #[cfg(not(windows))]
    let stdin = unsafe { Some(std::fs::File::from_raw_fd(0)) };
    #[cfg(windows)]
    let stdin = unsafe {
      let handle = winapi::um::processenv::GetStdHandle(
        winapi::um::winbase::STD_INPUT_HANDLE,
      );
      if handle.is_null() {
        return None;
      }
      Some(std::fs::File::from_raw_handle(handle))
    };
    stdin
  };
  static ref STDOUT_HANDLE: Option<std::fs::File> = {
    #[cfg(not(windows))]
    let stdout = unsafe { Some(std::fs::File::from_raw_fd(1)) };
    #[cfg(windows)]
    let stdout = unsafe {
      let handle = winapi::um::processenv::GetStdHandle(
        winapi::um::winbase::STD_OUTPUT_HANDLE,
      );
      if handle.is_null() {
        return None;
      }
      Some(std::fs::File::from_raw_handle(handle))
    };
    stdout
  };
  static ref STDERR_HANDLE: Option<std::fs::File> = {
    #[cfg(not(windows))]
    let stderr = unsafe { Some(std::fs::File::from_raw_fd(2)) };
    #[cfg(windows)]
    let stderr = unsafe {
      let handle = winapi::um::processenv::GetStdHandle(
        winapi::um::winbase::STD_ERROR_HANDLE,
      );
      if handle.is_null() {
        return None;
      }
      Some(std::fs::File::from_raw_handle(handle))
    };
    stderr
  };
}

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_read_async", op_async(op_read_async)),
      ("op_write_async", op_async(op_write_async)),
      ("op_read_sync", op_sync(op_read_sync)),
      ("op_write_sync", op_sync(op_write_sync)),
      ("op_shutdown", op_async(op_shutdown)),
    ])
    .build()
}

pub fn init_stdio() -> Extension {
  Extension::builder()
    .state(|state| {
      let t = &mut state.resource_table;
      let (stdin, stdout, stderr) = get_stdio();
      if let Some(stream) = stdin {
        t.add(stream);
      }
      if let Some(stream) = stdout {
        t.add(stream);
      }
      if let Some(stream) = stderr {
        t.add(stream);
      }
      Ok(())
    })
    .build()
}

pub fn get_stdio() -> (
  Option<StdFileResource>,
  Option<StdFileResource>,
  Option<StdFileResource>,
) {
  let stdin = get_stdio_stream(&STDIN_HANDLE, "stdin");
  let stdout = get_stdio_stream(&STDOUT_HANDLE, "stdout");
  let stderr = get_stdio_stream(&STDERR_HANDLE, "stderr");

  (stdin, stdout, stderr)
}

fn get_stdio_stream(
  handle: &Option<std::fs::File>,
  name: &str,
) -> Option<StdFileResource> {
  match handle {
    None => None,
    Some(file_handle) => match file_handle.try_clone() {
      Ok(clone) => {
        let tokio_file = tokio::fs::File::from_std(clone);
        Some(StdFileResource::stdio(tokio_file, name))
      }
      Err(_e) => None,
    },
  }
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

  async fn write(self: &Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let mut stream = self.borrow_mut().await;
    let nwritten = stream.write(buf).await?;
    Ok(nwritten)
  }

  async fn shutdown(self: &Rc<Self>) -> Result<(), AnyError> {
    let mut stream = self.borrow_mut().await;
    stream.shutdown().await?;
    Ok(())
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

  async fn read(self: &Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    let mut rd = self.borrow_mut().await;
    let nread = rd.read(buf).try_or_cancel(self.cancel_handle()).await?;
    Ok(nread)
  }
}

pub type ChildStdinResource = WriteOnlyResource<process::ChildStdin>;

impl Resource for ChildStdinResource {
  fn name(&self) -> Cow<str> {
    "childStdin".into()
  }
}

pub type ChildStdoutResource = ReadOnlyResource<process::ChildStdout>;

impl Resource for ChildStdoutResource {
  fn name(&self) -> Cow<str> {
    "childStdout".into()
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

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

#[derive(Debug, Default)]
pub struct StdFileResource {
  pub fs_file:
    Option<AsyncRefCell<(Option<tokio::fs::File>, Option<FileMetadata>)>>,
  cancel: CancelHandle,
  name: String,
}

impl StdFileResource {
  pub fn stdio(fs_file: tokio::fs::File, name: &str) -> Self {
    Self {
      fs_file: Some(AsyncRefCell::new((
        Some(fs_file),
        Some(FileMetadata::default()),
      ))),
      name: name.to_string(),
      ..Default::default()
    }
  }

  pub fn fs_file(fs_file: tokio::fs::File) -> Self {
    Self {
      fs_file: Some(AsyncRefCell::new((
        Some(fs_file),
        Some(FileMetadata::default()),
      ))),
      name: "fsFile".to_string(),
      ..Default::default()
    }
  }

  async fn read(self: &Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    if self.fs_file.is_some() {
      let mut fs_file = RcRef::map(&*self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let nwritten = fs_file.0.as_mut().unwrap().read(buf).await?;
      Ok(nwritten)
    } else {
      Err(resource_unavailable())
    }
  }

  async fn write(self: &Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    if self.fs_file.is_some() {
      let mut fs_file = RcRef::map(&*self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let nwritten = fs_file.0.as_mut().unwrap().write(buf).await?;
      fs_file.0.as_mut().unwrap().flush().await?;
      Ok(nwritten)
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
    // First we look up the rid in the resource table.
    let resource = state.resource_table.get::<StdFileResource>(rid)?;

    // Sync write only works for FsFile. It doesn't make sense to do this
    // for non-blocking sockets. So we error out if not FsFile.
    if resource.fs_file.is_none() {
      return f(Err(()));
    }

    // The object in the resource table is a tokio::fs::File - but in
    // order to do a blocking write on it, we must turn it into a
    // std::fs::File. Hopefully this code compiles down to nothing.
    let fs_file_resource =
      RcRef::map(&resource, |r| r.fs_file.as_ref().unwrap()).try_borrow_mut();

    if let Some(mut fs_file) = fs_file_resource {
      let tokio_file = fs_file.0.take().unwrap();
      match tokio_file.try_into_std() {
        Ok(mut std_file) => {
          let result = f(Ok(&mut std_file));
          // Turn the std_file handle back into a tokio file, put it back
          // in the resource table.
          let tokio_file = tokio::fs::File::from_std(std_file);
          fs_file.0 = Some(tokio_file);
          // return the result.
          result
        }
        Err(tokio_file) => {
          // This function will return an error containing the file if
          // some operation is in-flight.
          fs_file.0 = Some(tokio_file);
          Err(resource_unavailable())
        }
      }
    } else {
      Err(resource_unavailable())
    }
  }
}

impl Resource for StdFileResource {
  fn name(&self) -> Cow<str> {
    self.name.as_str().into()
  }

  fn close(self: Rc<Self>) {
    // TODO: do not cancel file I/O when file is writable.
    self.cancel.cancel()
  }
}

fn op_read_sync(
  state: &mut OpState,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let mut buf = buf.ok_or_else(null_opbuf)?;
  StdFileResource::with(state, rid, move |r| match r {
    Ok(std_file) => std_file
      .read(&mut buf)
      .map(|n: usize| n as u32)
      .map_err(AnyError::from),
    Err(_) => Err(not_supported()),
  })
}

async fn op_read_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let buf = &mut buf.ok_or_else(null_opbuf)?;
  let resource = state.borrow().resource_table.get_any(rid)?;
  let nread = if let Some(s) = resource.downcast_rc::<ChildStdoutResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<ChildStderrResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<StdFileResource>() {
    s.read(buf).await?
  } else {
    return Err(not_supported());
  };
  Ok(nread as u32)
}

fn op_write_sync(
  state: &mut OpState,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let buf = buf.ok_or_else(null_opbuf)?;
  StdFileResource::with(state, rid, move |r| match r {
    Ok(std_file) => std_file
      .write(&buf)
      .map(|nwritten: usize| nwritten as u32)
      .map_err(AnyError::from),
    Err(_) => Err(not_supported()),
  })
}

async fn op_write_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let buf = &buf.ok_or_else(null_opbuf)?;
  let resource = state.borrow().resource_table.get_any(rid)?;
  let nwritten = if let Some(s) = resource.downcast_rc::<ChildStdinResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<StdFileResource>() {
    s.write(buf).await?
  } else {
    return Err(not_supported());
  };
  Ok(nwritten as u32)
}

async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<(), AnyError> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  if let Some(s) = resource.downcast_rc::<ChildStdinResource>() {
    s.shutdown().await?;
  } else if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.shutdown().await?;
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.shutdown().await?;
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.shutdown().await?;
  } else {
    return Err(not_supported());
  }
  Ok(())
}
