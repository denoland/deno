// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::futures::TryFutureExt;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_error::JsErrorBox;
use socket2::SockRef;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp;
#[cfg(unix)]
use tokio::net::unix;

/// A full duplex resource has a read and write ends that are completely
/// independent, like TCP/Unix sockets and TLS streams.
#[derive(Debug)]
pub struct FullDuplexResource<R, W> {
  rd: AsyncRefCell<R>,
  wr: AsyncRefCell<W>,
  // When a full-duplex resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures should be attached to this cancel handle.
  cancel_handle: CancelHandle,
}

impl<R, W> FullDuplexResource<R, W>
where
  R: AsyncRead + Unpin + 'static,
  W: AsyncWrite + Unpin + 'static,
{
  pub fn new((rd, wr): (R, W)) -> Self {
    Self {
      rd: rd.into(),
      wr: wr.into(),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> (R, W) {
    (self.rd.into_inner(), self.wr.into_inner())
  }

  pub fn rd_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<R> {
    RcRef::map(self, |r| &r.rd).borrow_mut()
  }

  pub fn wr_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<W> {
    RcRef::map(self, |r| &r.wr).borrow_mut()
  }

  pub fn cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.cancel_handle)
  }

  pub fn cancel_read_ops(&self) {
    self.cancel_handle.cancel()
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let mut rd = self.rd_borrow_mut().await;
    let nread = rd.read(data).try_or_cancel(self.cancel_handle()).await?;
    Ok(nread)
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    let mut wr = self.wr_borrow_mut().await;
    let nwritten = wr.write(data).await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: Rc<Self>) -> Result<(), std::io::Error> {
    let mut wr = self.wr_borrow_mut().await;
    wr.shutdown().await?;
    Ok(())
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MapError {
  #[class(inherit)]
  #[error("{0}")]
  Io(std::io::Error),
  #[class(generic)]
  #[error("Unable to get resources")]
  NoResources,
}

pub type TcpStreamResource =
  FullDuplexResource<tcp::OwnedReadHalf, tcp::OwnedWriteHalf>;

impl Resource for TcpStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    "tcpStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

impl TcpStreamResource {
  pub fn set_nodelay(self: Rc<Self>, nodelay: bool) -> Result<(), MapError> {
    self.map_socket(Box::new(move |socket| socket.set_nodelay(nodelay)))
  }

  pub fn set_keepalive(
    self: Rc<Self>,
    keepalive: bool,
  ) -> Result<(), MapError> {
    self.map_socket(Box::new(move |socket| socket.set_keepalive(keepalive)))
  }

  #[allow(clippy::type_complexity)]
  fn map_socket(
    self: Rc<Self>,
    map: Box<dyn FnOnce(SockRef) -> Result<(), std::io::Error>>,
  ) -> Result<(), MapError> {
    if let Some(wr) = RcRef::map(self, |r| &r.wr).try_borrow() {
      let stream = wr.as_ref().as_ref();
      let socket = socket2::SockRef::from(stream);

      return map(socket).map_err(MapError::Io);
    }

    Err(MapError::NoResources)
  }
}

#[cfg(unix)]
pub type UnixStreamResource =
  FullDuplexResource<unix::OwnedReadHalf, unix::OwnedWriteHalf>;

#[cfg(not(unix))]
pub struct UnixStreamResource;

#[cfg(not(unix))]
impl UnixStreamResource {
  fn read(self: Rc<Self>, _data: &mut [u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  fn write(self: Rc<Self>, _data: &[u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  #[allow(clippy::unused_async)]
  pub async fn shutdown(self: Rc<Self>) -> Result<(), JsErrorBox> {
    unreachable!()
  }
  pub fn cancel_read_ops(&self) {
    unreachable!()
  }
}

impl Resource for UnixStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    "unixStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

#[cfg(unix)]
pub type VsockStreamResource =
  FullDuplexResource<tokio_vsock::OwnedReadHalf, tokio_vsock::OwnedWriteHalf>;

#[cfg(not(unix))]
pub struct VsockStreamResource;

#[cfg(not(unix))]
impl VsockStreamResource {
  fn read(self: Rc<Self>, _data: &mut [u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  fn write(self: Rc<Self>, _data: &[u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  #[allow(clippy::unused_async)]
  pub async fn shutdown(self: Rc<Self>) -> Result<(), JsErrorBox> {
    unreachable!()
  }
  pub fn cancel_read_ops(&self) {
    unreachable!()
  }
}

impl Resource for VsockStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<str> {
    "vsockStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}
