// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::futures::TryFutureExt;
use deno_error::JsErrorBox;
pub use deno_tunnel::Authentication;
pub use deno_tunnel::Error;
pub use deno_tunnel::Event;
pub use deno_tunnel::OwnedReadHalf;
pub use deno_tunnel::OwnedWriteHalf;
pub use deno_tunnel::TunnelAddr;
pub use deno_tunnel::TunnelConnection;
pub use deno_tunnel::TunnelStream;
pub use deno_tunnel::quinn;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

static TUNNEL: OnceLock<TunnelConnection> = OnceLock::new();
static RUN_BEFORE_EXIT: AtomicBool = AtomicBool::new(true);

pub fn set_tunnel(tunnel: TunnelConnection) {
  if TUNNEL.set(tunnel).is_ok() {
    deno_signals::before_exit(before_exit_internal);
  }
}

fn before_exit_internal() {
  if RUN_BEFORE_EXIT.load(Ordering::Relaxed) {
    before_exit();
  }
}

pub fn disable_before_exit() {
  RUN_BEFORE_EXIT.store(false, Ordering::Relaxed);
}

pub fn before_exit() {
  log::trace!("deno_net::tunnel::before_exit >");

  if let Some(tunnel) = get_tunnel() {
    // stay alive long enough to actually send the close frame, since
    // we can't rely on the linux kernel to close this like with tcp.
    deno_core::futures::executor::block_on(tunnel.close(1u32, b""));
  }

  log::trace!("deno_net::tunnel::before_exit <");
}

pub fn get_tunnel() -> Option<&'static TunnelConnection> {
  TUNNEL.get()
}

#[derive(Debug)]
pub struct TunnelStreamResource {
  tx: AsyncRefCell<OwnedWriteHalf>,
  rx: AsyncRefCell<OwnedReadHalf>,
  cancel_handle: CancelHandle,
}

impl TunnelStreamResource {
  pub fn new(stream: TunnelStream) -> Self {
    let (read_half, write_half) = stream.into_split();
    Self {
      tx: AsyncRefCell::new(write_half),
      rx: AsyncRefCell::new(read_half),
      cancel_handle: Default::default(),
    }
  }

  pub fn into_inner(self) -> TunnelStream {
    let tx = self.tx.into_inner();
    let rx = self.rx.into_inner();
    rx.unsplit(tx)
  }

  fn rd_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<OwnedReadHalf> {
    RcRef::map(self, |r| &r.rx).borrow_mut()
  }

  fn wr_borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<OwnedWriteHalf> {
    RcRef::map(self, |r| &r.tx).borrow_mut()
  }

  pub fn cancel_handle(self: &Rc<Self>) -> RcRef<CancelHandle> {
    RcRef::map(self, |r| &r.cancel_handle)
  }
}

impl Resource for TunnelStreamResource {
  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<deno_core::BufView> {
    Box::pin(async move {
      let mut vec = vec![0; limit];
      let nread = self
        .rd_borrow_mut()
        .await
        .read(&mut vec)
        .map_err(|e| JsErrorBox::generic(format!("{e}")))
        .try_or_cancel(self.cancel_handle())
        .await?;
      if nread != vec.len() {
        vec.truncate(nread);
      }
      Ok(vec.into())
    })
  }

  fn read_byob(
    self: Rc<Self>,
    mut buf: deno_core::BufMutView,
  ) -> AsyncResult<(usize, deno_core::BufMutView)> {
    Box::pin(async move {
      let nread = self
        .rd_borrow_mut()
        .await
        .read(&mut buf)
        .map_err(|e| JsErrorBox::generic(format!("{e}")))
        .try_or_cancel(self.cancel_handle())
        .await?;
      Ok((nread, buf))
    })
  }

  fn write(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> AsyncResult<deno_core::WriteOutcome> {
    Box::pin(async move {
      let nwritten = self
        .wr_borrow_mut()
        .await
        .write(&buf)
        .await
        .map_err(|e| JsErrorBox::generic(format!("{e}")))?;
      Ok(deno_core::WriteOutcome::Partial {
        nwritten,
        view: buf,
      })
    })
  }

  fn name(&self) -> std::borrow::Cow<'_, str> {
    "tunnelStream".into()
  }

  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(async move {
      let mut wr = self.wr_borrow_mut().await;
      wr.reset(0u32)
        .map_err(|e| JsErrorBox::generic(format!("{e}")))?;
      Ok(())
    })
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel()
  }
}

#[allow(dead_code)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum StreamHeader {
  Control {
    token: String,
    org: String,
    app: String,
  },
  Stream {
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
  },
  Agent {},
}

#[allow(dead_code)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum ControlMessage {
  Authenticated {
    metadata: HashMap<String, String>,
    addr: SocketAddr,
    hostnames: Vec<String>,
    env: HashMap<String, String>,
  },
  Routed {},
  Migrate {},
}
