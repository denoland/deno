// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use resources::{EagerAccept, EagerRead, EagerWrite, Resource};
use tokio_util;
use tokio_write;

use futures::future::{self, Either};
use std;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use tokio;
use tokio::net::{TcpListener, TcpStream};
use tokio_io;

pub fn tcp_read<T: AsMut<[u8]>>(
  tcp_stream: &TcpStream,
  resource: Resource,
  mut buf: T,
) -> EagerRead<Resource, T> {
  // Unforunately we can't just call read() on tokio::net::TcpStream
  let fd = (*tcp_stream).as_raw_fd();
  let mut std_tcp_stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };
  let read_result = std_tcp_stream.read(buf.as_mut());
  // std_tcp_stream will close when it gets dropped. Thus...
  let _ = std_tcp_stream.into_raw_fd();
  match read_result {
    Ok(nread) => Either::B(future::ok((resource, buf, nread))),
    Err(err) => {
      if err.kind() == ErrorKind::WouldBlock {
        Either::A(tokio_io::io::read(resource, buf))
      } else {
        Either::B(future::err(err))
      }
    }
  }
}

pub fn tcp_write<T: AsRef<[u8]>>(
  tcp_stream: &TcpStream,
  resource: Resource,
  buf: T,
) -> EagerWrite<Resource, T> {
  let fd = (*tcp_stream).as_raw_fd();
  let mut std_tcp_stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };
  let write_result = std_tcp_stream.write(buf.as_ref());
  // std_tcp_stream will close when it gets dropped. Thus...
  let _ = std_tcp_stream.into_raw_fd();
  match write_result {
    Ok(nwrite) => Either::B(future::ok((resource, buf, nwrite))),
    Err(err) => {
      if err.kind() == ErrorKind::WouldBlock {
        Either::A(tokio_write::write(resource, buf))
      } else {
        Either::B(future::err(err))
      }
    }
  }
}

pub fn tcp_accept(
  tcp_listener: &TcpListener,
  resource: Resource,
) -> EagerAccept {
  let fd = (*tcp_listener).as_raw_fd();
  let std_listener = unsafe { std::net::TcpListener::from_raw_fd(fd) };
  let result = std_listener.accept();
  // std_listener will close when it gets dropped. Thus...
  let _ = std_listener.into_raw_fd();
  match result {
    Ok((std_stream, addr)) => {
      let result = tokio::net::TcpStream::from_std(
        std_stream,
        &tokio::reactor::Handle::default(),
      );
      let tokio_stream = result.unwrap();
      Either::B(future::ok((tokio_stream, addr)))
    }
    Err(err) => {
      if err.kind() == ErrorKind::WouldBlock {
        Either::A(tokio_util::accept(resource))
      } else {
        Either::B(future::err(err))
      }
    }
  }
}
