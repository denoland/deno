// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#[macro_use]
extern crate log;

use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use futures::future::TryFuture;
use futures::future::TryFutureExt;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::env;
use std::fmt::Debug;
use std::io::Error;
use std::io::ErrorKind;
use std::mem::size_of;
use std::net::SocketAddr;
use std::ptr;
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

fn create_js_runtime() -> JsRuntime {
  let mut js_runtime = JsRuntime::new(Default::default());
  register_op_bin_sync(&mut js_runtime, "listen", op_listen);
  register_op_bin_sync(&mut js_runtime, "close", op_close);
  register_op_bin_async(&mut js_runtime, "accept", op_accept);
  register_op_bin_async(&mut js_runtime, "read", op_read);
  register_op_bin_async(&mut js_runtime, "write", op_write);
  js_runtime
}

fn op_listen(
  state: &mut OpState,
  _rid: u32,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("listen");
  let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(&addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
  let rid = state.resource_table.add(listener);
  Ok(rid)
}

fn op_close(
  state: &mut OpState,
  rid: u32,
  _bufs: &mut [ZeroCopyBuf],
) -> Result<u32, Error> {
  debug!("close rid={}", rid);
  state
    .resource_table
    .close(rid)
    .map(|_| 0)
    .ok_or_else(bad_resource_id)
}

async fn op_accept(
  state: Rc<RefCell<OpState>>,
  rid: u32,
  _bufs: BufVec,
) -> Result<u32, Error> {
  debug!("accept rid={}", rid);

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
  rid: u32,
  mut bufs: BufVec,
) -> Result<usize, Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  debug!("read rid={}", rid);

  let stream = state
    .borrow()
    .resource_table
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  stream.read(&mut bufs[0]).await
}

async fn op_write(
  state: Rc<RefCell<OpState>>,
  rid: u32,
  bufs: BufVec,
) -> Result<usize, Error> {
  assert_eq!(bufs.len(), 1, "Invalid number of arguments");
  debug!("write rid={}", rid);

  let stream = state
    .borrow()
    .resource_table
    .get::<TcpStream>(rid)
    .ok_or_else(bad_resource_id)?;
  stream.write(&bufs[0]).await
}

fn register_op_bin_sync<F>(
  js_runtime: &mut JsRuntime,
  name: &'static str,
  op_fn: F,
) where
  F: Fn(&mut OpState, u32, &mut [ZeroCopyBuf]) -> Result<u32, Error> + 'static,
{
  let base_op_fn = move |state: Rc<RefCell<OpState>>, mut bufs: BufVec| -> Op {
    let record = Record::from(bufs[0].as_ref());
    let is_sync = record.promise_id == 0;
    assert!(is_sync);

    let zero_copy_bufs = &mut bufs[1..];
    let result: i32 =
      match op_fn(&mut state.borrow_mut(), record.rid, zero_copy_bufs) {
        Ok(r) => r as i32,
        Err(_) => -1,
      };
    let buf = RecordBuf::from(Record { result, ..record })[..].into();
    Op::Sync(buf)
  };

  js_runtime.register_op(name, base_op_fn);
}

fn register_op_bin_async<F, R>(
  js_runtime: &mut JsRuntime,
  name: &'static str,
  op_fn: F,
) where
  F: Fn(Rc<RefCell<OpState>>, u32, BufVec) -> R + Copy + 'static,
  R: TryFuture,
  R::Ok: TryInto<i32>,
  <R::Ok as TryInto<i32>>::Error: Debug,
{
  let base_op_fn = move |state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
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

  js_runtime.register_op(name, base_op_fn);
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

  let mut js_runtime = create_js_runtime();
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  let future = async move {
    js_runtime
      .execute(
        "http_bench_bin_ops.js",
        include_str!("http_bench_bin_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop().await
  };
  runtime.block_on(future).unwrap();
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
