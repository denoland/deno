// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::futures::TryFutureExt;
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
  // op_cancel_read only cancels read ops. Closing the resource cancels both
  // halves so blocked writes don't keep the underlying socket alive.
  read_cancel_handle: CancelHandle,
  write_cancel_handle: CancelHandle,
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
      read_cancel_handle: Default::default(),
      write_cancel_handle: Default::default(),
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

  pub fn read_cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.read_cancel_handle)
  }

  pub fn write_cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.write_cancel_handle)
  }

  pub fn cancel_read_ops(&self) {
    self.read_cancel_handle.cancel()
  }

  pub fn cancel_ops(&self) {
    self.read_cancel_handle.cancel();
    self.write_cancel_handle.cancel();
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    let mut rd = self.rd_borrow_mut().await;
    let nread = rd
      .read(data)
      .try_or_cancel(self.read_cancel_handle())
      .await?;
    Ok(nread)
  }

  pub async fn write(
    self: Rc<Self>,
    data: &[u8],
  ) -> Result<usize, std::io::Error> {
    let mut wr = self.wr_borrow_mut().await;
    let nwritten = wr
      .write(data)
      .try_or_cancel(self.write_cancel_handle())
      .await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: Rc<Self>) -> Result<(), std::io::Error> {
    let mut wr = self.wr_borrow_mut().await;
    wr.shutdown()
      .try_or_cancel(self.write_cancel_handle())
      .await?;
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

  fn name(&self) -> Cow<'_, str> {
    "tcpStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_ops();
  }

  // Override the trait's no-op default. Without this, `self.cancel_read_ops()`
  // (in `close()` and via `op_cancel_read`) resolves to `Resource`'s default
  // empty `cancel_read_ops(self: Rc<Self>)` rather than the inherent
  // `FullDuplexResource::cancel_read_ops(&self)`, because the trait method's
  // by-value `Rc<Self>` receiver is an exact match while the inherent `&self`
  // method needs an autoref. The result is that closing a TCP stream never
  // cancels its in-flight read, so the socket is never dropped and no FIN is
  // sent until the process exits.
  fn cancel_read_ops(self: Rc<Self>) {
    TcpStreamResource::cancel_read_ops(&self);
  }
}

impl TcpStreamResource {
  pub fn dup_raw_fd(self: &Rc<Self>) -> Option<i32> {
    let wr = RcRef::map(self, |r| &r.wr).try_borrow()?;
    let sock = SockRef::from(wr.as_ref().as_ref()).try_clone().ok()?;
    #[cfg(unix)]
    {
      use std::os::unix::io::IntoRawFd;
      Some(sock.into_raw_fd())
    }
    #[cfg(windows)]
    {
      use std::os::windows::io::IntoRawSocket;
      i32::try_from(sock.into_raw_socket()).ok()
    }
  }

  pub fn set_nodelay(self: Rc<Self>, nodelay: bool) -> Result<(), MapError> {
    self.map_socket(Box::new(move |socket| socket.set_nodelay(nodelay)))
  }

  pub fn set_keepalive(
    self: Rc<Self>,
    keepalive: bool,
  ) -> Result<(), MapError> {
    self.map_socket(Box::new(move |socket| socket.set_keepalive(keepalive)))
  }

  #[allow(clippy::type_complexity, reason = "internal code")]
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
  #[allow(clippy::unused_async, reason = "not supported")]
  pub async fn shutdown(self: Rc<Self>) -> Result<(), JsErrorBox> {
    unreachable!()
  }
  pub fn cancel_read_ops(&self) {
    unreachable!()
  }
  pub fn cancel_ops(&self) {
    unreachable!()
  }
}

impl Resource for UnixStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    "unixStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_ops();
  }

  // See the note on TcpStreamResource::cancel_read_ops.
  fn cancel_read_ops(self: Rc<Self>) {
    UnixStreamResource::cancel_read_ops(&self);
  }
}

#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
pub type VsockStreamResource =
  FullDuplexResource<tokio_vsock::OwnedReadHalf, tokio_vsock::OwnedWriteHalf>;

#[cfg(not(any(
  target_os = "android",
  target_os = "linux",
  target_os = "macos"
)))]
pub struct VsockStreamResource;

#[cfg(not(any(
  target_os = "android",
  target_os = "linux",
  target_os = "macos"
)))]
impl VsockStreamResource {
  fn read(self: Rc<Self>, _data: &mut [u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  fn write(self: Rc<Self>, _data: &[u8]) -> AsyncResult<usize> {
    unreachable!()
  }
  #[allow(clippy::unused_async, reason = "not supported")]
  pub async fn shutdown(self: Rc<Self>) -> Result<(), JsErrorBox> {
    unreachable!()
  }
  pub fn cancel_read_ops(&self) {
    unreachable!()
  }
  pub fn cancel_ops(&self) {
    unreachable!()
  }
}

impl Resource for VsockStreamResource {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn name(&self) -> Cow<'_, str> {
    "vsockStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(self.shutdown().map_err(JsErrorBox::from_err))
  }

  fn close(self: Rc<Self>) {
    self.cancel_ops();
  }

  // See the note on TcpStreamResource::cancel_read_ops.
  fn cancel_read_ops(self: Rc<Self>) {
    VsockStreamResource::cancel_read_ops(&self);
  }
}

#[cfg(test)]
mod tests {
  use std::io;
  use std::pin::Pin;
  use std::rc::Rc;
  use std::task::Context;
  use std::task::Poll;
  use std::time::Duration;

  use tokio::io::AsyncRead;
  use tokio::io::AsyncWrite;
  use tokio::io::ReadBuf;

  use super::FullDuplexResource;

  struct PendingRead;

  impl AsyncRead for PendingRead {
    fn poll_read(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      _buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Pending
    }
  }

  struct PendingWrite;

  impl AsyncWrite for PendingWrite {
    fn poll_write(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
      Poll::Pending
    }

    fn poll_flush(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Pending
    }

    fn poll_shutdown(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Pending
    }
  }

  #[tokio::test]
  async fn close_cancels_pending_write() {
    let resource =
      Rc::new(FullDuplexResource::new((PendingRead, PendingWrite)));
    let write = resource.clone().write(&[1, 2, 3]);
    tokio::pin!(write);

    assert!(
      tokio::time::timeout(Duration::from_millis(10), &mut write)
        .await
        .is_err()
    );

    resource.cancel_read_ops();
    assert!(
      tokio::time::timeout(Duration::from_millis(10), &mut write)
        .await
        .is_err(),
      "canceling reads should not cancel writes"
    );

    resource.cancel_ops();
    let err = tokio::time::timeout(Duration::from_secs(1), write)
      .await
      .expect("write should be canceled")
      .expect_err("write should reject");
    assert_eq!(err.kind(), io::ErrorKind::Interrupted);
  }
}
