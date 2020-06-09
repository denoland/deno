use super::dispatch_minimal::MinimalOp;
use crate::http_util::HttpBody;
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::ready;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
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
  /// Due to portability issues on Windows handle to stdout is created from raw
  /// file descriptor.  The caveat of that approach is fact that when this
  /// handle is dropped underlying file descriptor is closed - that is highly
  /// not desirable in case of stdout.  That's why we store this global handle
  /// that is then cloned when obtaining stdio for process. In turn when
  /// resource table is dropped storing reference to that handle, the handle
  /// itself won't be closed (so Deno.core.print) will still work.
  // TODO(ry) It should be possible to close stdout.
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
  static ref STDERR_HANDLE: std::fs::File = {
    #[cfg(not(windows))]
    let stderr = unsafe { std::fs::File::from_raw_fd(2) };
    #[cfg(windows)]
    let stderr = unsafe {
      std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
        winapi::um::winbase::STD_ERROR_HANDLE,
      ))
    };
    stderr
  };
}

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_read", s.stateful_minimal_op2(op_read));
  i.register_op("op_write", s.stateful_minimal_op2(op_write));
}

pub fn get_stdio() -> (
  StreamResourceHolder,
  StreamResourceHolder,
  StreamResourceHolder,
) {
  let stdin = StreamResourceHolder::new(StreamResource::Stdin(
    tokio::io::stdin(),
    TTYMetadata::default(),
  ));
  let stdout = StreamResourceHolder::new(StreamResource::FsFile(Some({
    let stdout = STDOUT_HANDLE.try_clone().unwrap();
    (tokio::fs::File::from_std(stdout), FileMetadata::default())
  })));
  let stderr = StreamResourceHolder::new(StreamResource::FsFile(Some({
    let stderr = STDERR_HANDLE.try_clone().unwrap();
    (tokio::fs::File::from_std(stderr), FileMetadata::default())
  })));

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
  pub fn track_task(&mut self, cx: &Context) -> Result<usize, OpError> {
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

pub enum StreamResource {
  Stdin(tokio::io::Stdin, TTYMetadata),
  FsFile(Option<(tokio::fs::File, FileMetadata)>),
  TcpStream(Option<tokio::net::TcpStream>),
  #[cfg(not(windows))]
  UnixStream(tokio::net::UnixStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  HttpBody(Box<HttpBody>),
  ChildStdin(tokio::process::ChildStdin),
  ChildStdout(tokio::process::ChildStdout),
  ChildStderr(tokio::process::ChildStderr),
}

trait UnpinAsyncRead: AsyncRead + Unpin {}
trait UnpinAsyncWrite: AsyncWrite + Unpin {}

impl<T: AsyncRead + Unpin> UnpinAsyncRead for T {}
impl<T: AsyncWrite + Unpin> UnpinAsyncWrite for T {}

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
    let f: &mut dyn UnpinAsyncRead = match self {
      FsFile(Some((f, _))) => f,
      FsFile(None) => return Poll::Ready(Err(OpError::resource_unavailable())),
      Stdin(f, _) => f,
      TcpStream(Some(f)) => f,
      #[cfg(not(windows))]
      UnixStream(f) => f,
      ClientTlsStream(f) => f,
      ServerTlsStream(f) => f,
      ChildStdout(f) => f,
      ChildStderr(f) => f,
      HttpBody(f) => f,
      _ => return Err(OpError::bad_resource_id()).into(),
    };
    let v = ready!(Pin::new(f).poll_read(cx, buf))?;
    Ok(v).into()
  }
}

pub fn op_read(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  is_sync: bool,
  rid: i32,
  zero_copy: &mut [ZeroCopyBuf],
) -> MinimalOp {
  debug!("read rid={}", rid);
  match zero_copy.len() {
    0 => return MinimalOp::Sync(Err(no_buffer_specified())),
    1 => {}
    _ => panic!("Invalid number of arguments"),
  }
  let resource_table = isolate_state.resource_table.clone();

  if is_sync {
    MinimalOp::Sync({
      // First we look up the rid in the resource table.
      let mut resource_table = resource_table.borrow_mut();
      std_file_resource(&mut resource_table, rid as u32, move |r| match r {
        Ok(std_file) => {
          use std::io::Read;
          std_file
            .read(&mut zero_copy[0])
            .map(|n: usize| n as i32)
            .map_err(OpError::from)
        }
        Err(_) => Err(OpError::type_error(
          "sync read not allowed on this resource".to_string(),
        )),
      })
    })
  } else {
    let mut zero_copy = zero_copy[0].clone();
    MinimalOp::Async(
      poll_fn(move |cx| {
        let mut resource_table = resource_table.borrow_mut();
        let resource_holder = resource_table
          .get_mut::<StreamResourceHolder>(rid as u32)
          .ok_or_else(OpError::bad_resource_id)?;

        let mut task_tracker_id: Option<usize> = None;
        let nread = match resource_holder
          .resource
          .poll_read(cx, &mut zero_copy)
          .map_err(OpError::from)
        {
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
      .boxed_local(),
    )
  }
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
    let f: &mut dyn UnpinAsyncWrite = match self {
      FsFile(Some((f, _))) => f,
      FsFile(None) => return Poll::Pending,
      TcpStream(Some(f)) => f,
      #[cfg(not(windows))]
      UnixStream(f) => f,
      ClientTlsStream(f) => f,
      ServerTlsStream(f) => f,
      ChildStdin(f) => f,
      _ => return Err(OpError::bad_resource_id()).into(),
    };

    let v = ready!(Pin::new(f).poll_write(cx, buf))?;
    Ok(v).into()
  }

  fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), OpError>> {
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
      _ => return Err(OpError::bad_resource_id()).into(),
    };

    ready!(Pin::new(f).poll_flush(cx))?;
    Ok(()).into()
  }

  fn poll_close(&mut self, _cx: &mut Context) -> Poll<Result<(), OpError>> {
    unimplemented!()
  }
}

pub fn op_write(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  is_sync: bool,
  rid: i32,
  zero_copy: &mut [ZeroCopyBuf],
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
      let mut resource_table = isolate_state.resource_table.borrow_mut();
      std_file_resource(&mut resource_table, rid as u32, move |r| match r {
        Ok(std_file) => {
          use std::io::Write;
          std_file
            .write(&zero_copy[0])
            .map(|nwritten: usize| nwritten as i32)
            .map_err(OpError::from)
        }
        Err(_) => Err(OpError::type_error(
          "sync read not allowed on this resource".to_string(),
        )),
      })
    })
  } else {
    let zero_copy = zero_copy[0].clone();
    let resource_table = isolate_state.resource_table.clone();
    MinimalOp::Async(
      async move {
        let nwritten = poll_fn(|cx| {
          let mut resource_table = resource_table.borrow_mut();
          let resource_holder = resource_table
            .get_mut::<StreamResourceHolder>(rid as u32)
            .ok_or_else(OpError::bad_resource_id)?;
          resource_holder.resource.poll_write(cx, &zero_copy)
        })
        .await?;

        // TODO(bartlomieju): this step was added during upgrade to Tokio 0.2
        // and the reasons for the need to explicitly flush are not fully known.
        // Figure out why it's needed and preferably remove it.
        // https://github.com/denoland/deno/issues/3565
        poll_fn(|cx| {
          let mut resource_table = resource_table.borrow_mut();
          let resource_holder = resource_table
            .get_mut::<StreamResourceHolder>(rid as u32)
            .ok_or_else(OpError::bad_resource_id)?;
          resource_holder.resource.poll_flush(cx)
        })
        .await?;

        Ok(nwritten as i32)
      }
      .boxed_local(),
    )
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
  resource_table: &mut ResourceTable,
  rid: u32,
  mut f: F,
) -> Result<T, OpError>
where
  F: FnMut(
    Result<&mut std::fs::File, &mut StreamResource>,
  ) -> Result<T, OpError>,
{
  // First we look up the rid in the resource table.
  let mut r = resource_table.get_mut::<StreamResourceHolder>(rid);
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
              Err(OpError::resource_unavailable())
            }
          }
        } else {
          Err(OpError::resource_unavailable())
        }
      }
      _ => f(Err(&mut resource_holder.resource)),
    }
  } else {
    Err(OpError::bad_resource_id())
  }
}
