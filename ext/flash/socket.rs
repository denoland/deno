// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::UnsafeCell;
use std::future::Future;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use deno_core::error::AnyError;
use mio::net::TcpStream;
use tokio::sync::mpsc;

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

  /// Try to write to the socket.
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
    nwritten
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

  pub fn as_std(&mut self) -> std::net::TcpStream {
    #[cfg(unix)]
    let std_stream = {
      use std::os::unix::prelude::AsRawFd;
      use std::os::unix::prelude::FromRawFd;
      let fd = match self.inner {
        InnerStream::Tcp(ref tcp) => tcp.as_raw_fd(),
        _ => todo!(),
      };
      // SAFETY: `fd` is a valid file descriptor.
      unsafe { std::net::TcpStream::from_raw_fd(fd) }
    };
    #[cfg(windows)]
    let std_stream = {
      use std::os::windows::prelude::AsRawSocket;
      use std::os::windows::prelude::FromRawSocket;
      let fd = match self.inner {
        InnerStream::Tcp(ref tcp) => tcp.as_raw_socket(),
        _ => todo!(),
      };
      // SAFETY: `fd` is a valid file descriptor.
      unsafe { std::net::TcpStream::from_raw_socket(fd) }
    };
    std_stream
  }

  #[inline]
  pub async fn with_async_stream<F, T>(&mut self, f: F) -> Result<T, AnyError>
  where
    F: FnOnce(
      &mut tokio::net::TcpStream,
    ) -> Pin<Box<dyn '_ + Future<Output = Result<T, AnyError>>>>,
  {
    let mut async_stream = tokio::net::TcpStream::from_std(self.as_std())?;
    let result = f(&mut async_stream).await?;
    forget_stream(async_stream.into_std()?);
    Ok(result)
  }
}

#[inline]
pub fn forget_stream(stream: std::net::TcpStream) {
  #[cfg(unix)]
  {
    use std::os::unix::prelude::IntoRawFd;
    let _ = stream.into_raw_fd();
  }
  #[cfg(windows)]
  {
    use std::os::windows::prelude::IntoRawSocket;
    let _ = stream.into_raw_socket();
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
