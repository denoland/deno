// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privlaged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

#[cfg(unix)]
use eager_unix as eager;
use errors::DenoError;
use tokio_util;
use tokio_write;

use futures;
use futures::future::{Either, FutureResult};
use futures::Poll;
use std;
use std::collections::HashMap;
use std::io::{Error, Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_io;

pub type ResourceId = i32; // Sometimes referred to RID.

// These store Deno's file descriptors. These are not necessarily the operating
// system ones.
type ResourceTable = HashMap<ResourceId, Repr>;

lazy_static! {
  // Starts at 3 because stdio is [0-2].
  static ref NEXT_RID: AtomicIsize = AtomicIsize::new(3);
  static ref RESOURCE_TABLE: Mutex<ResourceTable> = Mutex::new({
    let mut m = HashMap::new();
    // TODO Load these lazily during lookup?
    m.insert(0, Repr::Stdin(tokio::io::stdin()));
    m.insert(1, Repr::Stdout(tokio::io::stdout()));
    m.insert(2, Repr::Stderr(tokio::io::stderr()));
    m
  });
}

// Internal representation of Resource.
enum Repr {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::io::Stdout),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
  TcpListener(tokio::net::TcpListener),
  TcpStream(tokio::net::TcpStream),
}

// Abstract async file interface.
// Ideally in unix, if Resource represents an OS rid, it will be the same.
#[derive(Debug)]
pub struct Resource {
  pub rid: ResourceId,
}

impl Resource {
  // TODO Should it return a Resource instead of net::TcpStream?
  pub fn poll_accept(&mut self) -> Poll<(TcpStream, SocketAddr), Error> {
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);
    match maybe_repr {
      None => panic!("bad rid"),
      Some(repr) => match repr {
        Repr::TcpListener(ref mut s) => s.poll_accept(),
        _ => panic!("Cannot accept"),
      },
    }
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the RESOURCE_TABLE.
  pub fn close(&mut self) {
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let r = table.remove(&self.rid);
    assert!(r.is_some());
  }

  pub fn shutdown(&mut self, how: Shutdown) -> Result<(), DenoError> {
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);
    match maybe_repr {
      None => panic!("bad rid"),
      Some(repr) => match repr {
        Repr::TcpStream(ref mut f) => {
          TcpStream::shutdown(f, how).map_err(|err| DenoError::from(err))
        }
        _ => panic!("Cannot shutdown"),
      },
    }
  }
}

impl Read for Resource {
  fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
    unimplemented!();
  }
}

impl AsyncRead for Resource {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, Error> {
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);
    match maybe_repr {
      None => panic!("bad rid"),
      Some(repr) => match repr {
        Repr::FsFile(ref mut f) => f.poll_read(buf),
        Repr::Stdin(ref mut f) => f.poll_read(buf),
        Repr::TcpStream(ref mut f) => f.poll_read(buf),
        Repr::Stdout(_) | Repr::Stderr(_) => {
          panic!("Cannot read from stdout/stderr")
        }
        Repr::TcpListener(_) => panic!("Cannot read"),
      },
    }
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

impl AsyncWrite for Resource {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, Error> {
    let mut table = RESOURCE_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.rid);
    match maybe_repr {
      None => panic!("bad rid"),
      Some(repr) => match repr {
        Repr::FsFile(ref mut f) => f.poll_write(buf),
        Repr::Stdout(ref mut f) => f.poll_write(buf),
        Repr::Stderr(ref mut f) => f.poll_write(buf),
        Repr::TcpStream(ref mut f) => f.poll_write(buf),
        Repr::Stdin(_) => panic!("Cannot write to stdin"),
        Repr::TcpListener(_) => panic!("Cannot write"),
      },
    }
  }

  fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
    unimplemented!()
  }
}

fn new_rid() -> ResourceId {
  let next_rid = NEXT_RID.fetch_add(1, Ordering::SeqCst);
  next_rid as ResourceId
}

