use deno_core::error::AnyError;
use mio::net::TcpStream;
use std::{
  cell::UnsafeCell,
  future::Future,
  io::{Read, Write},
  pin::Pin,
  sync::{atomic::AtomicBool, Arc, Mutex},
};
use tokio::sync::mpsc;

use crate::ParseStatus;

type TlsTcpStream = rustls::StreamOwned<rustls::ServerConnection, TcpStream>;

#[derive(Debug)]
pub enum InnerStream {
  Tcp(TcpStream),
  Tls(Box<TlsTcpStream>),
}

impl InnerStream {
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
    match self {
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

  #[inline]
  fn as_std(&mut self) -> std::net::TcpStream {
    #[cfg(unix)]
    let std_stream = {
      use std::os::unix::prelude::AsRawFd;
      use std::os::unix::prelude::FromRawFd;
      let fd = match self {
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
      let fd = match self {
        InnerStream::Tcp(ref tcp) => tcp.as_raw_socket(),
        _ => todo!(),
      };
      // SAFETY: `fd` is a valid file descriptor.
      unsafe { std::net::TcpStream::from_raw_socket(fd) }
    };
    std_stream
  }
}

/// A set of data associated with a request that we need to share across the flash
/// thread and the JS thread.
#[derive(Debug)]
pub struct RequestStatesSharedWithJS {
  pub stream: Mutex<InnerStream>,
  pub detached: AtomicBool,
  /// A receiver to get notification about the data availability on the stream.
  /// If it's `None` that means we don't need to read more data.
  pub read_rx: Mutex<Option<mpsc::Receiver<()>>>,
  /// A sender to notify JS thread that some data is available on the stream.
  /// TODO(magurotuna): is it needed to be shared with JS?
  pub read_tx: Mutex<Option<mpsc::Sender<()>>>,
}

/// A set of data associated with a request that we don't need to share with the
/// JS thread.
#[derive(Debug)]
pub struct RequestStatesInFlash {
  pub header_parse_status: ParseStatus,
  pub parse_buffer: UnsafeCell<Vec<u8>>,
}

#[derive(Debug)]
pub struct Stream {
  pub inner: Mutex<InnerStream>,
  pub detached: AtomicBool,
  pub read_rx: Mutex<Option<mpsc::Receiver<()>>>,
  pub read_tx: Mutex<Option<mpsc::Sender<()>>>,
  pub parse_done: Mutex<ParseStatus>,
  pub buffer: Mutex<Vec<u8>>,
  pub read_lock: Arc<Mutex<()>>,
}

impl Stream {
  pub fn detach_ownership(&self) {
    self
      .detached
      .store(true, std::sync::atomic::Ordering::Relaxed);
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

  /*
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
  */
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
    self.inner.lock().unwrap().write(buf)
  }

  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    self.inner.lock().unwrap().flush()
  }
}

impl Read for Stream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    self.inner.lock().unwrap().read(buf)
  }
}

impl Write for InnerStream {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self {
      InnerStream::Tcp(ref mut stream) => stream.write(buf),
      InnerStream::Tls(ref mut stream) => stream.write(buf),
    }
  }
  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    match self {
      InnerStream::Tcp(ref mut stream) => stream.flush(),
      InnerStream::Tls(ref mut stream) => stream.flush(),
    }
  }
}

impl Read for InnerStream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self {
      InnerStream::Tcp(ref mut stream) => stream.read(buf),
      InnerStream::Tls(ref mut stream) => stream.read(buf),
    }
  }
}
