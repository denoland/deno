// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::dispatch_minimal::minimal_op;
use super::dispatch_minimal::MinimalOp;
use crate::metrics::metrics_op;
use deno_core::error::bad_resource_id;
use deno_core::error::resource_unavailable;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::futures::ready;
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
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncRead, AsyncWrite};
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

pub fn get_stdio() -> (
  Option<StreamResourceHolder>,
  Option<StreamResourceHolder>,
  Option<StreamResourceHolder>,
) {
  let stdin = get_stdio_stream(&STDIN_HANDLE);
  let stdout = get_stdio_stream(&STDOUT_HANDLE);
  let stderr = get_stdio_stream(&STDERR_HANDLE);

  (stdin, stdout, stderr)
}

fn get_stdio_stream(
  handle: &Option<std::fs::File>,
) -> Option<StreamResourceHolder> {
  match handle {
    None => None,
    Some(file_handle) => match file_handle.try_clone() {
      Ok(clone) => Some(StreamResourceHolder::new(StreamResource::FsFile(
        Some((tokio::fs::File::from_std(clone), FileMetadata::default())),
      ))),
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

pub struct StreamResourceHolder {
  pub resource: StreamResource,
  waker: HashMap<usize, futures::task::AtomicWaker>,
  waker_counter: AtomicUsize,
}

impl StreamResourceHolder {
  pub fn new(resource: StreamResource) -> StreamResourceHolder {
    StreamResourceHolder {
      resource,
      // Atleast one task is expecter for the resource
      waker: HashMap::with_capacity(1),
      // Tracks wakers Ids
      waker_counter: AtomicUsize::new(0),
    }
  }
}

impl Drop for StreamResourceHolder {
  fn drop(&mut self) {
    self.wake_tasks();
  }
}

impl StreamResourceHolder {
  pub fn track_task(&mut self, cx: &Context) -> Result<usize, AnyError> {
    let waker = futures::task::AtomicWaker::new();
    waker.register(cx.waker());
    // Its OK if it overflows
    let task_waker_id = self.waker_counter.fetch_add(1, Ordering::Relaxed);
    self.waker.insert(task_waker_id, waker);
    Ok(task_waker_id)
  }

  pub fn wake_tasks(&mut self) {
    for waker in self.waker.values() {
      waker.wake();
    }
  }

  pub fn untrack_task(&mut self, task_waker_id: usize) {
    self.waker.remove(&task_waker_id);
  }
}

#[allow(dead_code)]
pub enum StreamResource {
  FsFile(Option<(tokio::fs::File, FileMetadata)>),
  TcpStream(Option<tokio::net::TcpStream>),
  #[cfg(not(windows))]
  UnixStream(tokio::net::UnixStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  ChildStdin(tokio::process::ChildStdin),
  ChildStdout(tokio::process::ChildStdout),
  ChildStderr(tokio::process::ChildStderr),
}

trait UnpinAsyncRead: AsyncRead + Unpin {}
trait UnpinAsyncWrite: AsyncWrite + Unpin {}

impl<T: AsyncRead + Unpin> UnpinAsyncRead for T {}
impl<T: AsyncWrite + Unpin> UnpinAsyncWrite for T {}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `AnyError` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, AnyError>>;
}

impl DenoAsyncRead for StreamResource {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, AnyError>> {
    use StreamResource::*;
    let f: &mut dyn UnpinAsyncRead = match self {
      FsFile(Some((f, _))) => f,
      FsFile(None) => return Poll::Ready(Err(resource_unavailable())),
      TcpStream(Some(f)) => f,
      #[cfg(not(windows))]
      UnixStream(f) => f,
      ClientTlsStream(f) => f,
      ServerTlsStream(f) => f,
      ChildStdout(f) => f,
      ChildStderr(f) => f,
      _ => return Err(bad_resource_id()).into(),
    };
    let v = ready!(Pin::new(f).poll_read(cx, buf))?;
    Ok(v).into()
  }
}

// pub enum NewStreamResource {
//   // FsFile(Option<(tokio::fs::File, FileMetadata)>),
//   // TcpStream(Option<tokio::net::TcpStream>),
//   // #[cfg(not(windows))]
//   // UnixStream(tokio::net::UnixStream),
//   // ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
//   // ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
// }

#[derive(Default)]
pub struct NewStreamResource {
  pub tcp_stream_read: Option<AsyncRefCell<tokio::net::tcp::OwnedReadHalf>>,
  pub tcp_stream_write: Option<AsyncRefCell<tokio::net::tcp::OwnedWriteHalf>>,

  child_stdin: Option<AsyncRefCell<tokio::process::ChildStdin>>,

  child_stdout: Option<AsyncRefCell<tokio::process::ChildStdout>>,

  child_stderr: Option<AsyncRefCell<tokio::process::ChildStderr>>,

  cancel: CancelHandle,
}

impl std::fmt::Debug for NewStreamResource {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "NewStreamResource")
  }
}

impl NewStreamResource {
  pub fn tcp_stream(tcp_stream: tokio::net::TcpStream) -> Self {
    let (read_half, write_half) = tcp_stream.into_split();
    Self {
      tcp_stream_read: Some(AsyncRefCell::new(read_half)),
      tcp_stream_write: Some(AsyncRefCell::new(write_half)),
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

  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, AnyError> {
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

    Err(bad_resource_id())
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
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

    Err(bad_resource_id())
  }
}

impl Resource for NewStreamResource {
  fn name(&self) -> Cow<str> {
    if self.child_stdout.is_some() {
      "childStdout".into()
    } else if self.child_stderr.is_some() {
      "childStderr".into()
    } else if self.child_stdin.is_some() {
      "childStdin".into()
    } else if self.tcp_stream_read.is_some() {
      "tcpStream".into()
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
      if rid >= 1_000_000 {
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
      } else {
        poll_fn(move |cx| {
          let mut state = state.borrow_mut();
          let resource_holder = state
            .resource_table
            .get_mut::<StreamResourceHolder>(rid as u32)
            .ok_or_else(bad_resource_id)?;

          let mut task_tracker_id: Option<usize> = None;
          let nread =
            match resource_holder.resource.poll_read(cx, &mut zero_copy) {
              Poll::Ready(t) => {
                if let Some(id) = task_tracker_id {
                  resource_holder.untrack_task(id);
                }
                t
              }
              Poll::Pending => {
                task_tracker_id.replace(resource_holder.track_task(cx)?);
                return Poll::Pending;
              }
            }?;
          Poll::Ready(Ok(nread as i32))
        })
        .boxed_local()
      }
    })
  }
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `AnyError` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, AnyError>>;

  fn poll_close(&mut self, cx: &mut Context) -> Poll<Result<(), AnyError>>;

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), AnyError>>;
}

impl DenoAsyncWrite for StreamResource {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, AnyError>> {
    use StreamResource::*;
    let f: &mut dyn UnpinAsyncWrite = match self {
      FsFile(Some((f, _))) => f,
      FsFile(None) => return Poll::Pending,
      TcpStream(Some(f)) => f,
      #[cfg(not(windows))]
      UnixStream(f) => f,
      ClientTlsStream(f) => f,
      ServerTlsStream(f) => f,
      ChildStdin(f) => f,
      _ => return Err(bad_resource_id()).into(),
    };

    let v = ready!(Pin::new(f).poll_write(cx, buf))?;
    Ok(v).into()
  }

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), AnyError>> {
    use StreamResource::*;
    let f: &mut dyn UnpinAsyncWrite = match self {
      FsFile(Some((f, _))) => f,
      FsFile(None) => return Poll::Pending,
      TcpStream(Some(f)) => f,
      #[cfg(not(windows))]
      UnixStream(f) => f,
      ClientTlsStream(f) => f,
      ServerTlsStream(f) => f,
      ChildStdin(f) => f,
      _ => return Err(bad_resource_id()).into(),
    };

    ready!(Pin::new(f).poll_flush(cx))?;
    Ok(()).into()
  }

  fn poll_close(&mut self, _cx: &mut Context) -> Poll<Result<(), AnyError>> {
    unimplemented!()
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
      if rid >= 1_000_000 {
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
      } else {
        async move {
          let nwritten = poll_fn(|cx| {
            let mut state = state.borrow_mut();
            let resource_holder = state
              .resource_table
              .get_mut::<StreamResourceHolder>(rid as u32)
              .ok_or_else(bad_resource_id)?;
            resource_holder.resource.poll_write(cx, &zero_copy)
          })
          .await?;

          // TODO(bartlomieju): this step was added during upgrade to Tokio 0.2
          // and the reasons for the need to explicitly flush are not fully known.
          // Figure out why it's needed and preferably remove it.
          // https://github.com/denoland/deno/issues/3565
          poll_fn(|cx| {
            let mut state = state.borrow_mut();
            let resource_holder = state
              .resource_table
              .get_mut::<StreamResourceHolder>(rid as u32)
              .ok_or_else(bad_resource_id)?;
            resource_holder.resource.poll_flush(cx)
          })
          .await?;

          Ok(nwritten as i32)
        }
        .boxed_local()
      }
    })
  }
}

