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
use tokio::net::windows::named_pipe;

pub struct NamedPipe {
  inner: AsyncRefCell<Inner>,
  cancel: CancelHandle,
}

enum Inner {
  Server(named_pipe::NamedPipeServer),
  Client(named_pipe::NamedPipeClient),
}

impl NamedPipe {
  pub fn new_server(
    addr: impl AsRef<OsStr>,
    options: &named_pipe::ServerOptions,
  ) -> io::Result<NamedPipe> {
    let server = options.create(addr)?;
    Ok(NamedPipe {
      inner: AsyncRefCell::new(Inner::Server(server)),
      cancel: Default::default(),
    })
  }

  pub fn new_client(
    addr: impl AsRef<OsStr>,
    options: &named_pipe::ClientOptions,
  ) -> io::Result<NamedPipe> {
    let client = options.open(addr)?;
    Ok(NamedPipe {
      inner: AsyncRefCell::new(Inner::Client(client)),
      cancel: Default::default(),
    })
  }

  pub async fn connect(self: Rc<Self>) -> io::Result<()> {
    let mut inner = RcRef::map(&self, |s| &s.inner).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *inner {
      Inner::Server(ref inner) => inner.connect().try_or_cancel(cancel).await,
      Inner::Client(_) => Ok(()),
    }
  }

  pub async fn write(self: Rc<Self>, buf: &[u8]) -> io::Result<usize> {
    let mut inner = RcRef::map(&self, |s| &s.inner).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *inner {
      Inner::Server(server) => server.write(buf).try_or_cancel(cancel).await,
      Inner::Client(client) => client.write(buf).try_or_cancel(cancel).await,
    }
  }

  pub async fn read(self: Rc<Self>, buf: &mut [u8]) -> io::Result<usize> {
    let mut inner = RcRef::map(&self, |s| &s.inner).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *inner {
      Inner::Server(server) => server.read(buf).try_or_cancel(cancel).await,
      Inner::Client(client) => client.read(buf).try_or_cancel(cancel).await,
    }
  }
}

impl Resource for NamedPipe {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    Cow::Borrowed("namedPipe")
  }

  fn close(self: Rc<Self>) {
    let inner = RcRef::map(self, |s| &s.cancel);
    inner.cancel();
  }
}
