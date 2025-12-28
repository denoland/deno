// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io;
use std::rc::Rc;

use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
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
