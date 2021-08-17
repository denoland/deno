// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ops_tls as tls;
use deno_core::error::not_supported;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpPair;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp;

#[cfg(unix)]
use tokio::net::unix;

pub fn init() -> Vec<OpPair> {
  vec![
    ("op_net_read_async", op_async(op_read_async)),
    ("op_net_write_async", op_async(op_write_async)),
    ("op_net_shutdown", op_async(op_shutdown)),
  ]
}

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
    self: &Rc<Self>,
    buf: &mut [u8],
  ) -> Result<usize, AnyError> {
    let mut rd = self.rd_borrow_mut().await;
    let nread = rd.read(buf).try_or_cancel(self.cancel_handle()).await?;
    Ok(nread)
  }

  pub async fn write(self: &Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let mut wr = self.wr_borrow_mut().await;
    let nwritten = wr.write(buf).await?;
    Ok(nwritten)
  }

  pub async fn shutdown(self: &Rc<Self>) -> Result<(), AnyError> {
    let mut wr = self.wr_borrow_mut().await;
    wr.shutdown().await?;
    Ok(())
  }
}

pub type TcpStreamResource =
  FullDuplexResource<tcp::OwnedReadHalf, tcp::OwnedWriteHalf>;

impl Resource for TcpStreamResource {
  fn name(&self) -> Cow<str> {
    "tcpStream".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

pub type TlsStreamResource = FullDuplexResource<tls::ReadHalf, tls::WriteHalf>;

impl Resource for TlsStreamResource {
  fn name(&self) -> Cow<str> {
    "tlsStream".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

#[cfg(unix)]
pub type UnixStreamResource =
  FullDuplexResource<unix::OwnedReadHalf, unix::OwnedWriteHalf>;

#[cfg(not(unix))]
pub struct UnixStreamResource;

#[cfg(not(unix))]
impl UnixStreamResource {
  pub async fn read(
    self: &Rc<Self>,
    _buf: &mut [u8],
  ) -> Result<usize, AnyError> {
    unreachable!()
  }
  pub async fn write(self: &Rc<Self>, _buf: &[u8]) -> Result<usize, AnyError> {
    unreachable!()
  }
  pub async fn shutdown(self: &Rc<Self>) -> Result<(), AnyError> {
    unreachable!()
  }
  pub fn cancel_read_ops(&self) {
    unreachable!()
  }
}

impl Resource for UnixStreamResource {
  fn name(&self) -> Cow<str> {
    "unixStream".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel_read_ops();
  }
}

async fn op_read_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let buf = &mut buf.ok_or_else(null_opbuf)?;
  let resource = state.borrow().resource_table.get_any(rid)?;
  let nread = if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.read(buf).await?
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.read(buf).await?
  } else {
    return Err(not_supported());
  };
  Ok(nread as u32)
}

async fn op_write_async(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<u32, AnyError> {
  let buf = &buf.ok_or_else(null_opbuf)?;
  let resource = state.borrow().resource_table.get_any(rid)?;
  let nwritten = if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.write(buf).await?
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.write(buf).await?
  } else {
    return Err(not_supported());
  };
  Ok(nwritten as u32)
}

async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<(), AnyError> {
  let resource = state.borrow().resource_table.get_any(rid)?;
  if let Some(s) = resource.downcast_rc::<TcpStreamResource>() {
    s.shutdown().await?;
  } else if let Some(s) = resource.downcast_rc::<TlsStreamResource>() {
    s.shutdown().await?;
  } else if let Some(s) = resource.downcast_rc::<UnixStreamResource>() {
    s.shutdown().await?;
  } else {
    return Err(not_supported());
  }
  Ok(())
}
