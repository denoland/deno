use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::deno_error::bad_resource;
use crate::http_util::HttpBody;
use crate::ops::minimal_op;
use crate::state::ThreadSafeState;
use deno::ErrBox;
use deno::Resource;
use deno::*;
use futures;
use futures::future::FutureExt;
use std;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;
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
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>>;
}

impl DenoAsyncRead for StreamResource {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let inner = self.get_mut();
    let mut f: Box<dyn AsyncRead + Unpin> = match inner {
      StreamResource::FsFile(f) => Box::new(f),
      StreamResource::Stdin(f) => Box::new(f),
      StreamResource::TcpStream(f) => Box::new(f),
      StreamResource::ClientTlsStream(f) => Box::new(f),
      StreamResource::ServerTlsStream(f) => Box::new(f),
      StreamResource::ChildStdout(f) => Box::new(f),
      StreamResource::ChildStderr(f) => Box::new(f),
      StreamResource::HttpBody(f) => Box::new(f),
      _ => {
        return Poll::Ready(Err(bad_resource()));
      }
    };

    let r = AsyncRead::poll_read(Pin::new(&mut f), cx, buf);

    match r {
      Poll::Ready(Err(e)) => Poll::Ready(Err(ErrBox::from(e))),
      Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
      Poll::Pending => Poll::Pending,
    }
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
    let nread = match DenoAsyncRead::poll_read(
      Pin::new(resource),
      cx,
      &mut inner.buf.as_mut()[..],
    ) {
      Poll::Ready(Ok(v)) => v,
      Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
      Poll::Pending => return Poll::Pending,
    };
    inner.io_state = IoState::Done;
    Poll::Ready(Ok(nread as i32))
  }
}

pub fn op_read(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<PinnedBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified()).boxed()
    }
    Some(buf) => buf,
  };

  let fut = read(state, rid as u32, zero_copy);

  fut.boxed()
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>>;

  fn poll_close(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>>;

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>>;
}

impl DenoAsyncWrite for StreamResource {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let inner = self.get_mut();
    let mut f: Box<dyn AsyncWrite + Unpin> = match inner {
      StreamResource::FsFile(f) => Box::new(f),
      StreamResource::Stdout(f) => Box::new(f),
      StreamResource::Stderr(f) => Box::new(f),
      StreamResource::TcpStream(f) => Box::new(f),
      StreamResource::ClientTlsStream(f) => Box::new(f),
      StreamResource::ServerTlsStream(f) => Box::new(f),
      StreamResource::ChildStdin(f) => Box::new(f),
      _ => {
        return Poll::Ready(Err(bad_resource()));
      }
    };

    let r = AsyncWrite::poll_write(Pin::new(&mut f), cx, buf);

    match r {
      Poll::Ready(Err(e)) => Poll::Ready(Err(ErrBox::from(e))),
      Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>> {
    let inner = self.get_mut();
    let mut f: Box<dyn AsyncWrite + Unpin> = match inner {
      StreamResource::FsFile(f) => Box::new(f),
      StreamResource::Stdout(f) => Box::new(f),
      StreamResource::Stderr(f) => Box::new(f),
      StreamResource::TcpStream(f) => Box::new(f),
      StreamResource::ClientTlsStream(f) => Box::new(f),
      StreamResource::ServerTlsStream(f) => Box::new(f),
      StreamResource::ChildStdin(f) => Box::new(f),
      _ => {
        return Poll::Ready(Err(bad_resource()));
      }
    };

    let r = AsyncWrite::poll_flush(Pin::new(&mut f), cx);

    match r {
      Poll::Ready(Err(e)) => Poll::Ready(Err(ErrBox::from(e))),
      Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_close(
    self: Pin<&mut Self>,
    _cx: &mut Context,
  ) -> Poll<Result<(), ErrBox>> {
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

      let nwritten = match DenoAsyncWrite::poll_write(
        Pin::new(resource),
        cx,
        inner.buf.as_ref(),
      ) {
        Poll::Ready(Ok(v)) => v,
        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
        Poll::Pending => return Poll::Pending,
      };
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
      match DenoAsyncWrite::poll_flush(Pin::new(resource), cx) {
        Poll::Ready(Ok(_)) => {
          inner.io_state = IoState::Done;
        }
        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
        Poll::Pending => return Poll::Pending,
      };
    }

    Poll::Ready(Ok(inner.nwritten))
  }
}

pub fn op_write(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<PinnedBuf>,
) -> Pin<Box<MinimalOp>> {
  debug!("write rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return futures::future::err(deno_error::no_buffer_specified()).boxed()
    }
    Some(buf) => buf,
  };

  let fut = write(state, rid as u32, zero_copy);

  fut.boxed()
}
