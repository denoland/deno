#[macro_use]
extern crate log;

use deno_core::js_check;
use deno_core::BasicState;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::Op;
use deno_core::OpRegistry;
use deno_core::Script;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::future::TryFuture;
use futures::future::TryFutureExt;
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

#[derive(Copy, Clone, Debug, PartialEq)]
struct Record {
  promise_id: u32,
  rid: u32,
  result: i32,
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

fn create_isolate() -> CoreIsolate {
  let state = BasicState::new();
  register_op_bin_sync(&state, "listen", op_listen);
  register_op_bin_sync(&state, "close", op_close);
  register_op_bin_async(&state, "accept", op_accept);
  register_op_bin_async(&state, "read", op_read);
  register_op_bin_async(&state, "write", op_write);

  let startup_data = StartupData::Script(Script {
    source: include_str!("http_bench_bin_ops.js"),
    filename: "http_bench_bin_ops.js",
  });

  CoreIsolate::new(state, startup_data, false)
}

fn op_listen(
  state: &BasicState,
  _rid: u32,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let rid = state
    .resource_table
    .borrow_mut()
    .add("tcpListener", Box::new(listener));
  Ok(rid)
}

fn op_close(
  state: &BasicState,
  rid: u32,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("close rid={}", rid);
  state
    .resource_table
    .borrow_mut()
    .close(rid)
    .map(|_| 0)
    .ok_or_else(bad_resource_id)
}

fn op_accept(
  state: Rc<BasicState>,
  rid: u32,
  _bufs: BufVec,
) -> impl TryFuture<Ok = u32, Error = Error> {
  debug!("accept rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.resource_table.borrow_mut();
    let listener = resource_table
      .get_mut::<TcpListener>(rid)
      .ok_or_else(bad_resource_id)?;
    listener.poll_accept(cx).map_ok(|(stream, _addr)| {
      resource_table.add("tcpStream", Box::new(stream))
    })
  })
}

fn op_read(
  state: Rc<BasicState>,
  rid: u32,
  bufs: BufVec,
) -> impl TryFuture<Ok = usize, Error = Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let mut buf = bufs[0].clone();

  debug!("read rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource_id)?;
    Pin::new(stream).poll_read(cx, &mut buf)
  })
}

fn op_write(
  state: Rc<BasicState>,
  rid: u32,
  bufs: BufVec,
) -> impl TryFuture<Ok = usize, Error = Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  let buf = bufs[0].clone();
  debug!("write rid={}", rid);

  poll_fn(move |cx| {
    let resource_table = &mut state.resource_table.borrow_mut();
    let stream = resource_table
      .get_mut::<TcpStream>(rid)
      .ok_or_else(bad_resource_id)?;
    Pin::new(stream).poll_write(cx, &buf)
  })
}

fn register_op_bin_sync<F>(state: &BasicState, name: &'static str, op_fn: F)
where
  F: Fn(&BasicState, u32, &mut [ZeroCopyBuf]) -> Result<u32, Error> + 'static,
{
  let base_op_fn = move |state: Rc<BasicState>, mut bufs: BufVec| -> Op {
    let record = Record::from(bufs[0].as_ref());
    let is_sync = record.promise_id == 0;
    assert!(is_sync);

    let zero_copy_bufs = &mut bufs[1..];
    let result: i32 = match op_fn(&state, record.rid, zero_copy_bufs) {
      Ok(r) => r as i32,
      Err(_) => -1,
    };
    let buf = RecordBuf::from(Record { result, ..record })[..].into();
    Op::Sync(buf)
  };

  state.register_op(name, base_op_fn);
}

fn register_op_bin_async<F, R>(state: &BasicState, name: &'static str, op_fn: F)
where
  F: Fn(Rc<BasicState>, u32, BufVec) -> R + Copy + 'static,
  R: TryFuture,
  R::Ok: TryInto<i32>,
  <R::Ok as TryInto<i32>>::Error: Debug,
{
  let base_op_fn = move |state: Rc<BasicState>, bufs: BufVec| -> Op {
    let mut bufs_iter = bufs.into_iter();
    let record_buf = bufs_iter.next().unwrap();
    let zero_copy_bufs = bufs_iter.collect::<BufVec>();

    let record = Record::from(record_buf.as_ref());
    let is_sync = record.promise_id == 0;
    assert!(!is_sync);

    let fut = async move {
      let op = op_fn(state, record.rid, zero_copy_bufs);
      let result = op
        .map_ok(|r| r.try_into().expect("op result does not fit in i32"))
        .unwrap_or_else(|_| -1)
        .await;
      RecordBuf::from(Record { result, ..record })[..].into()
    };

    Op::Async(fut.boxed_local())
  };

  state.register_op(name, base_op_fn);
}

fn bad_resource_id() -> Error {
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

  let isolate = create_isolate();
  let mut runtime = runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  js_check(runtime.block_on(isolate));
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
