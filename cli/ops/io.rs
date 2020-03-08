use super::dispatch_minimal::MinimalOp;
use crate::http_util::HttpBody;
use crate::op_error::OpError;
use crate::ops::minimal_op;
use crate::state::State;
use deno_core::*;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::ready;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;

#[cfg(not(windows))]
use std::os::unix::io::FromRawFd;

#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

#[cfg(windows)]
extern crate winapi;

lazy_static! {
  /// Due to portability issues on Windows handle to stdout is created from raw file descriptor.
  /// The caveat of that approach is fact that when this handle is dropped underlying
  /// file descriptor is closed - that is highly not desirable in case of stdout.
  /// That's why we store this global handle that is then cloned when obtaining stdio
  /// for process. In turn when resource table is dropped storing reference to that handle,
  /// the handle itself won't be closed (so Deno.core.print) will still work.
  static ref STDOUT_HANDLE: std::fs::File = {
    #[cfg(not(windows))]
    let stdout = unsafe { std::fs::File::from_raw_fd(1) };
    #[cfg(windows)]
    let stdout = unsafe {
      std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
        winapi::um::winbase::STD_OUTPUT_HANDLE,
      ))
    };

    stdout
  };
}

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "op_read",
    s.core_op(minimal_op(s.stateful_minimal_op(op_read))),
  );
  i.register_op(
    "op_write",
    s.core_op(minimal_op(s.stateful_minimal_op(op_write))),
  );
}

pub fn get_stdio() -> (StreamResource, StreamResource, StreamResource) {
  let stdin = StreamResource::Stdin(tokio::io::stdin(), TTYMetadata::default());
  let stdout = StreamResource::Stdout({
    let stdout = STDOUT_HANDLE
      .try_clone()
      .expect("Unable to clone stdout handle");
    tokio::fs::File::from_std(stdout)
  });
  let stderr = StreamResource::Stderr(tokio::io::stderr());

  (stdin, stdout, stderr)
}

fn no_buffer_specified() -> OpError {
  OpError::type_error("no buffer specified".to_string())
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
  resource: StreamResource,
  waker: Option<futures::task::AtomicWaker>,
}

impl StreamResourceHolder {
  pub fn new(resource: StreamResource) -> StreamResourceHolder {
    StreamResourceHolder {
      resource,
      waker: None,
    }
  }
}

impl Drop for StreamResourceHolder {
  fn drop(&mut self) {
    self.wake_task();
  }
}

impl StreamResourceHolder {
  /// Track the current task so future awaiting for connection
  /// can be notified when listener is closed.
  ///
  /// Throws an error if another task is already tracked.
  pub fn track_task(&mut self, cx: &Context) -> Result<(), OpError> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      return Err(OpError::other("Another accept task is ongoing".to_string()));
    }
    let waker = futures::task::AtomicWaker::new();
    waker.register(cx.waker());
    self.waker.replace(waker);
    Ok(())
  }

  /// Notifies a task when listener is closed so accept future can resolve.
  pub fn wake_task(&mut self) {
    if let Some(waker) = self.waker.as_ref() {
      waker.wake();
    }
  }

  /// Stop tracking a task.
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&mut self) {
    if self.waker.is_some() {
      self.waker.take();
    }
  }
}

pub enum StreamResource {
  Stdin(tokio::io::Stdin, TTYMetadata),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File, FileMetadata),
  TcpStream(tokio::net::TcpStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  HttpBody(Box<HttpBody>),
  ChildStdin(tokio::process::ChildStdin),
  ChildStdout(tokio::process::ChildStdout),
  ChildStderr(tokio::process::ChildStderr),
}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `OpError` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, OpError>>;
}

impl DenoAsyncRead for StreamResource {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, OpError>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncRead>> = match self {
      FsFile(f, _) => Box::pin(f),
      Stdin(f, _) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdout(f) => Box::pin(f),
      ChildStderr(f) => Box::pin(f),
      HttpBody(f) => Box::pin(f),
      _ => return Err(OpError::bad_resource_id()).into(),
    };

    let v = ready!(f.as_mut().poll_read(cx, buf))?;
    Ok(v).into()
  }
}

pub fn op_read(
  state: &State,
  rid: i32,
  zero_copy: Option<ZeroCopyBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("read rid={}", rid);
  if zero_copy.is_none() {
    return futures::future::err(no_buffer_specified()).boxed_local();
  }

  let state = state.clone();
  let mut buf = zero_copy.unwrap();

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let resource_holder = resource_table
      .get_mut::<StreamResourceHolder>(rid as u32)
      .ok_or_else(OpError::bad_resource_id)?;
    let nread = match resource_holder
      .resource
      .poll_read(cx, &mut buf.as_mut()[..])
    {
      Poll::Ready(t) => {
        resource_holder.untrack_task();
        t
      }
      Poll::Pending => {
        resource_holder.track_task(cx)?;
        return Poll::Pending;
      }
    }?;
    Poll::Ready(Ok(nread as i32))
  })
  .boxed_local()
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `OpError` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, OpError>>;

  fn poll_close(&mut self, cx: &mut Context) -> Poll<Result<(), OpError>>;

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), OpError>>;
}

impl DenoAsyncWrite for StreamResource {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, OpError>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncWrite>> = match self {
      FsFile(f, _) => Box::pin(f),
      Stdout(f) => Box::pin(f),
      Stderr(f) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdin(f) => Box::pin(f),
      _ => return Err(OpError::bad_resource_id()).into(),
    };

    let v = ready!(f.as_mut().poll_write(cx, buf))?;
    Ok(v).into()
  }

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), OpError>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncWrite>> = match self {
      FsFile(f, _) => Box::pin(f),
      Stdout(f) => Box::pin(f),
      Stderr(f) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdin(f) => Box::pin(f),
      _ => return Err(OpError::bad_resource_id()).into(),
    };

    ready!(f.as_mut().poll_flush(cx))?;
    Ok(()).into()
  }

  fn poll_close(&mut self, _cx: &mut Context) -> Poll<Result<(), OpError>> {
    unimplemented!()
  }
}

pub fn op_write(
  state: &State,
  rid: i32,
  zero_copy: Option<ZeroCopyBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("write rid={}", rid);
  if zero_copy.is_none() {
    return futures::future::err(no_buffer_specified()).boxed_local();
  }

  let state = state.clone();
  let buf = zero_copy.unwrap();

  async move {
    let nwritten = poll_fn(|cx| {
      let resource_table = &mut state.borrow_mut().resource_table;
      let resource_holder = resource_table
        .get_mut::<StreamResourceHolder>(rid as u32)
        .ok_or_else(OpError::bad_resource_id)?;
      resource_holder.resource.poll_write(cx, &buf.as_ref()[..])
    })
    .await?;

    // TODO(bartlomieju): this step was added during upgrade to Tokio 0.2
    // and the reasons for the need to explicitly flush are not fully known.
    // Figure out why it's needed and preferably remove it.
    // https://github.com/denoland/deno/issues/3565
    poll_fn(|cx| {
      let resource_table = &mut state.borrow_mut().resource_table;
      let resource_holder = resource_table
        .get_mut::<StreamResourceHolder>(rid as u32)
        .ok_or_else(OpError::bad_resource_id)?;
      resource_holder.resource.poll_flush(cx)
    })
    .await?;

    Ok(nwritten as i32)
  }
  .boxed_local()
}
