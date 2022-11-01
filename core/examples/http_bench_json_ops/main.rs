// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::op;
use deno_core::AsyncResult;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::rc::Rc;

mod polloi;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

struct TcpListener {
  inner: polloi::TcpListener,
}

impl TcpListener {
  async fn accept(self: Rc<Self>) -> Result<TcpStream, std::io::Error> {
    let stream = self.inner.accept().await?.0.into();
    Ok(stream)
  }
}

impl Resource for TcpListener {}

struct TcpStream {
  inner: polloi::TcpStream,
}

impl TcpStream {
  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, Error> {
    let nread = self.inner.read(data).await?;
    Ok(nread)
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let nwritten = self.inner.write(data).await?;
    Ok(nwritten)
  }

  fn try_write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let nwritten = self.inner.try_write(data)?;
    Ok(nwritten)
  }
}

impl Resource for TcpStream {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();
}

impl From<polloi::TcpStream> for TcpStream {
  fn from(inner: polloi::TcpStream) -> Self {
    Self { inner }
  }
}

fn create_js_runtime() -> JsRuntime {
  let ext = deno_core::Extension::builder()
    .ops(vec![
      op_listen::decl(),
      op_accept::decl(),
      op_try_write::decl(),
    ])
    .build();

  JsRuntime::new(deno_core::RuntimeOptions {
    extensions: vec![ext],
    ..Default::default()
  })
}

#[op]
fn op_listen(state: &mut OpState) -> Result<ResourceId, Error> {
  let addr = "127.0.0.1:4570".parse::<SocketAddr>().unwrap();
  let rt = state.borrow::<Rc<polloi::Runtime>>();
  let inner = polloi::TcpListener::bind(rt, addr)?;
  let listener = TcpListener { inner };
  let rid = state.resource_table.add(listener);
  Ok(rid)
}

#[op]
async fn op_accept(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ResourceId, Error> {
  let listener = state.borrow().resource_table.get::<TcpListener>(rid)?;
  let stream = listener.accept().await?;
  let rid = state.borrow_mut().resource_table.add(stream);
  Ok(rid)
}

#[op(fast)]
fn op_try_write(
  state: &mut OpState,
  rid: u32,
  value: &[u8],
) -> Result<bool, Error> {
  let stream = state.resource_table.get::<TcpStream>(rid)?;
  Ok(stream.try_write(value).is_ok())
}

fn main() {
  // NOTE: `--help` arg will display V8 help and exit
  deno_core::v8_set_flags(env::args().collect());

  let rt = polloi::Runtime::new().expect("new runtime");
  let mut js_runtime = create_js_runtime();
  {
    let state = js_runtime.op_state();
    state.borrow_mut().put(rt.clone());
  }
  rt.block_on(async move {
    js_runtime
      .execute_script(
        "http_bench_json_ops.js",
        include_str!("http_bench_json_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop(false).await
  })
  .unwrap();
}