pub fn add_fs_file(fs_file: tokio::fs::File) -> Resource {
  let rid = new_rid();
  let mut tg = RESOURCE_TABLE.lock().unwrap();
  match tg.insert(rid, Repr::FsFile(fs_file)) {
    Some(_) => panic!("There is already a file with that rid"),
    None => Resource { rid },
  }
}

pub fn add_tcp_listener(listener: tokio::net::TcpListener) -> Resource {
  let rid = new_rid();
  let mut tg = RESOURCE_TABLE.lock().unwrap();
  let r = tg.insert(rid, Repr::TcpListener(listener));
  assert!(r.is_none());
  Resource { rid }
}

pub fn add_tcp_stream(stream: tokio::net::TcpStream) -> Resource {
  let rid = new_rid();
  let mut tg = RESOURCE_TABLE.lock().unwrap();
  let r = tg.insert(rid, Repr::TcpStream(stream));
  assert!(r.is_none());
  Resource { rid }
}

pub fn lookup(rid: ResourceId) -> Option<Resource> {
  let table = RESOURCE_TABLE.lock().unwrap();
  table.get(&rid).map(|_| Resource { rid })
}

pub type EagerRead<R, T> =
  Either<tokio_io::io::Read<R, T>, FutureResult<(R, T, usize), std::io::Error>>;

pub type EagerWrite<R, T> =
  Either<tokio_write::Write<R, T>, FutureResult<(R, T, usize), std::io::Error>>;

pub type EagerAccept = Either<
  tokio_util::Accept,
  FutureResult<(tokio::net::TcpStream, std::net::SocketAddr), std::io::Error>,
>;

#[cfg(not(unix))]
#[allow(unused_mut)]
pub fn eager_read<T: AsMut<[u8]>>(
  resource: Resource,
  mut buf: T,
) -> EagerRead<Resource, T> {
  Either::A(tokio_io::io::read(resource, buf)).into()
}

#[cfg(not(unix))]
pub fn eager_write<T: AsRef<[u8]>>(
  resource: Resource,
  buf: T,
) -> EagerWrite<Resource, T> {
  Either::A(tokio_write::write(resource, buf)).into()
}

#[cfg(not(unix))]
pub fn eager_accept(resource: Resource) -> EagerAccept {
  Either::A(tokio_util::accept(resource)).into()
}

// This is an optimization that Tokio should do.
// Attempt to call read() on the main thread.
#[cfg(unix)]
pub fn eager_read<T: AsMut<[u8]>>(
  resource: Resource,
  buf: T,
) -> EagerRead<Resource, T> {
  let mut table = RESOURCE_TABLE.lock().unwrap();
  let maybe_repr = table.get_mut(&resource.rid);
  match maybe_repr {
    None => panic!("bad rid"),
    Some(repr) => match repr {
      Repr::TcpStream(ref mut tcp_stream) => {
        eager::tcp_read(tcp_stream, resource, buf)
      }
      _ => Either::A(tokio_io::io::read(resource, buf)),
    },
  }
}

// This is an optimization that Tokio should do.
// Attempt to call write() on the main thread.
#[cfg(unix)]
pub fn eager_write<T: AsRef<[u8]>>(
  resource: Resource,
  buf: T,
) -> EagerWrite<Resource, T> {
  let mut table = RESOURCE_TABLE.lock().unwrap();
  let maybe_repr = table.get_mut(&resource.rid);
  match maybe_repr {
    None => panic!("bad rid"),
    Some(repr) => match repr {
      Repr::TcpStream(ref mut tcp_stream) => {
        eager::tcp_write(tcp_stream, resource, buf)
      }
      _ => Either::A(tokio_write::write(resource, buf)),
    },
  }
}

#[cfg(unix)]
pub fn eager_accept(resource: Resource) -> EagerAccept {
  let mut table = RESOURCE_TABLE.lock().unwrap();
  let maybe_repr = table.get_mut(&resource.rid);
  match maybe_repr {
    None => panic!("bad rid"),
    Some(repr) => match repr {
      Repr::TcpListener(ref mut tcp_listener) => {
        eager::tcp_accept(tcp_listener, resource)
      }
      _ => Either::A(tokio_util::accept(resource)),
    },
  }
}
