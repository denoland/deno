// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::ReadBuf;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::net::windows::named_pipe;

/// A Windows named pipe resource that supports concurrent read and write.
/// This is achieved by splitting the pipe into separate read and write halves,
/// each with its own async lock, allowing duplex communication.
pub struct NamedPipe {
  read_half: AsyncRefCell<NamedPipeRead>,
  write_half: AsyncRefCell<NamedPipeWrite>,
  cancel: CancelHandle,
  /// Server pipe waiting for connection (before split)
  pending_server: AsyncRefCell<Option<named_pipe::NamedPipeServer>>,
}

enum NamedPipeRead {
  Server(ReadHalf<named_pipe::NamedPipeServer>),
  Client(ReadHalf<named_pipe::NamedPipeClient>),
  None,
}

enum NamedPipeWrite {
  Server(WriteHalf<named_pipe::NamedPipeServer>),
  Client(WriteHalf<named_pipe::NamedPipeClient>),
  None,
}

impl NamedPipe {
  pub fn new_server(
    addr: impl AsRef<OsStr>,
    options: &named_pipe::ServerOptions,
  ) -> io::Result<NamedPipe> {
    let server = options.create(addr)?;
    // Server starts in pending state - will be split after connect()
    Ok(NamedPipe {
      read_half: AsyncRefCell::new(NamedPipeRead::None),
      write_half: AsyncRefCell::new(NamedPipeWrite::None),
      cancel: Default::default(),
      pending_server: AsyncRefCell::new(Some(server)),
    })
  }

  pub fn new_client(
    addr: impl AsRef<OsStr>,
    options: &named_pipe::ClientOptions,
  ) -> io::Result<NamedPipe> {
    let client = options.open(addr)?;
    // Client is immediately connected, split into read/write halves
    let (read, write) = tokio::io::split(client);
    Ok(NamedPipe {
      read_half: AsyncRefCell::new(NamedPipeRead::Client(read)),
      write_half: AsyncRefCell::new(NamedPipeWrite::Client(write)),
      cancel: Default::default(),
      pending_server: AsyncRefCell::new(None),
    })
  }

  pub async fn connect(self: Rc<Self>) -> io::Result<()> {
    let mut pending =
      RcRef::map(&self, |s| &s.pending_server).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);

    if let Some(server) = pending.take() {
      // Wait for client to connect
      server.connect().try_or_cancel(cancel).await?;

      // Now split the connected server into read/write halves
      let (read, write) = tokio::io::split(server);

      let mut read_half =
        RcRef::map(&self, |s| &s.read_half).borrow_mut().await;
      let mut write_half =
        RcRef::map(&self, |s| &s.write_half).borrow_mut().await;

      *read_half = NamedPipeRead::Server(read);
      *write_half = NamedPipeWrite::Server(write);
    }
    // Client is already connected, nothing to do
    Ok(())
  }

  pub async fn write(self: Rc<Self>, buf: &[u8]) -> io::Result<usize> {
    let mut write_half =
      RcRef::map(&self, |s| &s.write_half).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *write_half {
      NamedPipeWrite::Server(w) => w.write(buf).try_or_cancel(cancel).await,
      NamedPipeWrite::Client(w) => w.write(buf).try_or_cancel(cancel).await,
      NamedPipeWrite::None => Err(io::Error::new(
        io::ErrorKind::NotConnected,
        "pipe not connected",
      )),
    }
  }

  pub async fn read(self: Rc<Self>, buf: &mut [u8]) -> io::Result<usize> {
    let mut read_half = RcRef::map(&self, |s| &s.read_half).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *read_half {
      NamedPipeRead::Server(r) => r.read(buf).try_or_cancel(cancel).await,
      NamedPipeRead::Client(r) => r.read(buf).try_or_cancel(cancel).await,
      NamedPipeRead::None => Err(io::Error::new(
        io::ErrorKind::NotConnected,
        "pipe not connected",
      )),
    }
  }

  /// Cancel all pending read/write operations on this pipe.
  /// This triggers the `CancelHandle`, causing any in-flight async ops
  /// (e.g., reads started by `readStart()`) to complete with a cancellation
  /// error, releasing their `Rc` references to this resource.
  pub fn cancel_pending_ops(&self) {
    self.cancel.cancel();
  }

  /// Consume this `NamedPipe` and reunite the split read/write halves back
  /// into a `NamedPipeClient`. Only works for client pipes.
  pub fn into_client(self) -> io::Result<named_pipe::NamedPipeClient> {
    let read_half = self.read_half.into_inner();
    let write_half = self.write_half.into_inner();
    match (read_half, write_half) {
      (NamedPipeRead::Client(r), NamedPipeWrite::Client(w)) => Ok(r.unsplit(w)),
      _ => Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "cannot extract client from non-client pipe",
      )),
    }
  }
}

impl Resource for NamedPipe {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    Cow::Borrowed("namedPipe")
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

/// A stub address type for Windows named pipes.
/// Pipes don't have traditional network addresses.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct WindowsPipeAddr;

/// A wrapper around `NamedPipeClient` that implements the stream traits
/// required by the network stream abstraction.
pub struct WindowsPipeStream(named_pipe::NamedPipeClient);

impl WindowsPipeStream {
  pub fn new(client: named_pipe::NamedPipeClient) -> Self {
    Self(client)
  }

  pub fn local_addr(&self) -> io::Result<WindowsPipeAddr> {
    Ok(WindowsPipeAddr)
  }

  pub fn peer_addr(&self) -> io::Result<WindowsPipeAddr> {
    Ok(WindowsPipeAddr)
  }

  pub fn into_split(
    self,
  ) -> (tokio::io::ReadHalf<Self>, tokio::io::WriteHalf<Self>) {
    tokio::io::split(self)
  }
}

impl AsyncRead for WindowsPipeStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
  }
}

impl AsyncWrite for WindowsPipeStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().0).poll_flush(cx)
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
  }
}

/// A stub listener type for Windows named pipes.
/// Not used for HTTP client connections, but required by the
/// network stream abstraction.
pub struct WindowsPipeListener;

impl WindowsPipeListener {
  #[allow(clippy::unused_async, reason = "same interface as unix")]
  pub async fn accept(
    &self,
  ) -> io::Result<(WindowsPipeStream, WindowsPipeAddr)> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "WindowsPipeListener::accept is not supported",
    ))
  }

  pub fn local_addr(&self) -> io::Result<WindowsPipeAddr> {
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "WindowsPipeListener::local_addr is not supported",
    ))
  }
}
