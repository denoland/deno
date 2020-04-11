#[macro_use]
extern crate derive_deref;
#[macro_use]
extern crate log;

use deno_core::Isolate as CoreIsolate;
use deno_core::JsonError;
use deno_core::*;
use futures::future::poll_fn;
use futures::prelude::*;
use futures::task::Context;
use futures::task::Poll;
use serde_derive::Deserialize;
use serde_json;
use serde_json::json;
use serde_json::Value;
use std::cell::RefCell;
use std::env;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
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

struct Isolate {
  core_isolate: Box<CoreIsolate>, // Unclear why CoreIsolate::new() returns a box.
  state: State,
}

#[derive(Clone, Default, Deref)]
struct State(Rc<RefCell<StateInner>>);

#[derive(Default)]
struct StateInner {
  resource_table: ResourceTable,
}

impl Isolate {
  pub fn new() -> Self {
    let mut isolate = Self {
      core_isolate: CoreIsolate::new(StartupData::None, false),
      state: Default::default(),
    };

    isolate.register_op("listen", op_listen);
    isolate.register_op("accept", op_accept);
    isolate.register_op("read", op_read);
    isolate.register_op("write", op_write);
    isolate.register_op("close", op_close);

    isolate
  }

  fn register_op<D>(&mut self, name: &str, dispatcher: D)
  where
    D: Fn(
        State,
        Value,
        Option<ZeroCopyBuf>,
      ) -> Result<JsonOp<BenchError>, BenchError>
      + 'static,
  {
    let wrapped_op = json_op(self.stateful_op(dispatcher));
    self.core_isolate.register_op(name, wrapped_op);
  }

  fn stateful_op<D>(
    &mut self,
    dispatcher: D,
  ) -> impl Fn(Value, Option<ZeroCopyBuf>) -> Result<JsonOp<BenchError>, BenchError>
  where
    D: Fn(
      State,
      Value,
      Option<ZeroCopyBuf>,
    ) -> Result<JsonOp<BenchError>, BenchError>,
  {
    let state = self.state.clone();

    move |args: Value,
          zero_copy: Option<ZeroCopyBuf>|
          -> Result<JsonOp<BenchError>, BenchError> {
      dispatcher(state.clone(), args, zero_copy)
    }
  }
}

impl Future for Isolate {
  type Output = <CoreIsolate as Future>::Output;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    self.core_isolate.poll_unpin(cx)
  }
}

#[derive(Deserialize)]
struct RidArgs {
  rid: i32,
}

fn op_close(
  state: State,
  args: Value,
  _buf: Option<ZeroCopyBuf>,
) -> Result<JsonOp<BenchError>, BenchError> {
  let args: RidArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  debug!("close rid={}", rid);

  let resource_table = &mut state.borrow_mut().resource_table;
  if resource_table.close(rid).is_none() {
    return Err(BenchError::bad_rid());
  }

  Ok(JsonOp::Sync(json!(0)))
}

fn op_listen(
  state: State,
  _args: Value,
  _buf: Option<ZeroCopyBuf>,
) -> Result<JsonOp<BenchError>, BenchError> {
  debug!("listen");

  let op = async move {
    let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let resource_table = &mut state.borrow_mut().resource_table;
    let rid = resource_table.add("tcpListener", Box::new(listener));
    Ok(json!(rid))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_accept(
  state: State,
  args: Value,
  _buf: Option<ZeroCopyBuf>,
) -> Result<JsonOp<BenchError>, BenchError> {
  let args: RidArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  debug!("accept rid={}", rid);

  let op = poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(BenchError::bad_rid)
      .unwrap();
    listener.poll_accept(cx).map_err(BenchError::from).map_ok(
      |(stream, _addr)| {
        let rid = resource_table.add("tcpStream", Box::new(stream));
        json!(rid)
      },
    )
  });

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_read(
  state: State,
  args: Value,
  buf: Option<ZeroCopyBuf>,
) -> Result<JsonOp<BenchError>, BenchError> {
  let args: RidArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let mut buf = buf.unwrap();
  debug!("read rid={}", rid);

  let op = poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(BenchError::bad_rid)
      .unwrap();
    Pin::new(stream)
      .poll_read(cx, &mut buf)
      .map_err(BenchError::from)
      .map_ok(|nread| json!(nread))
  });

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_write(
  state: State,
  args: Value,
  buf: Option<ZeroCopyBuf>,
) -> Result<JsonOp<BenchError>, BenchError> {
  let args: RidArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let buf = buf.unwrap();
  debug!("write rid={}", rid);

  let op = poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(BenchError::bad_rid)
      .unwrap();
    Pin::new(stream)
      .poll_write(cx, &buf)
      .map_err(BenchError::from)
      .map_ok(|nwritten| json!(nwritten))
  });

  Ok(JsonOp::Async(op.boxed_local()))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum BenchErrorKind {
  JsonError = 1,
  BadResourceId = 2,
  IoError = 3,
}

#[derive(Debug)]
struct BenchError {
  kind: BenchErrorKind,
  msg: String,
}

impl BenchError {
  fn bad_rid() -> Self {
    Self {
      kind: BenchErrorKind::BadResourceId,
      msg: "Bad resource id".to_string(),
    }
  }
}

impl JsonError for BenchError {
  fn kind(&self) -> i32 {
    self.kind as i32
  }

  fn message(&self) -> String {
    self.msg.clone()
  }
}

impl From<serde_json::Error> for BenchError {
  fn from(e: serde_json::Error) -> Self {
    Self {
      kind: BenchErrorKind::JsonError,
      msg: e.to_string(),
    }
  }
}

impl From<io::Error> for BenchError {
  fn from(e: io::Error) -> Self {
    Self {
      kind: BenchErrorKind::IoError,
      msg: e.to_string(),
    }
  }
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

  let mut isolate = Isolate::new();

  isolate
    .core_isolate
    .execute("dispatch_json", include_str!("../dispatch_json.js"))
    .expect("Failed to execute dispatch_json");
  isolate
    .core_isolate
    .execute("http_bench_json", include_str!("http_bench_json.js"))
    .expect("Failed to execute http_bench_json");

  let mut runtime = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  runtime.block_on(isolate).expect("unexpected isolate error");
}
