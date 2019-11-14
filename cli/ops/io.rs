use super::dispatch_minimal::MinimalOp;
use crate::deno_error;
use crate::deno_error::bad_resource;
use crate::http_body::HttpBody;
use crate::ops::minimal_op;
use crate::state::ThreadSafeState;
use deno::ErrBox;
use deno::Resource;
use deno::*;
use futures;
use futures::Future;
use futures::Poll;
use std;
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_process;
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
  HttpBody(HttpBody),
  ChildStdin(tokio_process::ChildStdin),
  ChildStdout(tokio_process::ChildStdout),
  ChildStderr(tokio_process::ChildStderr),
}

impl Resource for StreamResource {}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox>;
}

impl DenoAsyncRead for StreamResource {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox> {
    let r = match self {
      StreamResource::FsFile(ref mut f) => f.poll_read(buf),
      StreamResource::Stdin(ref mut f) => f.poll_read(buf),
      StreamResource::TcpStream(ref mut f) => f.poll_read(buf),
      StreamResource::ClientTlsStream(ref mut f) => f.poll_read(buf),
      StreamResource::ServerTlsStream(ref mut f) => f.poll_read(buf),
      StreamResource::HttpBody(ref mut f) => f.poll_read(buf),
      StreamResource::ChildStdout(ref mut f) => f.poll_read(buf),
      StreamResource::ChildStderr(ref mut f) => f.poll_read(buf),
      _ => {
        return Err(bad_resource());
      }
    };

    r.map_err(ErrBox::from)
  }
}

#[derive(Debug, PartialEq)]
enum IoState {
  Pending,
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
  T: AsMut<[u8]>,
{
  type Item = usize;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    if self.io_state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    let mut table = self.state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(self.rid)
      .ok_or_else(bad_resource)?;
    let nread = try_ready!(resource.poll_read(&mut self.buf.as_mut()[..]));
    self.io_state = IoState::Done;
    Ok(nread.into())
  }
}

pub fn op_read(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<PinnedBuf>,
) -> Box<MinimalOp> {
  debug!("read rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return Box::new(futures::future::err(deno_error::no_buffer_specified()));
    }
    Some(buf) => buf,
  };

  let fut = read(state, rid as u32, zero_copy)
    .map_err(ErrBox::from)
    .and_then(move |nread| Ok(nread as i32));

  Box::new(fut)
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox>;

  fn shutdown(&mut self) -> Poll<(), ErrBox>;
}

impl DenoAsyncWrite for StreamResource {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox> {
    let r = match self {
      StreamResource::FsFile(ref mut f) => f.poll_write(buf),
      StreamResource::Stdout(ref mut f) => f.poll_write(buf),
      StreamResource::Stderr(ref mut f) => f.poll_write(buf),
      StreamResource::TcpStream(ref mut f) => f.poll_write(buf),
      StreamResource::ClientTlsStream(ref mut f) => f.poll_write(buf),
      StreamResource::ServerTlsStream(ref mut f) => f.poll_write(buf),
      StreamResource::ChildStdin(ref mut f) => f.poll_write(buf),
      _ => {
        return Err(bad_resource());
      }
    };

    r.map_err(ErrBox::from)
  }

  fn shutdown(&mut self) -> futures::Poll<(), ErrBox> {
    unimplemented!()
  }
}

/// A future used to write some data to a stream.
pub struct Write<T> {
  rid: ResourceId,
  buf: T,
  io_state: IoState,
  state: ThreadSafeState,
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
  }
}

/// This is almost the same implementation as in tokio, difference is
/// that error type is `ErrBox` instead of `std::io::Error`.
impl<T> Future for Write<T>
where
  T: AsRef<[u8]>,
{
  type Item = usize;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    if self.io_state == IoState::Done {
      panic!("poll a Read after it's done");
    }

    let mut table = self.state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(self.rid)
      .ok_or_else(bad_resource)?;
    let nwritten = try_ready!(resource.poll_write(self.buf.as_ref()));
    self.io_state = IoState::Done;
    Ok(nwritten.into())
  }
}

pub fn op_write(
  state: &ThreadSafeState,
  rid: i32,
  zero_copy: Option<PinnedBuf>,
) -> Box<MinimalOp> {
  debug!("write rid={}", rid);
  let zero_copy = match zero_copy {
    None => {
      return Box::new(futures::future::err(deno_error::no_buffer_specified()));
    }
    Some(buf) => buf,
  };

  let fut = write(state, rid as u32, zero_copy)
    .map_err(ErrBox::from)
    .and_then(move |nwritten| Ok(nwritten as i32));

  Box::new(fut)
}
