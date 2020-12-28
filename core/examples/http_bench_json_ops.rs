// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate log;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::Future;
use serde_json::Value;
use std::cell::RefCell;
use std::convert::TryInto;
use std::env;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::runtime;

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
  let listener = TcpListener::from_std(std_listener)?;
  let rid = state.resource_table.add("tcpListener", Box::new(listener));
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
    .resource_table
    .close(rid)
    .map(|_| serde_json::json!(()))
    .ok_or_else(bad_resource_id)
}

fn op_accept(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> impl Future<Output = Result<Value, AnyError>> {
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("accept rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;

    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(bad_resource_id)?;
    listener.poll_accept(cx)?.map(|(stream, _addr)| {
      let rid = resource_table.add("tcpStream", Box::new(stream));
      Ok(serde_json::json!({ "rid": rid }))
    })
  })
}

fn op_read(
  state: Rc<RefCell<OpState>>,
  args: Value,
  mut bufs: BufVec,
) -> impl Future<Output = Result<Value, AnyError>> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("read rid={}", rid);

  poll_fn(move |cx| -> Poll<Result<Value, AnyError>> {
    let resource_table = &mut state.borrow_mut().resource_table;

    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource_id)?;
    Pin::new(stream)
      .poll_read(cx, &mut bufs[0])?
      .map(|nread| Ok(serde_json::json!({ "nread": nread })))
  })
}

fn op_write(
  state: Rc<RefCell<OpState>>,
  args: Value,
  bufs: BufVec,
) -> impl Future<Output = Result<Value, AnyError>> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("write rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;

    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource_id)?;
    Pin::new(stream)
      .poll_write(cx, &bufs[0])?
      .map(|nwritten| Ok(serde_json::json!({ "nwritten": nwritten })))
  })
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
  let mut runtime = runtime::Builder::new()
    .basic_scheduler()
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
