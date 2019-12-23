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

impl StreamResource {
  fn poll_read(
    &mut self,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let mut f: Box<dyn AsyncRead + Unpin> = match self {
      StreamResource::FsFile(f) => Box::new(f),
      StreamResource::Stdin(f) => Box::new(f),
      StreamResource::TcpStream(f) => Box::new(f),
      StreamResource::ClientTlsStream(f) => Box::new(f),
      StreamResource::ServerTlsStream(f) => Box::new(f),
      StreamResource::HttpBody(f) => Box::new(f),
      StreamResource::ChildStdout(f) => Box::new(f),
      StreamResource::ChildStderr(f) => Box::new(f),
      _ => {
        return Err(bad_resource()).into();
      }
    };

    let n = ready!(Pin::new(&mut f).poll_read(cx, buf))?;
    Ok(n).into()
  }

  fn poll_write(
    &mut self,
    cx: &mut Context,
    buf: &[u8],
  ) -> Poll<Result<usize, ErrBox>> {
    let mut f: Box<dyn AsyncWrite + Unpin> = match self {
      StreamResource::FsFile(f) => Box::new(f),
      StreamResource::Stdout(f) => Box::new(f),
      StreamResource::Stderr(f) => Box::new(f),
      StreamResource::TcpStream(f) => Box::new(f),
      StreamResource::ClientTlsStream(f) => Box::new(f),
      StreamResource::ServerTlsStream(f) => Box::new(f),
      StreamResource::ChildStdin(f) => Box::new(f),
      _ => {
        return Err(bad_resource()).into();
      }
    };

    let n = ready!(Pin::new(&mut f).poll_write(cx, buf))?;
    Ok(n).into()
  }
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
pub async fn read<T>(
  state: ThreadSafeState,
  rid: ResourceId,
  mut buf: T,
) -> Result<i32, ErrBox>
where
  T: AsMut<[u8]>,
{
  poll_fn(move |cx| {
    let mut table = state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(rid)
      .ok_or_else(bad_resource)?;
    let nread = ready!(resource.poll_read(cx, &mut buf.as_mut()[..]))?;
    Ok(nread as i32).into()
  })
  .await
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

  let fut = read(state.clone(), rid as u32, zero_copy);
  fut.boxed()
}

/// Creates a future that will write some of the buffer `buf` to
/// the stream resource with `rid`.
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
pub async fn write<T>(
  state: ThreadSafeState,
  rid: ResourceId,
  buf: T,
) -> Result<i32, ErrBox>
where
  T: AsRef<[u8]>,
{
  poll_fn(move |cx| {
    let mut table = state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(rid)
      .ok_or_else(bad_resource)?;
    let nwritten = ready!(resource.poll_write(cx, buf.as_ref()))?;
    Ok(nwritten as i32).into()
  })
  .await
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

  let fut = write(state.clone(), rid as u32, zero_copy);
  fut.boxed()
}
