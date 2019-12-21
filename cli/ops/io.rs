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
use futures::future::FutureExt;
use std;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::process;
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
  ChildStdin(process::ChildStdin),
  ChildStdout(process::ChildStdout),
  ChildStderr(process::ChildStderr),
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

impl StreamResource {
  pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrBox> {
    let n = match self {
      StreamResource::FsFile(f) => f.read(buf).await?,
      StreamResource::Stdin(f) => f.read(buf).await?,
      StreamResource::TcpStream(f) => f.read(buf).await?,
      StreamResource::ClientTlsStream(f) => f.read(buf).await?,
      StreamResource::ServerTlsStream(f) => f.read(buf).await?,
      StreamResource::HttpBody(f) => f.read(buf).await?,
      StreamResource::ChildStdout(f) => f.read(buf).await?,
      StreamResource::ChildStderr(f) => f.read(buf).await?,
      _ => return Err(bad_resource()),
    };

    Ok(n)
  }

  pub async fn write(&mut self, buf: &[u8]) -> Result<usize, ErrBox> {
    let n = match self {
      StreamResource::FsFile(f) => f.write(buf).await?,
      StreamResource::Stdout(f) => f.write(buf).await?,
      StreamResource::Stderr(f) => f.write(buf).await?,
      StreamResource::TcpStream(f) => f.write(buf).await?,
      StreamResource::ClientTlsStream(f) => f.write(buf).await?,
      StreamResource::ServerTlsStream(f) => f.write(buf).await?,
      StreamResource::ChildStdin(f) => f.write(buf).await?,
      _ => return Err(bad_resource()),
    };

    Ok(n)
  }
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
pub fn read<T>(
  state: &ThreadSafeState,
  rid: ResourceId,
  mut buf: T,
) -> impl Future<Output = Result<i32, ErrBox>>
where
  T: AsMut<[u8]>,
{
  let state = state.clone();
  async move {
    let mut table = state.lock_resource_table_async().await;
    let resource = table
      .get_mut::<StreamResource>(rid)
      .ok_or_else(bad_resource)?;
    let nread = resource.read(&mut buf.as_mut()[..]).await?;
    Ok(nread as i32)
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

/// Creates a future that will write some of the buffer `buf` to
/// the stream resource with `rid`.
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
pub fn write<T>(
  state: &ThreadSafeState,
  rid: ResourceId,
  buf: T,
) -> impl Future<Output = Result<i32, ErrBox>>
where
  T: AsRef<[u8]>,
{
  let state = state.clone();
  async move {
    let mut table = state.lock_resource_table_async().await;
    let resource = table
      .get_mut::<StreamResource>(rid)
      .ok_or_else(bad_resource);
    let buf = buf.as_ref();
    let nwritten = resource?.write(buf).await?;
    Ok(nwritten as i32)
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
