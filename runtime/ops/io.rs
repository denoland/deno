// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::dispatch_minimal::minimal_op;
use super::dispatch_minimal::MinimalOp;
use crate::metrics::metrics_op;
use deno_core::error::bad_resource_id;
use deno_core::error::resource_unavailable;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

lazy_static! {
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

pub fn init(rt: &mut JsRuntime) {
  rt.register_op(
    "op_read",
    metrics_op("op_read".to_string(), minimal_op(op_read)),
  );
  rt.register_op(
    "op_write",
    metrics_op("op_write".to_string(), minimal_op(op_write)),
  );
}

pub fn get_stdio() -> (
  Option<StreamResource>,
  Option<StreamResource>,
  Option<StreamResource>,
) {
  let stdin = get_stdio_stream(&STDIN_HANDLE, "stdin");
  let stdout = get_stdio_stream(&STDOUT_HANDLE, "stdout");
  let stderr = get_stdio_stream(&STDERR_HANDLE, "stderr");

  (stdin, stdout, stderr)
}

fn get_stdio_stream(
  handle: &Option<std::fs::File>,
  name: &str,
) -> Option<StreamResource> {
  match handle {
    None => None,
    Some(file_handle) => match file_handle.try_clone() {
      Ok(clone) => {
        let tokio_file = tokio::fs::File::from_std(clone);
        Some(StreamResource::stdio(tokio_file, name))
      }
      Err(_e) => None,
    },
  }
}

fn no_buffer_specified() -> AnyError {
  type_error("no buffer specified")
}

#[cfg(unix)]
use nix::sys::termios;

#[derive(Default)]
pub struct TTYMetadata {
  #[cfg(unix)]
  pub mode: Option<termios::Termios>,
}

#[derive(Default)]
pub struct FileMetadata {
  pub tty: TTYMetadata,
}

#[derive(Debug)]
pub struct FullDuplexResource<R, W> {
  rd: AsyncRefCell<R>,
  wr: AsyncRefCell<W>,
  // When a full-duplex resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures should be attached to this cancel handle.
  cancel_handle: CancelHandle,
}

impl<R: 'static, W: 'static> FullDuplexResource<R, W> {
  pub fn new((rd, wr): (R, W)) -> Self {
    Self {
      rd: rd.into(),
      wr: wr.into(),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> (R, W) {
    (self.rd.into_inner(), self.wr.into_inner())
  }

  pub fn rd_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<R> {
    RcRef::map(self, |r| &r.rd).borrow_mut()
  }

  pub fn wr_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<W> {
    RcRef::map(self, |r| &r.wr).borrow_mut()
  }

  pub fn cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.cancel_handle)
  }

  pub fn cancel_read_ops(&self) {
    self.cancel_handle.cancel()
  }
}

impl<R, W> FullDuplexResource<R, W>
where
  R: AsyncRead + Unpin + 'static,
  W: AsyncWrite + Unpin + 'static,
{
  async fn read(self: &Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    let mut rd = self.rd_borrow_mut().await;
    let nread = rd.read(buf).try_or_cancel(self.cancel_handle()).await?;
    Ok(nread)
  }

  async fn write(self: &Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let mut wr = self.wr_borrow_mut().await;
    let nwritten = wr.write(buf).await?;
    Ok(nwritten)
  }
}

pub type TcpStreamResource =
  FullDuplexResource<tcp::OwnedReadHalf, tcp::OwnedWriteHalf>;

impl Resource for TcpStreamResource {
  fn name(&self) -> Cow<str> {
    "tcpStream".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

#[derive(Default)]
pub struct StreamResource {
  pub fs_file:
    Option<AsyncRefCell<(Option<tokio::fs::File>, Option<FileMetadata>)>>,

  #[cfg(unix)]
  pub unix_stream: Option<AsyncRefCell<tokio::net::UnixStream>>,

  child_stdin: Option<AsyncRefCell<tokio::process::ChildStdin>>,

  child_stdout: Option<AsyncRefCell<tokio::process::ChildStdout>>,

  child_stderr: Option<AsyncRefCell<tokio::process::ChildStderr>>,

  client_tls_stream: Option<AsyncRefCell<ClientTlsStream<TcpStream>>>,

  server_tls_stream: Option<AsyncRefCell<ServerTlsStream<TcpStream>>>,

  cancel: CancelHandle,
  name: String,
}

impl std::fmt::Debug for StreamResource {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "StreamResource")
  }
}

impl StreamResource {
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

  #[cfg(unix)]
  pub fn unix_stream(unix_stream: tokio::net::UnixStream) -> Self {
    Self {
      unix_stream: Some(AsyncRefCell::new(unix_stream)),
      name: "unixStream".to_string(),
      ..Default::default()
    }
  }

  pub fn child_stdout(child: tokio::process::ChildStdout) -> Self {
    Self {
      child_stdout: Some(AsyncRefCell::new(child)),
      name: "childStdout".to_string(),
      ..Default::default()
    }
  }

  pub fn child_stderr(child: tokio::process::ChildStderr) -> Self {
    Self {
      child_stderr: Some(AsyncRefCell::new(child)),
      name: "childStderr".to_string(),
      ..Default::default()
    }
  }

  pub fn child_stdin(child: tokio::process::ChildStdin) -> Self {
    Self {
      child_stdin: Some(AsyncRefCell::new(child)),
      name: "childStdin".to_string(),
      ..Default::default()
    }
  }

  pub fn client_tls_stream(stream: ClientTlsStream<TcpStream>) -> Self {
    Self {
      client_tls_stream: Some(AsyncRefCell::new(stream)),
      name: "clientTlsStream".to_string(),
      ..Default::default()
    }
  }

  pub fn server_tls_stream(stream: ServerTlsStream<TcpStream>) -> Self {
    Self {
      server_tls_stream: Some(AsyncRefCell::new(stream)),
      name: "serverTlsStream".to_string(),
      ..Default::default()
    }
  }

  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    // TODO(bartlomieju): in the future, it would be better for `StreamResource`
    // to be an enum instead a struct with many `Option` fields, however I
    // wasn't able to get it to work with `AsyncRefCell`s.
    if self.fs_file.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.server_tls_stream.is_none());
      debug_assert!(self.client_tls_stream.is_none());
      let mut fs_file = RcRef::map(&self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let nwritten = (*fs_file).0.as_mut().unwrap().read(buf).await?;
      return Ok(nwritten);
    } else if self.child_stdout.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.server_tls_stream.is_none());
      debug_assert!(self.client_tls_stream.is_none());
      let mut child_stdout =
        RcRef::map(&self, |r| r.child_stdout.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = child_stdout.read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    } else if self.child_stderr.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.server_tls_stream.is_none());
      debug_assert!(self.client_tls_stream.is_none());
      let mut child_stderr =
        RcRef::map(&self, |r| r.child_stderr.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = child_stderr.read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    } else if self.client_tls_stream.is_some() {
      debug_assert!(self.server_tls_stream.is_none());
      let mut client_tls_stream =
        RcRef::map(&self, |r| r.client_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = client_tls_stream.read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    } else if self.server_tls_stream.is_some() {
      let mut server_tls_stream =
        RcRef::map(&self, |r| r.server_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = server_tls_stream.read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    #[cfg(unix)]
    if self.unix_stream.is_some() {
      let mut unix_stream =
        RcRef::map(&self, |r| r.unix_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = unix_stream.read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    Err(bad_resource_id())
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    // TODO(bartlomieju): in the future, it would be better for `StreamResource`
    // to be an enum instead a struct with many `Option` fields, however I
    // wasn't able to get it to work with `AsyncRefCell`s.
    if self.fs_file.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.server_tls_stream.is_none());
      debug_assert!(self.client_tls_stream.is_none());
      let mut fs_file = RcRef::map(&self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let nwritten = (*fs_file).0.as_mut().unwrap().write(buf).await?;
      (*fs_file).0.as_mut().unwrap().flush().await?;
      return Ok(nwritten);
    } else if self.child_stdin.is_some() {
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.server_tls_stream.is_none());
      debug_assert!(self.client_tls_stream.is_none());
      let mut child_stdin =
        RcRef::map(&self, |r| r.child_stdin.as_ref().unwrap())
          .borrow_mut()
          .await;
      let nwritten = child_stdin.write(buf).await?;
      child_stdin.flush().await?;
      return Ok(nwritten);
    } else if self.client_tls_stream.is_some() {
      debug_assert!(self.server_tls_stream.is_none());
      let mut client_tls_stream =
        RcRef::map(&self, |r| r.client_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let nwritten = client_tls_stream.write(buf).await?;
      client_tls_stream.flush().await?;
      return Ok(nwritten);
    } else if self.server_tls_stream.is_some() {
      let mut server_tls_stream =
        RcRef::map(&self, |r| r.server_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let nwritten = server_tls_stream.write(buf).await?;
      server_tls_stream.flush().await?;
      return Ok(nwritten);
    }

    #[cfg(unix)]
    if self.unix_stream.is_some() {
      let mut unix_stream =
        RcRef::map(&self, |r| r.unix_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let nwritten = unix_stream.write(buf).await?;
      unix_stream.flush().await?;
      return Ok(nwritten);
    }

    Err(bad_resource_id())
  }
}

impl Resource for StreamResource {
  fn name(&self) -> Cow<str> {
    self.name.clone().into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

pub fn op_read(
  state: Rc<RefCell<OpState>>,
  is_sync: bool,
  rid: i32,
  mut zero_copy: BufVec,
) -> MinimalOp {
  debug!("read rid={}", rid);
  match zero_copy.len() {
    0 => return MinimalOp::Sync(Err(no_buffer_specified())),
    1 => {}
    _ => panic!("Invalid number of arguments"),
  }

  if is_sync {
    MinimalOp::Sync({
      // First we look up the rid in the resource table.
      std_file_resource(&mut state.borrow_mut(), rid as u32, move |r| match r {
        Ok(std_file) => {
          use std::io::Read;
          std_file
            .read(&mut zero_copy[0])
            .map(|n: usize| n as i32)
            .map_err(AnyError::from)
        }
        Err(_) => Err(type_error("sync read not allowed on this resource")),
      })
    })
  } else {
    let mut zero_copy = zero_copy[0].clone();
    MinimalOp::Async({
      async move {
        let resource = state
          .borrow()
          .resource_table
          .get_any(rid as u32)
          .ok_or_else(bad_resource_id)?;
        let nread = if let Some(stream) =
          resource.downcast_rc::<TcpStreamResource>()
        {
          stream.read(&mut zero_copy).await?
        } else if let Some(stream) = resource.downcast_rc::<StreamResource>() {
          stream.clone().read(&mut zero_copy).await?
        } else {
          return Err(bad_resource_id());
        };
        Ok(nread as i32)
      }
      .boxed_local()
    })
  }
}

pub fn op_write(
  state: Rc<RefCell<OpState>>,
  is_sync: bool,
  rid: i32,
  zero_copy: BufVec,
) -> MinimalOp {
  debug!("write rid={}", rid);
  match zero_copy.len() {
    0 => return MinimalOp::Sync(Err(no_buffer_specified())),
    1 => {}
    _ => panic!("Invalid number of arguments"),
  }

  if is_sync {
    MinimalOp::Sync({
      // First we look up the rid in the resource table.
      std_file_resource(&mut state.borrow_mut(), rid as u32, move |r| match r {
        Ok(std_file) => {
          use std::io::Write;
          std_file
            .write(&zero_copy[0])
            .map(|nwritten: usize| nwritten as i32)
            .map_err(AnyError::from)
        }
        Err(_) => Err(type_error("sync read not allowed on this resource")),
      })
    })
  } else {
    let zero_copy = zero_copy[0].clone();
    MinimalOp::Async({
      async move {
        let resource = state
          .borrow()
          .resource_table
          .get_any(rid as u32)
          .ok_or_else(bad_resource_id)?;
        let nwritten = if let Some(stream) =
          resource.downcast_rc::<TcpStreamResource>()
        {
          stream.write(&zero_copy).await?
        } else if let Some(stream) = resource.downcast_rc::<StreamResource>() {
          stream.clone().write(&zero_copy).await?
        } else {
          return Err(bad_resource_id());
        };
        Ok(nwritten as i32)
      }
      .boxed_local()
    })
  }
}

pub fn std_file_resource<F, T>(
  state: &mut OpState,
  rid: u32,
  mut f: F,
) -> Result<T, AnyError>
where
  F: FnMut(Result<&mut std::fs::File, ()>) -> Result<T, AnyError>,
{
  // First we look up the rid in the resource table.
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(bad_resource_id)?;

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
