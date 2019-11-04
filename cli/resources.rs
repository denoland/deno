// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::deno_error;
use crate::deno_error::bad_resource;
use crate::http_body::HttpBody;
use deno::ErrBox;
pub use deno::Resource as CoreResource;
pub use deno::ResourceId;
use deno::ResourceTable;

use futures;
use futures::Future;
use futures::Poll;
use reqwest::r#async::Decoder as ReqwestDecoder;
use std;
use std::io::{Error, Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, SocketAddr};
use std::process::ExitStatus;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_process;
use tokio_rustls::client::TlsStream as ClientTlsStream;
use tokio_rustls::server::TlsStream as ServerTlsStream;
use tokio_rustls::TlsAcceptor;

#[cfg(not(windows))]
use std::os::unix::io::FromRawFd;

use futures::future::Either;
#[cfg(windows)]
use std::os::windows::io::FromRawHandle;

#[cfg(windows)]
extern crate winapi;

lazy_static! {
  static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new({
    let mut table = ResourceTable::default();

    // TODO Load these lazily during lookup?
    table.add(Box::new(CliResource::Stdin(tokio::io::stdin())));

    table.add(Box::new(CliResource::Stdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    table.add(Box::new(CliResource::Stderr(tokio::io::stderr())));
    table
  });
}

// TODO: move listeners out of this enum and rename to `StreamResource`
enum CliResource {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
  // Since TcpListener might be closed while there is a pending accept task,
  // we need to track the task so that when the listener is closed,
  // this pending task could be notified and die.
  // Currently TcpListener itself does not take care of this issue.
  // See: https://github.com/tokio-rs/tokio/issues/846
  TcpListener(tokio::net::TcpListener, Option<futures::task::Task>),
  TlsListener(
    tokio::net::TcpListener,
    TlsAcceptor,
    Option<futures::task::Task>,
  ),
  TcpStream(tokio::net::TcpStream),
  ServerTlsStream(Box<ServerTlsStream<TcpStream>>),
  ClientTlsStream(Box<ClientTlsStream<TcpStream>>),
  HttpBody(HttpBody),
  // Enum size is bounded by the largest variant.
  // Use `Box` around large `Child` struct.
  // https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant
  Child(Box<tokio_process::Child>),
  ChildStdin(tokio_process::ChildStdin),
  ChildStdout(tokio_process::ChildStdout),
  ChildStderr(tokio_process::ChildStderr),
}

impl CoreResource for CliResource {
  fn close(&self) {
    match self {
      CliResource::TcpListener(_, Some(t)) => {
        t.notify();
      }
      CliResource::TlsListener(_, _, Some(t)) => {
        t.notify();
      }
      _ => {}
    }
  }

  fn inspect_repr(&self) -> &str {
    match self {
      CliResource::Stdin(_) => "stdin",
      CliResource::Stdout(_) => "stdout",
      CliResource::Stderr(_) => "stderr",
      CliResource::FsFile(_) => "fsFile",
      CliResource::TcpListener(_, _) => "tcpListener",
      CliResource::TlsListener(_, _, _) => "tlsListener",
      CliResource::TcpStream(_) => "tcpStream",
      CliResource::ClientTlsStream(_) => "clientTlsStream",
      CliResource::ServerTlsStream(_) => "serverTlsStream",
      CliResource::HttpBody(_) => "httpBody",
      CliResource::Child(_) => "child",
      CliResource::ChildStdin(_) => "childStdin",
      CliResource::ChildStdout(_) => "childStdout",
      CliResource::ChildStderr(_) => "childStderr",
    }
  }
}

pub fn lock_resource_table<'a>() -> MutexGuard<'a, ResourceTable> {
  RESOURCE_TABLE.lock().unwrap()
}

// Abstract async file interface.
// Ideally in unix, if Resource represents an OS rid, it will be the same.
#[derive(Clone, Debug)]
pub struct Resource {
  pub rid: ResourceId,
}

impl Resource {
  // TODO Should it return a Resource instead of net::TcpStream?
  pub fn poll_accept(&mut self) -> Poll<(TcpStream, SocketAddr), Error> {
    let mut table = lock_resource_table();
    match table.get_mut::<CliResource>(self.rid) {
      None => Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Listener has been closed",
      )),
      Some(repr) => match repr {
        CliResource::TcpListener(ref mut s, _) => s.poll_accept(),
        CliResource::TlsListener(ref mut s, _, _) => s.poll_accept(),
        _ => panic!("Cannot accept"),
      },
    }
  }

  pub fn poll_accept_tls(
    &mut self,
    tcp_stream: TcpStream,
  ) -> impl Future<Item = ServerTlsStream<TcpStream>, Error = Error> {
    let mut table = lock_resource_table();
    match table.get_mut::<CliResource>(self.rid) {
      None => Either::A(futures::future::err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Listener has been closed",
      ))),
      Some(repr) => match repr {
        CliResource::TlsListener(_, ref mut acceptor, _) => {
          Either::B(acceptor.accept(tcp_stream))
        }
        _ => panic!("Cannot accept"),
      },
    }
  }

  /// Track the current task (for TcpListener resource).
  /// Throws an error if another task is already tracked.
  pub fn track_task(&mut self) -> Result<(), std::io::Error> {
    let mut table = lock_resource_table();
    // Only track if is TcpListener.
    if let Some(CliResource::TcpListener(_, t)) =
      table.get_mut::<CliResource>(self.rid)
    {
      // Currently, we only allow tracking a single accept task for a listener.
      // This might be changed in the future with multiple workers.
      // Caveat: TcpListener by itself also only tracks an accept task at a time.
      // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
      if t.is_some() {
        return Err(std::io::Error::new(
          std::io::ErrorKind::Other,
          "Another accept task is ongoing",
        ));
      }
      t.replace(futures::task::current());
    }
    Ok(())
  }

  /// Stop tracking a task (for TcpListener resource).
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&mut self) {
    let mut table = lock_resource_table();
    // Only untrack if is TcpListener.
    if let Some(CliResource::TcpListener(_, t)) =
      table.get_mut::<CliResource>(self.rid)
    {
      if t.is_some() {
        t.take();
      }
    }
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the RESOURCE_TABLE.
  pub fn close(&self) {
    let mut table = lock_resource_table();
    table.close(self.rid).unwrap();
  }

  pub fn shutdown(&mut self, how: Shutdown) -> Result<(), ErrBox> {
    let mut table = lock_resource_table();
    let repr = table
      .get_mut::<CliResource>(self.rid)
      .ok_or_else(bad_resource)?;

    match repr {
      CliResource::TcpStream(ref mut f) => {
        TcpStream::shutdown(f, how).map_err(ErrBox::from)
      }
      _ => Err(bad_resource()),
    }
  }
}

