#[macro_use]
extern crate log;

use deno_core::js_check;
use deno_core::BasicState;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::Script;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::Future;
use serde_json::Value;
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

fn create_isolate() -> CoreIsolate {
  let state = BasicState::new();
  state.register_op_json_sync("listen", op_listen);
  state.register_op_json_sync("close", op_close);
  state.register_op_json_async("accept", op_accept);
  state.register_op_json_async("read", op_read);
  state.register_op_json_async("write", op_write);

  let startup_data = StartupData::Script(Script {
    source: include_str!("http_bench_json_ops.js"),
    filename: "http_bench_json_ops.js",
  });

  CoreIsolate::new(state, startup_data, false)
}

fn op_listen(
  state: &BasicState,
  _args: Value,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let rid = state
    .resource_table
    .borrow_mut()
    .add("tcpListener", Box::new(listener));
  Ok(serde_json::json!({ "rid": rid }))
}

fn op_close(
  state: &BasicState,
  args: Value,
  _buf: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
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
    .borrow_mut()
    .close(rid)
    .map(|_| serde_json::json!(()))
    .ok_or_else(ErrBox::bad_resource_id)
}

fn op_accept(
  state: Rc<BasicState>,
  args: Value,
  _bufs: BufVec,
) -> impl Future<Output = Result<Value, ErrBox>> {
  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("accept rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.resource_table.borrow_mut();
    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;
    listener.poll_accept(cx)?.map(|(stream, _addr)| {
      let rid = resource_table.add("tcpStream", Box::new(stream));
      Ok(serde_json::json!({ "rid": rid }))
    })
  })
}

fn op_read(
  state: Rc<BasicState>,
  args: Value,
  mut bufs: BufVec,
) -> impl Future<Output = Result<Value, ErrBox>> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");

  let rid: u32 = args
    .get("rid")
    .unwrap()
    .as_u64()
    .unwrap()
    .try_into()
    .unwrap();
  debug!("read rid={}", rid);

  poll_fn(move |cx| -> Poll<Result<Value, ErrBox>> {
    let resource_table = &mut state.resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;
    Pin::new(stream)
      .poll_read(cx, &mut bufs[0])?
      .map(|nread| Ok(serde_json::json!({ "nread": nread })))
  })
}

fn op_write(
  state: Rc<BasicState>,
  args: Value,
  bufs: BufVec,
) -> impl Future<Output = Result<Value, ErrBox>> {
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
    let resource_table = &mut state.resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(ErrBox::bad_resource_id)?;
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

  let isolate = create_isolate();
  let mut runtime = runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  js_check(runtime.block_on(isolate));
}
