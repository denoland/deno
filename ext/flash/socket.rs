use mio::net::TcpStream;
use std::{
  cell::UnsafeCell,
  io::{Read, Write},
  sync::{Arc, Mutex},
};
use tokio::{io::AsyncWrite, sync::mpsc};

use crate::ParseStatus;

type TlsTcpStream = rustls::StreamOwned<rustls::ServerConnection, TcpStream>;

pub enum InnerStream {
  Tcp(TcpStream),
  Tls(Box<TlsTcpStream>),
}

pub struct Stream {
  pub inner: InnerStream,
  pub detached: bool,
  pub read_rx: Option<mpsc::Receiver<()>>,
  pub read_tx: Option<mpsc::Sender<()>>,
  pub parse_done: ParseStatus,
  pub buffer: UnsafeCell<Vec<u8>>,
  pub read_lock: Arc<Mutex<()>>,
}

impl Stream {
  pub fn detach_ownership(&mut self) {
    self.detached = true;
  }

  pub fn reattach_ownership(&mut self) {
    self.detached = false;
  }

  /// Try to write to the socket. If socket will block, return the amount of bytes left.
  #[inline]
  pub fn try_write(&mut self, buf: &[u8]) -> usize {
    let mut nwritten = 0;
    while nwritten < buf.len() {
      match self.write(&buf[nwritten..]) {
        Ok(n) => nwritten += n,
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          break;
        }
        Err(e) => {
          log::trace!("Error writing to socket: {}", e);
          break;
        }
      }
    }
    buf.len() - nwritten
  }

  #[inline]
  pub fn shutdown(&mut self) {
    match &mut self.inner {
      InnerStream::Tcp(stream) => {
        // Typically shutdown shouldn't fail.
        let _ = stream.shutdown(std::net::Shutdown::Both);
      }
      InnerStream::Tls(stream) => {
        let _ = stream.sock.shutdown(std::net::Shutdown::Both);
      }
    }
  }

  #[inline]
  pub(crate) fn poll_write_inner(
    &mut self,
    buf: &[u8],
  ) -> std::task::Poll<std::io::Result<usize>> {
    match self.write(buf) {
      Ok(ret) => std::task::Poll::Ready(Ok(ret)),
      Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        std::task::Poll::Pending
      }
      Err(e) => std::task::Poll::Ready(Err(e)),
    }
  }
}

impl Write for Stream {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self.inner {
      InnerStream::Tcp(ref mut stream) => stream.write(buf),
      InnerStream::Tls(ref mut stream) => stream.write(buf),
    }
  }
  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    match self.inner {
      InnerStream::Tcp(ref mut stream) => stream.flush(),
      InnerStream::Tls(ref mut stream) => stream.flush(),
    }
  }
}

impl Read for Stream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self.inner {
      InnerStream::Tcp(ref mut stream) => stream.read(buf),
      InnerStream::Tls(ref mut stream) => stream.read(buf),
    }
  }
}

impl AsyncWrite for Stream {
  #[inline]
  fn poll_write(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<std::io::Result<usize>> {
    self.get_mut().poll_write_inner(buf)
  }
  #[inline]
  fn poll_flush(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    // no-op for tcp
    std::task::Poll::Ready(Ok(()))
  }
  #[inline]
  fn poll_shutdown(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    unreachable!()
  }
}
