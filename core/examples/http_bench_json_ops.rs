#[macro_use]
extern crate log;

use deno_core::serde_json;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ErrBox;
use deno_core::Script;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::Future;
use std::env;
use std::io::Error;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

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

pub fn isolate_new() -> CoreIsolate {
  let startup_data = StartupData::Script(Script {
    source: include_str!("http_bench_json_ops.js"),
    filename: "http_bench_json_ops.js",
  });

  let mut isolate = CoreIsolate::new(startup_data, false);

  isolate.register_op_json_sync("listen", op_listen);
  isolate.register_op_json_async("accept", op_accept);
  isolate.register_op_json_async("read", op_read);
  isolate.register_op_json_async("write", op_write);
  isolate.register_op_json_sync("close", op_close);

  isolate
}

fn op_close(
  state: &mut CoreIsolateState,
  args: serde_json::Value,
  _buf: &mut [ZeroCopyBuf],
) -> Result<serde_json::Value, ErrBox> {
  let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
  debug!("close rid={}", rid);

  let resource_table = &mut state.resource_table.borrow_mut();
  resource_table
    .close(rid)
    .map(|_| serde_json::json!(()))
    .ok_or_else(bad_resource)
}

fn op_listen(
  state: &mut CoreIsolateState,
  _args: serde_json::Value,
  _buf: &mut [ZeroCopyBuf],
) -> Result<serde_json::Value, ErrBox> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let resource_table = &mut state.resource_table.borrow_mut();
  let rid = resource_table.add("tcpListener", Box::new(listener));
  Ok(serde_json::json!({ "rid": rid }))
}

fn op_accept(
  state: &mut CoreIsolateState,
  args: serde_json::Value,
  _buf: &mut [ZeroCopyBuf],
) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
  let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
  debug!("accept rid={}", rid);

  let resource_table = state.resource_table.clone();
  poll_fn(move |cx| {
    let resource_table = &mut resource_table.borrow_mut();
    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(bad_resource)?;
    listener.poll_accept(cx)?.map(|(stream, _addr)| {
      let rid = resource_table.add("tcpStream", Box::new(stream));
      Ok(serde_json::json!({ "rid": rid }))
    })
  })
}

fn op_read(
  state: &mut CoreIsolateState,
  args: serde_json::Value,
  bufs: &mut [ZeroCopyBuf],
) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
  debug!("read rid={}", rid);

  let mut buf = bufs[0].clone();
  let resource_table = state.resource_table.clone();

  poll_fn(move |cx| -> Poll<Result<serde_json::Value, ErrBox>> {
    let resource_table = &mut resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource)?;
    Pin::new(stream)
      .poll_read(cx, &mut buf)?
      .map(|nread| Ok(serde_json::json!({ "nread": nread })))
  })
}

fn op_write(
  state: &mut CoreIsolateState,
  args: serde_json::Value,
  bufs: &mut [ZeroCopyBuf],
) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
  debug!("write rid={}", rid);

  let buf = bufs[0].clone();
  let resource_table = state.resource_table.clone();

  poll_fn(move |cx| {
    let resource_table = &mut resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource)?;
    Pin::new(stream)
      .poll_write(cx, &buf)?
      .map(|nwritten| Ok(serde_json::json!({ "nwritten": nwritten })))
  })
}

fn bad_resource() -> ErrBox {
  Error::new(ErrorKind::NotFound, "bad resource id").into()
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

  let isolate = isolate_new();
  let mut runtime = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  deno_core::js_check(runtime.block_on(isolate));
}
