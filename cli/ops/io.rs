use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::deno_error::bad_resource;
use crate::http_util::HttpBody;
use crate::ops::minimal_op;
use crate::state::ThreadSafeState;
use deno_core::ErrBox;
use deno_core::Resource;
use deno_core::*;
use futures::future::FutureExt;
use futures::ready;
use std::future::Future;
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

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "read",
    s.core_op(minimal_op(s.stateful_minimal_op(op_read))),
  );
  i.register_op(
    "write",
    s.core_op(minimal_op(s.stateful_minimal_op(op_write))),
  );
}

pub fn get_stdio() -> (StreamResource, StreamResource, StreamResource) {
  let stdin = StreamResource::Stdin(tokio::io::stdin());
  let stdout = StreamResource::Stdout({
    let stdout = STDOUT_HANDLE
      .try_clone()
      .expect("Unable to clone stdout handle");
    tokio::fs::File::from_std(stdout)
  });
  let stderr = StreamResource::Stderr(tokio::io::stderr());

  (stdin, stdout, stderr)
}

pub enum StreamResource {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
  TcpStream(tokio::net::TcpStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  HttpBody(Box<HttpBody>),
  ChildStdin(tokio::process::ChildStdin),
  ChildStdout(tokio::process::ChildStdout),
  ChildStderr(tokio::process::ChildStderr),
}

impl Resource for StreamResource {}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>>;
}

impl DenoAsyncRead for StreamResource {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncRead>> = match self {
      FsFile(f) => Box::pin(f),
      Stdin(f) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdout(f) => Box::pin(f),
      ChildStderr(f) => Box::pin(f),
      HttpBody(f) => Box::pin(f),
      _ => return Err(bad_resource()).into(),
    };

    let v = ready!(f.as_mut().poll_read(cx, buf))?;
    Ok(v).into()
  }
}

#[derive(Debug, PartialEq)]
enum IoState {
  Pending,
  Flush,
  Done,
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
pub fn read<T>(state: &ThreadSafeState, rid: ResourceId, buf: T) -> Read<T>
where
  T: AsMut<[u8]>,
{
  Read {
    rid,
    buf,
    io_state: IoState::Pending,
    state: state.clone(),
  }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
pub struct Read<T> {
  rid: ResourceId,
  buf: T,
  io_state: IoState,
  state: ThreadSafeState,
}

impl<T> Future for Read<T>
where
  T: AsMut<[u8]> + Unpin,
{
  type Output = Result<i32, ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.io_state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    let mut table = inner.state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(inner.rid)
      .ok_or_else(bad_resource)?;
    let nread = ready!(resource.poll_read(cx, &mut inner.buf.as_mut()[..]))?;
    inner.io_state = IoState::Done;
    Poll::Ready(Ok(nread as i32))
  }
}

pub fn op_read(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<ZeroCopyBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified())
        .boxed_local()
    }
    Some(buf) => buf,
  };

  let fut = read(state, rid as u32, zero_copy);
  fut.boxed_local()
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>>;

  fn poll_close(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>>;

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>>;
}

impl DenoAsyncWrite for StreamResource {
  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncWrite>> = match self {
      FsFile(f) => Box::pin(f),
      Stdout(f) => Box::pin(f),
      Stderr(f) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdin(f) => Box::pin(f),
      _ => return Err(bad_resource()).into(),
    };

    let v = ready!(f.as_mut().poll_write(cx, buf))?;
    Ok(v).into()
  }

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    use StreamResource::*;
    let mut f: Pin<Box<dyn AsyncWrite>> = match self {
      FsFile(f) => Box::pin(f),
      Stdout(f) => Box::pin(f),
      Stderr(f) => Box::pin(f),
      TcpStream(f) => Box::pin(f),
      ClientTlsStream(f) => Box::pin(f),
      ServerTlsStream(f) => Box::pin(f),
      ChildStdin(f) => Box::pin(f),
      _ => return Err(bad_resource()).into(),
    };

    ready!(f.as_mut().poll_flush(cx))?;
    Ok(()).into()
  }

  fn poll_close(&mut self, _cx: &mut Context) -> Poll<Result<(), ErrBox>> {
    unimplemented!()
  }
}

/// A future used to write some data to a stream.
pub struct Write<T> {
  rid: ResourceId,
  buf: T,
  io_state: IoState,
  state: ThreadSafeState,
  nwritten: i32,
}

/// Creates a future that will write some of the buffer `buf` to
/// the stream resource with `rid`.
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
pub fn write<T>(state: &ThreadSafeState, rid: ResourceId, buf: T) -> Write<T>
where
  T: AsRef<[u8]>,
{
  Write {
    rid,
    buf,
    io_state: IoState::Pending,
    state: state.clone(),
    nwritten: 0,
  }
}

/// This is almost the same implementation as in tokio, difference is
/// that error type is `ErrBox` instead of `std::io::Error`.
impl<T> Future for Write<T>
where
  T: AsRef<[u8]> + Unpin,
{
  type Output = Result<i32, ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.io_state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    if inner.io_state == IoState::Pending {
      let mut table = inner.state.lock_resource_table();
      let resource = table
        .get_mut::<StreamResource>(inner.rid)
        .ok_or_else(bad_resource)?;

      let nwritten = ready!(resource.poll_write(cx, inner.buf.as_ref()))?;
      inner.io_state = IoState::Flush;
      inner.nwritten = nwritten as i32;
    }

    // TODO(bartlomieju): this step was added during upgrade to Tokio 0.2
    // and the reasons for the need to explicitly flush are not fully known.
    // Figure out why it's needed and preferably remove it.
    // https://github.com/denoland/deno/issues/3565
    if inner.io_state == IoState::Flush {
      let mut table = inner.state.lock_resource_table();
      let resource = table
        .get_mut::<StreamResource>(inner.rid)
        .ok_or_else(bad_resource)?;
      ready!(resource.poll_flush(cx))?;
      inner.io_state = IoState::Done;
    }

    Poll::Ready(Ok(inner.nwritten))
  }
}

pub fn op_write(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<ZeroCopyBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("write rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified())
        .boxed_local()
    }
    Some(buf) => buf,
  };

  let fut = write(state, rid as u32, zero_copy);

  fut.boxed_local()
}
