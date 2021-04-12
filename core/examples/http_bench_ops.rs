// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::AsyncRefCell;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::env;
use std::io::Error;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

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
  async fn accept(self: Rc<Self>) -> Result<TcpStream, Error> {
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
  type Error = Error;
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
  async fn read(self: Rc<Self>, buf: &mut [u8]) -> Result<usize, Error> {
    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    let cancel = RcRef::map(self, |r| &r.cancel);
    rd.read(buf).try_or_cancel(cancel).await
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, Error> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    wr.write(buf).await
  }
}

impl Resource for TcpStream {
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
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("listen", deno_core::op_sync(op_listen));
  runtime.register_op("close", deno_core::op_sync(op_close));
  runtime.register_op("accept", deno_core::op_async(op_accept));
  runtime.register_op("read", deno_core::op_async(op_read));
  runtime.register_op("write", deno_core::op_async(op_write));
  runtime
}

fn op_listen(
  state: &mut OpState,
  _args: (),
  _bufs: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  log::debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
  let rid = state.resource_table.add(listener);
  Ok(rid)
}

fn op_close(
  state: &mut OpState,
  rid: ResourceId,
  _buf: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  log::debug!("close rid={}", rid);
  state
    .resource_table
    .close(rid)
    .map(|_| ())
    .ok_or_else(bad_resource_id)
}

async fn op_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _buf: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  log::debug!("accept rid={}", rid);

  let listener = state
    .borrow()
    .resource_table
    .get::<TcpListener>(rid)
    .ok_or_else(bad_resource_id)?;
  let stream = listener.accept().await?;
  let rid = state.borrow_mut().resource_table.add(stream);
  Ok(rid)
}

async fn op_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<usize, AnyError> {
  let mut buf = buf.ok_or_else(null_opbuf)?;
  log::debug!("read rid={}", rid);

  let stream = state
    .borrow()
    .resource_table
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  let nread = stream.read(&mut buf).await?;
  Ok(nread)
}

async fn op_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<usize, AnyError> {
  let buf = buf.ok_or_else(null_opbuf)?;
  log::debug!("write rid={}", rid);

  let stream = state
    .borrow()
    .resource_table
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  let nwritten = stream.write(&buf).await?;
  Ok(nwritten)
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
      .execute("http_bench_ops.js", include_str!("http_bench_ops.js"))
      .unwrap();
    js_runtime.run_event_loop().await
  };
  runtime.block_on(future).unwrap();
}
