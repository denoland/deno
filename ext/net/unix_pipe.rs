// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io;
use std::path::Path;
use std::rc::Rc;

use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::pipe;

pub struct NamedPipe {
  inner: AsyncRefCell<Inner>,
  cancel: CancelHandle,
}

enum Inner {
  Receiver(pipe::Receiver),
  Sender(pipe::Sender),
}

impl NamedPipe {
  pub fn new_receiver(path: impl AsRef<Path>) -> io::Result<NamedPipe> {
    let receiver = pipe::OpenOptions::new().open_receiver(path)?;
    Ok(NamedPipe {
      inner: AsyncRefCell::new(Inner::Receiver(receiver)),
      cancel: Default::default(),
    })
  }

  pub fn new_sender(path: impl AsRef<Path>) -> io::Result<NamedPipe> {
    let sender = pipe::OpenOptions::new().open_sender(path)?;
    Ok(NamedPipe {
      inner: AsyncRefCell::new(Inner::Sender(sender)),
      cancel: Default::default(),
    })
  }

  pub async fn write(self: Rc<Self>, buf: &[u8]) -> io::Result<usize> {
    let mut inner = RcRef::map(&self, |s| &s.inner).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *inner {
      Inner::Receiver(_) => Err(io::ErrorKind::Unsupported.into()),
      Inner::Sender(sender) => sender.write(buf).try_or_cancel(cancel).await,
    }
  }

  pub async fn read(self: Rc<Self>, buf: &mut [u8]) -> io::Result<usize> {
    let mut inner = RcRef::map(&self, |s| &s.inner).borrow_mut().await;
    let cancel = RcRef::map(&self, |s| &s.cancel);
    match &mut *inner {
      Inner::Receiver(receiver) => {
        receiver.read(buf).try_or_cancel(cancel).await
      }
      Inner::Sender(_) => Err(io::ErrorKind::Unsupported.into()),
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
