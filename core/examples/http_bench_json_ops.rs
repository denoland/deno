// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

struct Logger;

impl log::Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.level() <= log::max_level()
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      println!("{} - {}", record.level(), record.args());
    }
  }

  fn flush(&self) {}
}

// Note: a `tokio::net::TcpListener` doesn't need to be wrapped in a cell,
// because it only supports one op (`accept`) which does not require a mutable
// reference to the listener.
struct TcpListener {
  inner: tokio::net::TcpListener,
  cancel: CancelHandle,
}

impl TcpListener {
  async fn accept(self: Rc<Self>) -> Result<TcpStream, std::io::Error> {
    let cancel = RcRef::map(&self, |r| &r.cancel);
    let stream = self.inner.accept().try_or_cancel(cancel).await?.0.into();
    Ok(stream)
  }
}

impl Resource for TcpListener {
  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl TryFrom<std::net::TcpListener> for TcpListener {
  type Error = std::io::Error;
  fn try_from(
    std_listener: std::net::TcpListener,
  ) -> Result<Self, Self::Error> {
    tokio::net::TcpListener::try_from(std_listener).map(|tokio_listener| Self {
      inner: tokio_listener,
      cancel: Default::default(),
    })
  }
}

struct TcpStream {
  rd: AsyncRefCell<tokio::net::tcp::OwnedReadHalf>,
  wr: AsyncRefCell<tokio::net::tcp::OwnedWriteHalf>,
  // When a `TcpStream` resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures are attached to this cancel handle.
  cancel: CancelHandle,
}

impl TcpStream {
  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, Error> {
    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    let cancel = RcRef::map(self, |r| &r.cancel);
    let nread = rd.read(data).try_or_cancel(cancel).await?;
    Ok(nread)
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    Ok(nwritten)
  }
}

impl Resource for TcpStream {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

impl From<tokio::net::TcpStream> for TcpStream {
  fn from(s: tokio::net::TcpStream) -> Self {
    let (rd, wr) = s.into_split();
    Self {
      rd: rd.into(),
      wr: wr.into(),
      cancel: Default::default(),
    }
  }
}

fn create_js_runtime() -> JsRuntime {
  let ext = deno_core::Extension::builder()
    .ops(vec![op_listen::decl(), op_accept::decl()])
    .build();

  JsRuntime::new(deno_core::RuntimeOptions {
    extensions: vec![ext],
    ..Default::default()
  })
}

#[op]
fn op_listen(state: &mut OpState) -> Result<ResourceId, Error> {
  log::debug!("listen");
  let addr = "127.0.0.1:4570".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
  let rid = state.resource_table.add(listener);
  Ok(rid)
}

#[op]
async fn op_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ResourceId, Error> {
  log::debug!("accept rid={}", rid);

  let listener = state.borrow().resource_table.get::<TcpListener>(rid)?;
  let stream = listener.accept().await?;
  let rid = state.borrow_mut().resource_table.add(stream);
  Ok(rid)
}

fn main() {
  log::set_logger(&Logger).unwrap();
  log::set_max_level(
    env::args()
      .find(|a| a == "-D")
      .map(|_| log::LevelFilter::Debug)
      .unwrap_or(log::LevelFilter::Warn),
  );

  // NOTE: `--help` arg will display V8 help and exit
  deno_core::v8_set_flags(env::args().collect());

  let mut js_runtime = create_js_runtime();
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  let future = async move {
    js_runtime
      .execute_script(
        "http_bench_json_ops.js",
        include_str!("http_bench_json_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop(false).await
  };
  runtime.block_on(future).unwrap();
}