impl Read for Resource {
  fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
    unimplemented!();
  }
}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox>;
}

impl DenoAsyncRead for Resource {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox> {
    let mut table = lock_resource_table();
    let repr = table.get_mut(self.rid).ok_or_else(bad_resource)?;

    let r = match repr {
      CliResource::FsFile(ref mut f) => f.poll_read(buf),
      CliResource::Stdin(ref mut f) => f.poll_read(buf),
      CliResource::TcpStream(ref mut f) => f.poll_read(buf),
      CliResource::ClientTlsStream(ref mut f) => f.poll_read(buf),
      CliResource::ServerTlsStream(ref mut f) => f.poll_read(buf),
      CliResource::HttpBody(ref mut f) => f.poll_read(buf),
      CliResource::ChildStdout(ref mut f) => f.poll_read(buf),
      CliResource::ChildStderr(ref mut f) => f.poll_read(buf),
      _ => {
        return Err(bad_resource());
      }
    };

    r.map_err(ErrBox::from)
  }
}

impl Write for Resource {
  fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
    unimplemented!()
  }

  fn flush(&mut self) -> std::io::Result<()> {
    unimplemented!()
  }
}

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox>;

  fn shutdown(&mut self) -> Poll<(), ErrBox>;
}

impl DenoAsyncWrite for Resource {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox> {
    let mut table = lock_resource_table();
    let repr = table
      .get_mut::<CliResource>(self.rid)
      .ok_or_else(bad_resource)?;

    let r = match repr {
      CliResource::FsFile(ref mut f) => f.poll_write(buf),
      CliResource::Stdout(ref mut f) => f.poll_write(buf),
      CliResource::Stderr(ref mut f) => f.poll_write(buf),
      CliResource::TcpStream(ref mut f) => f.poll_write(buf),
      CliResource::ClientTlsStream(ref mut f) => f.poll_write(buf),
      CliResource::ServerTlsStream(ref mut f) => f.poll_write(buf),
      CliResource::ChildStdin(ref mut f) => f.poll_write(buf),
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

pub fn add_fs_file(fs_file: tokio::fs::File) -> Resource {
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::FsFile(fs_file)));
  Resource { rid }
}

pub fn add_tcp_listener(listener: tokio::net::TcpListener) -> Resource {
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::TcpListener(listener, None)));
  Resource { rid }
}

pub fn add_tls_listener(
  listener: tokio::net::TcpListener,
  acceptor: TlsAcceptor,
) -> Resource {
  let mut table = lock_resource_table();
  let rid =
    table.add(Box::new(CliResource::TlsListener(listener, acceptor, None)));
  Resource { rid }
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> Resource {
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::TcpStream(stream)));
  Resource { rid }
}

pub fn add_tls_stream(stream: ClientTlsStream<TcpStream>) -> Resource {
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::ClientTlsStream(Box::new(stream))));
  Resource { rid }
}

pub fn add_server_tls_stream(stream: ServerTlsStream<TcpStream>) -> Resource {
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::ServerTlsStream(Box::new(stream))));
  Resource { rid }
}

