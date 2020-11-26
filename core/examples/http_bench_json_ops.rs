// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate log;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::AsyncRefFuture;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use serde_json::Value;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::convert::TryInto;
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

// Note: it isn't actually necessary to wrap the `tokio::net::TcpListener` in
// a cell, because it only supports one op (`accept`) which does not require
// a mutable reference to the listener.
struct TcpListener(AsyncRefCell<tokio::net::TcpListener>);

impl Resource for TcpListener {}

impl TcpListener {
  /// Returns a future that yields a shared borrow of the TCP listener.
  fn borrow(self: Rc<Self>) -> AsyncRefFuture<tokio::net::TcpListener> {
    RcRef::map(self, |r| &r.0).borrow()
  }
}

impl TryFrom<std::net::TcpListener> for TcpListener {
  type Error = Error;
  fn try_from(l: std::net::TcpListener) -> Result<Self, Self::Error> {
    tokio::net::TcpListener::try_from(l)
      .map(AsyncRefCell::new)
      .map(Self)
  }
}

struct TcpStream {
  rd: AsyncRefCell<tokio::net::tcp::OwnedReadHalf>,
  wr: AsyncRefCell<tokio::net::tcp::OwnedWriteHalf>,
}

impl Resource for TcpStream {}

impl TcpStream {
  /// Returns a future that yields an exclusive borrow of the read end of the
  /// tcp stream.
  fn rd_borrow_mut(
    self: Rc<Self>,
  ) -> AsyncMutFuture<tokio::net::tcp::OwnedReadHalf> {
    RcRef::map(self, |r| &r.rd).borrow_mut()
  }

  /// Returns a future that yields an exclusive borrow of the write end of the
  /// tcp stream.
  fn wr_borrow_mut(
    self: Rc<Self>,
  ) -> AsyncMutFuture<tokio::net::tcp::OwnedWriteHalf> {
    RcRef::map(self, |r| &r.wr).borrow_mut()
  }
}

impl From<tokio::net::TcpStream> for TcpStream {
  fn from(s: tokio::net::TcpStream) -> Self {
    let (rd, wr) = s.into_split();
    Self {
      rd: rd.into(),
      wr: wr.into(),
    }
  }
}

fn create_js_runtime() -> JsRuntime {
  let mut runtime = JsRuntime::new(Default::default());
  runtime.register_op("listen", deno_core::json_op_sync(op_listen));
  runtime.register_op("close", deno_core::json_op_sync(op_close));
  runtime.register_op("accept", deno_core::json_op_async(op_accept));
  runtime.register_op("read", deno_core::json_op_async(op_read));
  runtime.register_op("write", deno_core::json_op_async(op_write));
  runtime
}

fn op_listen(
  state: &mut OpState,
  _args: Value,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
  let rid = state.resource_table_2.add(listener);
  Ok(serde_json::json!({ "rid": rid }))
}

fn op_close(
  state: &mut OpState,
  args: Value,
  _buf: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("close rid={}", rid);
  state
    .resource_table_2
    .close(rid)
    .map(|_| serde_json::json!(()))
    .ok_or_else(bad_resource_id)
}

async fn op_accept(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("accept rid={}", rid);

  let listener_rc = state
    .borrow()
    .resource_table_2
    .get::<TcpListener>(rid)
    .ok_or_else(bad_resource_id)?;
  let listener_ref = listener_rc.borrow().await;

  let stream: TcpStream = listener_ref.accept().await?.0.into();
  let rid = state.borrow_mut().resource_table_2.add(stream);
  Ok(serde_json::json!({ "rid": rid }))
}

async fn op_read(
  state: Rc<RefCell<OpState>>,
  args: Value,
  mut bufs: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("read rid={}", rid);

  let stream_rc = state
    .borrow()
    .resource_table_2
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut rd_stream_mut = stream_rc.rd_borrow_mut().await;

  let nread = rd_stream_mut.read(&mut bufs[0]).await?;
  Ok(serde_json::json!({ "nread": nread }))
}

async fn op_write(
  state: Rc<RefCell<OpState>>,
  args: Value,
  bufs: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("write rid={}", rid);

  let stream_rc = state
    .borrow()
    .resource_table_2
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut wr_stream_mut = stream_rc.wr_borrow_mut().await;

  let nwritten = wr_stream_mut.write(&bufs[0]).await?;
  Ok(serde_json::json!({ "nwritten": nwritten }))
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
      .execute(
        "http_bench_json_ops.js",
        include_str!("http_bench_json_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop().await
  };
  runtime.block_on(future).unwrap();
}
