// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::dispatch_minimal::minimal_op;
use super::dispatch_minimal::MinimalOp;
use crate::metrics::metrics_op;
use deno_core::error::bad_resource_id;
use deno_core::error::resource_unavailable;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
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
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;

#[cfg(not(windows))]
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
  rt.register_op("op_read", metrics_op(minimal_op(op_read)));
  rt.register_op("op_write", metrics_op(minimal_op(op_write)));
}

pub fn new_get_stdio() -> (
  Option<NewStreamResource>,
  Option<NewStreamResource>,
  Option<NewStreamResource>,
) {
  let stdin = new_get_stdio_stream(&STDIN_HANDLE);
  let stdout = new_get_stdio_stream(&STDOUT_HANDLE);
  let stderr = new_get_stdio_stream(&STDERR_HANDLE);

  (stdin, stdout, stderr)
}

fn new_get_stdio_stream(
  handle: &Option<std::fs::File>,
) -> Option<NewStreamResource> {
  match handle {
    None => None,
    Some(file_handle) => match file_handle.try_clone() {
      Ok(clone) => {
        let tokio_file = tokio::fs::File::from_std(clone);
        Some(NewStreamResource::fs_file(tokio_file))
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

#[derive(Default)]
pub struct NewStreamResource {
  pub fs_file:
    Option<AsyncRefCell<(Option<tokio::fs::File>, Option<FileMetadata>)>>,

  pub tcp_stream_read: Option<AsyncRefCell<tokio::net::tcp::OwnedReadHalf>>,
  pub tcp_stream_write: Option<AsyncRefCell<tokio::net::tcp::OwnedWriteHalf>>,

  #[cfg(not(windows))]
  pub unix_stream: Option<AsyncRefCell<tokio::net::UnixStream>>,

  child_stdin: Option<AsyncRefCell<tokio::process::ChildStdin>>,

  child_stdout: Option<AsyncRefCell<tokio::process::ChildStdout>>,

  child_stderr: Option<AsyncRefCell<tokio::process::ChildStderr>>,

  client_tls_stream: Option<AsyncRefCell<ClientTlsStream<TcpStream>>>,

  server_tls_stream: Option<AsyncRefCell<ServerTlsStream<TcpStream>>>,

  cancel: CancelHandle,
}

impl std::fmt::Debug for NewStreamResource {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "NewStreamResource")
  }
}

impl NewStreamResource {
  pub fn fs_file(fs_file: tokio::fs::File) -> Self {
    Self {
      fs_file: Some(AsyncRefCell::new((
        Some(fs_file),
        Some(FileMetadata::default()),
      ))),
      ..Default::default()
    }
  }

  pub fn tcp_stream(tcp_stream: tokio::net::TcpStream) -> Self {
    let (read_half, write_half) = tcp_stream.into_split();
    Self {
      tcp_stream_read: Some(AsyncRefCell::new(read_half)),
      tcp_stream_write: Some(AsyncRefCell::new(write_half)),
      ..Default::default()
    }
  }

  #[cfg(not(windows))]
  pub fn unix_stream(unix_stream: tokio::net::UnixStream) -> Self {
    Self {
      unix_stream: Some(AsyncRefCell::new(unix_stream)),
      ..Default::default()
    }
  }

  pub fn child_stdout(child: tokio::process::ChildStdout) -> Self {
    Self {
      child_stdout: Some(AsyncRefCell::new(child)),
      ..Default::default()
    }
  }

  pub fn child_stderr(child: tokio::process::ChildStderr) -> Self {
    Self {
      child_stderr: Some(AsyncRefCell::new(child)),
      ..Default::default()
    }
  }

  pub fn child_stdin(child: tokio::process::ChildStdin) -> Self {
    Self {
      child_stdin: Some(AsyncRefCell::new(child)),
      ..Default::default()
    }
  }

  pub fn client_tls_stream(stream: ClientTlsStream<TcpStream>) -> Self {
    Self {
      client_tls_stream: Some(AsyncRefCell::new(stream)),
      ..Default::default()
    }
  }

  pub fn server_tls_stream(stream: ServerTlsStream<TcpStream>) -> Self {
    Self {
      server_tls_stream: Some(AsyncRefCell::new(stream)),
      ..Default::default()
    }
  }

  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
    if self.fs_file.is_some() {
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      let mut fs_file = RcRef::map(&self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten = (*fs_file)
        .0
        .as_mut()
        .unwrap()
        .read(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nwritten);
    }

    if self.child_stdout.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      let mut child_stdout =
        RcRef::map(&self, |r| r.child_stdout.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *child_stdout).read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    if self.child_stderr.is_some() {
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      let mut child_stderr =
        RcRef::map(&self, |r| r.child_stderr.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *child_stderr).read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    if self.tcp_stream_read.is_some() {
      debug_assert!(self.tcp_stream_write.is_some());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut tcp_stream_read =
        RcRef::map(&self, |r| r.tcp_stream_read.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *tcp_stream_read)
        .read(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nread);
    }

    if self.client_tls_stream.is_some() {
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut client_tls_stream =
        RcRef::map(&self, |r| r.client_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *client_tls_stream)
        .read(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nread);
    }

    if self.server_tls_stream.is_some() {
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut server_tls_stream =
        RcRef::map(&self, |r| r.server_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *server_tls_stream)
        .read(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nread);
    }

    #[cfg(not(windows))]
    if self.unix_stream.is_some() {
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut unix_stream =
        RcRef::map(&self, |r| r.unix_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *unix_stream).read(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    Err(bad_resource_id())
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    if self.fs_file.is_some() {
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      let mut fs_file = RcRef::map(&self, |r| r.fs_file.as_ref().unwrap())
        .borrow_mut()
        .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten = (*fs_file)
        .0
        .as_mut()
        .unwrap()
        .write(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nwritten);
    }

    if self.child_stdin.is_some() {
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      let mut child_stdin =
        RcRef::map(&self, |r| r.child_stdin.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten =
        (&mut *child_stdin).write(buf).try_or_cancel(cancel).await?;
      return Ok(nwritten);
    }

    if self.tcp_stream_write.is_some() {
      debug_assert!(self.tcp_stream_read.is_some());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut tcp_stream_write =
        RcRef::map(&self, |r| r.tcp_stream_write.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten = (&mut *tcp_stream_write)
        .write(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nwritten);
    }

    if self.client_tls_stream.is_some() {
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut client_tls_stream =
        RcRef::map(&self, |r| r.client_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten = (&mut *client_tls_stream)
        .write(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nwritten);
    }

    if self.server_tls_stream.is_some() {
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut server_tls_stream =
        RcRef::map(&self, |r| r.server_tls_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nwritten = (&mut *server_tls_stream)
        .write(buf)
        .try_or_cancel(cancel)
        .await?;
      return Ok(nwritten);
    }

    #[cfg(not(windows))]
    if self.unix_stream.is_some() {
      debug_assert!(self.tcp_stream_read.is_none());
      debug_assert!(self.tcp_stream_write.is_none());
      debug_assert!(self.child_stdin.is_none());
      debug_assert!(self.child_stdout.is_none());
      debug_assert!(self.child_stderr.is_none());
      let mut unix_stream =
        RcRef::map(&self, |r| r.unix_stream.as_ref().unwrap())
          .borrow_mut()
          .await;
      let cancel = RcRef::map(self, |r| &r.cancel);
      let nread = (&mut *unix_stream).write(buf).try_or_cancel(cancel).await?;
      return Ok(nread);
    }

    Err(bad_resource_id())
  }
}

impl Resource for NewStreamResource {
  fn name(&self) -> Cow<str> {
    #[cfg(not(windows))]
    if self.unix_stream.is_some() {
      return "unixStream".into();
    }

    if self.fs_file.is_some() {
      "fsFile".into()
    } else if self.child_stdout.is_some() {
      "childStdout".into()
    } else if self.child_stderr.is_some() {
      "childStderr".into()
    } else if self.child_stdin.is_some() {
      "childStdin".into()
    } else if self.tcp_stream_read.is_some() {
      "tcpStream".into()
    } else if self.client_tls_stream.is_some() {
      "clientTlsStream".into()
    } else if self.server_tls_stream.is_some() {
      "serverTlsStream".into()
    } else {
      "<todo>".into()
    }
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
      new_std_file_resource(&mut state.borrow_mut(), rid as u32, move |r| {
        match r {
          Ok(std_file) => {
            use std::io::Read;
            std_file
              .read(&mut zero_copy[0])
              .map(|n: usize| n as i32)
              .map_err(AnyError::from)
          }
          Err(_) => Err(type_error("sync read not allowed on this resource")),
        }
      })
    })
  } else {
    let mut zero_copy = zero_copy[0].clone();
    MinimalOp::Async({
      async move {
        let resource = state
          .borrow()
          .resource_table_2
          .get::<NewStreamResource>(rid as u32)
          .ok_or_else(bad_resource_id)?;
        let nread = resource.read(&mut zero_copy).await?;
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
      new_std_file_resource(&mut state.borrow_mut(), rid as u32, move |r| {
        match r {
          Ok(std_file) => {
            use std::io::Write;
            std_file
              .write(&zero_copy[0])
              .map(|nwritten: usize| nwritten as i32)
              .map_err(AnyError::from)
          }
          Err(_) => Err(type_error("sync read not allowed on this resource")),
        }
      })
    })
  } else {
    let zero_copy = zero_copy[0].clone();
    MinimalOp::Async({
      async move {
        let resource = state
          .borrow()
          .resource_table_2
          .get::<NewStreamResource>(rid as u32)
          .ok_or_else(bad_resource_id)?;
        let nread = resource.write(&zero_copy).await?;
        Ok(nread as i32)
      }
      .boxed_local()
    })
  }
}

pub fn new_std_file_resource<F, T>(
  state: &mut OpState,
  rid: u32,
  mut f: F,
) -> Result<T, AnyError>
where
  F: FnMut(Result<&mut std::fs::File, ()>) -> Result<T, AnyError>,
{
  // First we look up the rid in the resource table.
  let resource = state
    .resource_table_2
    .get::<NewStreamResource>(rid)
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