pub fn add_reqwest_body(body: ReqwestDecoder) -> Resource {
  let body = HttpBody::from(body);
  let mut table = lock_resource_table();
  let rid = table.add(Box::new(CliResource::HttpBody(body)));
  Resource { rid }
}

pub struct ChildResources {
  pub child_rid: Option<ResourceId>,
  pub stdin_rid: Option<ResourceId>,
  pub stdout_rid: Option<ResourceId>,
  pub stderr_rid: Option<ResourceId>,
}

pub fn add_child(mut child: tokio_process::Child) -> ChildResources {
  let mut table = lock_resource_table();

  let mut resources = ChildResources {
    child_rid: None,
    stdin_rid: None,
    stdout_rid: None,
    stderr_rid: None,
  };

  if child.stdin().is_some() {
    let stdin = child.stdin().take().unwrap();
    let rid = table.add(Box::new(CliResource::ChildStdin(stdin)));
    resources.stdin_rid = Some(rid);
  }
  if child.stdout().is_some() {
    let stdout = child.stdout().take().unwrap();
    let rid = table.add(Box::new(CliResource::ChildStdout(stdout)));
    resources.stdout_rid = Some(rid);
  }
  if child.stderr().is_some() {
    let stderr = child.stderr().take().unwrap();
    let rid = table.add(Box::new(CliResource::ChildStderr(stderr)));
    resources.stderr_rid = Some(rid);
  }

  let rid = table.add(Box::new(CliResource::Child(Box::new(child))));
  resources.child_rid = Some(rid);

  resources
}

pub struct ChildStatus {
  rid: ResourceId,
}

// Invert the dumbness that tokio_process causes by making Child itself a future.
impl Future for ChildStatus {
  type Item = ExitStatus;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<ExitStatus, ErrBox> {
    let mut table = lock_resource_table();
    let repr = table
      .get_mut::<CliResource>(self.rid)
      .ok_or_else(bad_resource)?;
    match repr {
      CliResource::Child(ref mut child) => child.poll().map_err(ErrBox::from),
      _ => Err(bad_resource()),
    }
  }
}

pub fn child_status(rid: ResourceId) -> Result<ChildStatus, ErrBox> {
  let mut table = lock_resource_table();
  let maybe_repr =
    table.get_mut::<CliResource>(rid).ok_or_else(bad_resource)?;
  match maybe_repr {
    CliResource::Child(ref mut _child) => Ok(ChildStatus { rid }),
    _ => Err(bad_resource()),
  }
}

// TODO: revamp this after the following lands:
// https://github.com/tokio-rs/tokio/pull/785
pub fn get_file(rid: ResourceId) -> Result<std::fs::File, ErrBox> {
  let mut table = lock_resource_table();
  // We take ownership of File here.
  // It is put back below while still holding the lock.
  let repr = table.map.remove(&rid).ok_or_else(bad_resource)?;
  let repr = repr
    .downcast::<CliResource>()
    .or_else(|_| Err(bad_resource()))?;

  match *repr {
    CliResource::FsFile(r) => {
      // Trait Clone not implemented on tokio::fs::File,
      // so convert to std File first.
      let std_file = r.into_std();
      // Create a copy and immediately put back.
      // We don't want to block other resource ops.
      // try_clone() would yield a copy containing the same
      // underlying fd, so operations on the copy would also
      // affect the one in resource table, and we don't need
      // to write back.
      let maybe_std_file_copy = std_file.try_clone();
      // Insert the entry back with the same rid.
      table.map.insert(
        rid,
        Box::new(CliResource::FsFile(tokio_fs::File::from_std(std_file))),
      );

      maybe_std_file_copy.map_err(ErrBox::from)
    }
    _ => Err(bad_resource()),
  }
}

pub fn lookup(rid: ResourceId) -> Result<Resource, ErrBox> {
  debug!("resource lookup {}", rid);
  let table = lock_resource_table();
  let _ = table.get::<CliResource>(rid).ok_or_else(bad_resource)?;
  Ok(Resource { rid })
}

pub fn seek(
  resource: Resource,
  offset: i32,
  whence: u32,
) -> Box<dyn Future<Item = (), Error = ErrBox> + Send> {
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(i64::from(offset)),
    2 => SeekFrom::End(i64::from(offset)),
    _ => {
      return Box::new(futures::future::err(
        deno_error::DenoError::new(
          deno_error::ErrorKind::InvalidSeekMode,
          format!("Invalid seek mode: {}", whence),
        )
        .into(),
      ));
    }
  };

  match get_file(resource.rid) {
    Ok(mut file) => Box::new(futures::future::lazy(move || {
      let result = file.seek(seek_from).map(|_| {}).map_err(ErrBox::from);
      futures::future::result(result)
    })),
    Err(err) => Box::new(futures::future::err(err)),
  }
}
