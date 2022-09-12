// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use polloi::Runtime;
use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

thread_local! {
  // Ambient state for op_listen().
  static RUNTIME: RefCell<Option<Rc<Runtime>>> = RefCell::new(None);
}

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

// Note: a `polloi::net::TcpListener` doesn't need to be wrapped in a cell,
// because it only supports one op (`accept`) which does not require a mutable
// reference to the listener.
struct TcpListener {
  inner: polloi::TcpListener,
  cancel: CancelHandle,
}

impl TcpListener {
  async fn accept(self: Rc<Self>) -> Result<TcpStream, std::io::Error> {
    let cancel = RcRef::map(&self, |r| &r.cancel);
    let stream = self.inner.accept().try_or_cancel(cancel).await?.0;
    Ok(TcpStream {
      stream: AsyncRefCell::new(stream),
      cancel: CancelHandle::new(),
    })
  }
}

impl Resource for TcpListener {
  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

struct TcpStream {
  stream: AsyncRefCell<polloi::TcpStream>,
  // When a `TcpStream` resource is closed, all pending 'read' ops are
  // canceled, while 'write' ops are allowed to complete. Therefore only
  // 'read' futures are attached to this cancel handle.
  cancel: CancelHandle,
}

impl TcpStream {
  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, Error> {
    let rd = RcRef::map(&self, |r| &r.stream).borrow_mut().await;
    let cancel = RcRef::map(self, |r| &r.cancel);
    let nread = rd.read(data).try_or_cancel(cancel).await?;
    Ok(nread)
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let wr = RcRef::map(self, |r| &r.stream).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    Ok(nwritten)
  }
}

impl Resource for TcpStream {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

fn create_js_runtime() -> JsRuntime {
  let ext = deno_core::Extension::builder()
    .ops(vec![op_listen::decl(), op_accept::decl()])
    .build();

  JsRuntime::new(deno_core::RuntimeOptions {
    extensions: vec![ext],
    ..Default::default()
  })
}

#[op]
fn op_listen(state: &mut OpState) -> Result<ResourceId, Error> {
  log::debug!("listen");
  RUNTIME.with(|slot| {
    let runtime = slot.borrow();
    let runtime = runtime.as_ref().expect("no runtime");
    let addr = "127.0.0.1:4570".parse::<SocketAddr>().unwrap();
    let listener = polloi::TcpListener::bind(runtime, addr)?;
    listener.set_defer_accept(Duration::from_secs(10))?;
    let listener = TcpListener {
      inner: listener,
      cancel: CancelHandle::new(),
    };
    let rid = state.resource_table.add(listener);
    Ok(rid)
  })
}

#[op]
async fn op_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ResourceId, Error> {
  log::debug!("accept rid={}", rid);

  let listener = state.borrow().resource_table.get::<TcpListener>(rid)?;
  let stream = listener.accept().await?;
  let rid = state.borrow_mut().resource_table.add(stream);
  Ok(rid)
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
  let runtime = polloi::Runtime::new().expect("create new runtime");

  // Ambient state for op_listen().
  RUNTIME.with(|slot| slot.borrow_mut().replace(Rc::clone(&runtime)));

  let future = async move {
    js_runtime
      .execute_script(
        "http_bench_json_ops.js",
        include_str!("http_bench_json_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop(false).await
  };
  runtime.block_on(future).unwrap();
}