/// Helper function for operating on a std::fs::File stored in the resource table.
///
/// We store file system file resources as tokio::fs::File, so this is a little
/// utility function that gets a std::fs:File when you need to do blocking
/// operations.
///
/// Returns ErrorKind::Busy if the resource is being used by another op.
pub fn std_file_resource<F, T>(
  state: &mut OpState,
  rid: u32,
  mut f: F,
) -> Result<T, AnyError>
where
  F: FnMut(
    Result<&mut std::fs::File, &mut StreamResource>,
  ) -> Result<T, AnyError>,
{
  // First we look up the rid in the resource table.
  let mut r = state.resource_table.get_mut::<StreamResourceHolder>(rid);
  if let Some(ref mut resource_holder) = r {
    // Sync write only works for FsFile. It doesn't make sense to do this
    // for non-blocking sockets. So we error out if not FsFile.
    match &mut resource_holder.resource {
      StreamResource::FsFile(option_file_metadata) => {
        // The object in the resource table is a tokio::fs::File - but in
        // order to do a blocking write on it, we must turn it into a
        // std::fs::File. Hopefully this code compiles down to nothing.
        if let Some((tokio_file, metadata)) = option_file_metadata.take() {
          match tokio_file.try_into_std() {
            Ok(mut std_file) => {
              let result = f(Ok(&mut std_file));
              // Turn the std_file handle back into a tokio file, put it back
              // in the resource table.
              let tokio_file = tokio::fs::File::from_std(std_file);
              resource_holder.resource =
                StreamResource::FsFile(Some((tokio_file, metadata)));
              // return the result.
              result
            }
            Err(tokio_file) => {
              // This function will return an error containing the file if
              // some operation is in-flight.
              resource_holder.resource =
                StreamResource::FsFile(Some((tokio_file, metadata)));
              Err(resource_unavailable())
            }
          }
        } else {
          Err(resource_unavailable())
        }
      }
      _ => f(Err(&mut resource_holder.resource)),
    }
  } else {
    Err(bad_resource_id())
  }
}
