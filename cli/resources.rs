// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::deno_error::bad_resource;
use crate::http_body::HttpBody;
use deno::ErrBox;
pub use deno::Resource;
pub use deno::ResourceId;
use deno::ResourceTable;

use futures;
use futures::Future;
use futures::Poll;
use reqwest::r#async::Decoder as ReqwestDecoder;
use std;
use std::process::ExitStatus;
use std::sync::Mutex;
use std::sync::MutexGuard;
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
  static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new({
    let mut table = ResourceTable::default();

    // TODO Load these lazily during lookup?
    table.add("stdin", Box::new(CliResource::Stdin(tokio::io::stdin())));

    table.add("stdout", Box::new(CliResource::Stdout({
      #[cfg(not(windows))]
      let stdout = unsafe { std::fs::File::from_raw_fd(1) };
      #[cfg(windows)]
      let stdout = unsafe {
        std::fs::File::from_raw_handle(winapi::um::processenv::GetStdHandle(
            winapi::um::winbase::STD_OUTPUT_HANDLE))
      };
      tokio::fs::File::from_std(stdout)
    })));

    table.add("stderr", Box::new(CliResource::Stderr(tokio::io::stderr())));
    table
  });
}

// TODO: move listeners out of this enum and rename to `StreamResource`
pub enum CliResource {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::fs::File),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
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

impl Resource for CliResource {}

pub fn lock_resource_table<'a>() -> MutexGuard<'a, ResourceTable> {
  RESOURCE_TABLE.lock().unwrap()
}

/// `DenoAsyncRead` is the same as the `tokio_io::AsyncRead` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncRead {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox>;
}

impl DenoAsyncRead for CliResource {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, ErrBox> {
    let r = match self {
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

/// `DenoAsyncWrite` is the same as the `tokio_io::AsyncWrite` trait
/// but uses an `ErrBox` error instead of `std::io:Error`
pub trait DenoAsyncWrite {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox>;

  fn shutdown(&mut self) -> Poll<(), ErrBox>;
}

impl DenoAsyncWrite for CliResource {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, ErrBox> {
    let r = match self {
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

pub fn add_fs_file(fs_file: tokio::fs::File) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("fsFile", Box::new(CliResource::FsFile(fs_file)))
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> ResourceId {
  let mut table = lock_resource_table();
  table.add("tcpStream", Box::new(CliResource::TcpStream(stream)))
}

pub fn add_tls_stream(stream: ClientTlsStream<TcpStream>) -> ResourceId {
  let mut table = lock_resource_table();
  table.add(
    "clientTlsStream",
    Box::new(CliResource::ClientTlsStream(Box::new(stream))),
  )
}

pub fn add_server_tls_stream(stream: ServerTlsStream<TcpStream>) -> ResourceId {
  let mut table = lock_resource_table();
  table.add(
    "serverTlsStream",
    Box::new(CliResource::ServerTlsStream(Box::new(stream))),
  )
}

pub fn add_reqwest_body(body: ReqwestDecoder) -> ResourceId {
  let body = HttpBody::from(body);
  let mut table = lock_resource_table();
  table.add("httpBody", Box::new(CliResource::HttpBody(body)))
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
    let rid = table.add("childStdin", Box::new(CliResource::ChildStdin(stdin)));
    resources.stdin_rid = Some(rid);
  }
  if child.stdout().is_some() {
    let stdout = child.stdout().take().unwrap();
    let rid =
      table.add("childStdout", Box::new(CliResource::ChildStdout(stdout)));
    resources.stdout_rid = Some(rid);
  }
  if child.stderr().is_some() {
    let stderr = child.stderr().take().unwrap();
    let rid =
      table.add("childStderr", Box::new(CliResource::ChildStderr(stderr)));
    resources.stderr_rid = Some(rid);
  }

  let rid = table.add("child", Box::new(CliResource::Child(Box::new(child))));
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
  let (_name, repr) = table.map.remove(&rid).ok_or_else(bad_resource)?;
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
        (
          "fsFile".to_string(),
          Box::new(CliResource::FsFile(tokio_fs::File::from_std(std_file))),
        ),
      );

      maybe_std_file_copy.map_err(ErrBox::from)
    }
    _ => Err(bad_resource()),
  }
}
