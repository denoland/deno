#[macro_use]
extern crate derive_deref;
#[macro_use]
extern crate log;

use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::Op;
use deno_core::ResourceTable;
use deno_core::Script;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::prelude::*;
use futures::task::Context;
use futures::task::Poll;
use std::cell::RefCell;
use std::convert::TryInto;
use std::env;
use std::fmt::Debug;
use std::io::Error;
use std::io::ErrorKind;
use std::mem::size_of;
use std::net::SocketAddr;
use std::pin::Pin;
use std::ptr;
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

#[derive(Copy, Clone, Debug, PartialEq)]
struct Record {
  pub promise_id: u32,
  pub rid: u32,
  pub result: i32,
}

type RecordBuf = [u8; size_of::<Record>()];

impl From<&[u8]> for Record {
  fn from(buf: &[u8]) -> Self {
    assert_eq!(buf.len(), size_of::<RecordBuf>());
    unsafe { *(buf as *const _ as *const RecordBuf) }.into()
  }
}

impl From<RecordBuf> for Record {
  fn from(buf: RecordBuf) -> Self {
    unsafe {
      #[allow(clippy::cast_ptr_alignment)]
      ptr::read_unaligned(&buf as *const _ as *const Self)
    }
  }
}

impl From<Record> for RecordBuf {
  fn from(record: Record) -> Self {
    unsafe { ptr::read(&record as *const _ as *const Self) }
  }
}

struct Isolate {
  core_isolate: CoreIsolate,
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
    let startup_data = StartupData::Script(Script {
      source: include_str!("http_bench.js"),
      filename: "http_bench.js",
    });

    let mut isolate = Self {
      core_isolate: CoreIsolate::new(startup_data, false),
      state: Default::default(),
    };

    isolate.register_sync_op("listen", op_listen);
    isolate.register_op("accept", op_accept);
    isolate.register_op("read", op_read);
    isolate.register_op("write", op_write);
    isolate.register_sync_op("close", op_close);

    isolate
  }

  fn register_sync_op<F>(&mut self, name: &'static str, handler: F)
  where
    F: 'static + Fn(State, u32, &mut [ZeroCopyBuf]) -> Result<u32, Error>,
  {
    let state = self.state.clone();
    let core_handler = move |_isolate_state: &mut CoreIsolateState,
                             control_buf: &[u8],
                             zero_copy_bufs: &mut [ZeroCopyBuf]|
          -> Op {
      let state = state.clone();
      let record = Record::from(control_buf);
      let is_sync = record.promise_id == 0;
      assert!(is_sync);

      let result: i32 = match handler(state, record.rid, zero_copy_bufs) {
        Ok(r) => r as i32,
        Err(_) => -1,
      };
      let buf = RecordBuf::from(Record { result, ..record })[..].into();
      Op::Sync(buf)
    };

    self.core_isolate.register_op(name, core_handler);
  }

  fn register_op<F>(
    &mut self,
    name: &'static str,
    handler: impl Fn(State, u32, &mut [ZeroCopyBuf]) -> F + Copy + 'static,
  ) where
    F: TryFuture,
    F::Ok: TryInto<i32>,
    <F::Ok as TryInto<i32>>::Error: Debug,
  {
    let state = self.state.clone();
    let core_handler = move |_isolate_state: &mut CoreIsolateState,
                             control_buf: &[u8],
                             zero_copy_bufs: &mut [ZeroCopyBuf]|
          -> Op {
      let state = state.clone();
      let record = Record::from(control_buf);
      let is_sync = record.promise_id == 0;
      assert!(!is_sync);

      let mut zero_copy = zero_copy_bufs.to_vec();
      let fut = async move {
        let op = handler(state, record.rid, &mut zero_copy);
        let result = op
          .map_ok(|r| r.try_into().expect("op result does not fit in i32"))
          .unwrap_or_else(|_| -1)
          .await;
        RecordBuf::from(Record { result, ..record })[..].into()
      };

      Op::Async(fut.boxed_local())
    };

    self.core_isolate.register_op(name, core_handler);
  }
}

impl Future for Isolate {
  type Output = <CoreIsolate as Future>::Output;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    self.core_isolate.poll_unpin(cx)
  }
}

fn op_close(
  state: State,
  rid: u32,
  _buf: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("close rid={}", rid);
  let resource_table = &mut state.borrow_mut().resource_table;
  resource_table
    .close(rid)
    .map(|_| 0)
    .ok_or_else(bad_resource)
}

fn op_listen(
  state: State,
  _rid: u32,
  _buf: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let resource_table = &mut state.borrow_mut().resource_table;
  let rid = resource_table.add("tcpListener", Box::new(listener));
  Ok(rid)
}

fn op_accept(
  state: State,
  rid: u32,
  _buf: &mut [ZeroCopyBuf],
) -> impl TryFuture<Ok = u32, Error = Error> {
  debug!("accept rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(bad_resource)?;
    listener.poll_accept(cx).map_ok(|(stream, _addr)| {
      resource_table.add("tcpStream", Box::new(stream))
    })
  })
}

fn op_read(
  state: State,
  rid: u32,
  bufs: &mut [ZeroCopyBuf],
) -> impl TryFuture<Ok = usize, Error = Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let mut buf = bufs[0].clone();

  debug!("read rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource)?;
    Pin::new(stream).poll_read(cx, &mut buf)
  })
}

fn op_write(
  state: State,
  rid: u32,
  bufs: &mut [ZeroCopyBuf],
) -> impl TryFuture<Ok = usize, Error = Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let buf = bufs[0].clone();
  debug!("write rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource)?;
    Pin::new(stream).poll_write(cx, &buf)
  })
}

fn bad_resource() -> Error {
  Error::new(ErrorKind::NotFound, "bad resource id")
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

  let isolate = Isolate::new();
  let mut runtime = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  runtime.block_on(isolate).expect("unexpected isolate error");
}

#[test]
fn test_record_from() {
  let expected = Record {
    promise_id: 1,
    rid: 3,
    result: 4,
  };
  let buf = RecordBuf::from(expected);
  if cfg!(target_endian = "little") {
    assert_eq!(buf, [1u8, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
  }
  let actual = Record::from(buf);
  assert_eq!(actual, expected);
}
