#[macro_use]
extern crate log;

use deno_core::serde_json;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::Op;
use deno_core::OpFn;
use deno_core::OpId;
use deno_core::OpRegistry;
use deno_core::OpRouter;
use deno_core::ResourceTable;
use deno_core::Script;
use deno_core::StartupData;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::Future;
use indexmap::IndexMap;
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

#[derive(Default)]
struct State {
  resource_table: RefCell<ResourceTable>,
  op_registry: RefCell<IndexMap<String, Rc<OpFn<Self>>>>,
}

impl State {
  fn new() -> Rc<Self> {
    let s = Rc::new(Self::default());
    s.register_op_json_catalog(Self::op_catalog);
    s.register_op_json_sync("listen", Self::op_listen);
    s.register_op_json_sync("close", Self::op_close);
    s.register_op_json_async("accept", Self::op_accept);
    s.register_op_json_async("read", Self::op_read);
    s.register_op_json_async("write", Self::op_write);
    s
  }

  fn op_catalog(state: &State, visitor: &mut dyn FnMut((String, OpId))) {
    state
      .op_registry
      .borrow()
      .keys()
      .cloned()
      .zip(0..)
      .for_each(visitor)
  }

  fn op_listen(
    &self,
    _args: serde_json::Value,
    _bufs: &mut [ZeroCopyBuf],
  ) -> Result<serde_json::Value, ErrBox> {
    debug!("listen");
    let addr = "127.0.0.1:4544".parse::<SocketAddr>().unwrap();
    let std_listener = std::net::TcpListener::bind(&addr)?;
    let listener = TcpListener::from_std(std_listener)?;
    let rid = self
      .resource_table
      .borrow_mut()
      .add("tcpListener", Box::new(listener));
    Ok(serde_json::json!({ "rid": rid }))
  }

  fn op_close(
    &self,
    args: serde_json::Value,
    _buf: &mut [ZeroCopyBuf],
  ) -> Result<serde_json::Value, ErrBox> {
    let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
    debug!("close rid={}", rid);

    self
      .resource_table
      .borrow_mut()
      .close(rid)
      .map(|_| serde_json::json!(()))
      .ok_or_else(ErrBox::bad_resource_id)
  }

  fn op_accept(
    self: Rc<Self>,
    args: serde_json::Value,
    _bufs: BufVec,
  ) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
    let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
    debug!("accept rid={}", rid);

    poll_fn(move |cx| {
      let resource_table = &mut self.resource_table.borrow_mut();
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
    self: Rc<Self>,
    args: serde_json::Value,
    mut bufs: BufVec,
  ) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
    assert_eq!(bufs.len(), 1, "Invalid number of arguments");

    let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
    debug!("read rid={}", rid);

    poll_fn(move |cx| -> Poll<Result<serde_json::Value, ErrBox>> {
      let resource_table = &mut self.resource_table.borrow_mut();
      let stream = resource_table
        .get_mut::<TcpStream>(rid)
        .ok_or_else(ErrBox::bad_resource_id)?;
      Pin::new(stream)
        .poll_read(cx, &mut bufs[0])?
        .map(|nread| Ok(serde_json::json!({ "nread": nread })))
    })
  }

  fn op_write(
    self: Rc<Self>,
    args: serde_json::Value,
    bufs: BufVec,
  ) -> impl Future<Output = Result<serde_json::Value, ErrBox>> {
    assert_eq!(bufs.len(), 1, "Invalid number of arguments");

    let rid = args.get("rid").unwrap().as_u64().unwrap() as u32;
    debug!("write rid={}", rid);

    poll_fn(move |cx| {
      let resource_table = &mut self.resource_table.borrow_mut();
      let stream = resource_table
        .get_mut::<TcpStream>(rid)
        .ok_or_else(ErrBox::bad_resource_id)?;
      Pin::new(stream)
        .poll_write(cx, &bufs[0])?
        .map(|nwritten| Ok(serde_json::json!({ "nwritten": nwritten })))
    })
  }
}

impl OpRegistry for State {
  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static,
  {
    let mut op_registry = self.op_registry.borrow_mut();
    let (op_id, removed_op_fn) =
      op_registry.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(removed_op_fn.is_none());
    op_id.try_into().unwrap()
  }
}

impl OpRouter for State {
  fn route_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op {
    let index = op_id.try_into().unwrap();
    let op_fn = self
      .op_registry
      .borrow()
      .get_index(index)
      .map(|(_, op_fn)| op_fn.clone())
      .unwrap();
    (op_fn)(self, bufs)
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

  let startup_data = StartupData::Script(Script {
    source: include_str!("http_bench_json_ops.js"),
    filename: "http_bench_json_ops.js",
  });
  let isolate = CoreIsolate::new(State::new(), startup_data, false);

  let mut runtime = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .build()
    .unwrap();
  deno_core::js_check(runtime.block_on(isolate));
}
