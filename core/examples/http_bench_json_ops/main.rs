// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::JsBuffer;
use deno_core::JsRuntimeForSnapshot;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::env;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

// Note: a `tokio::net::TcpListener` doesn't need to be wrapped in a cell,
// because it only supports one op (`accept`) which does not require a mutable
// reference to the listener.
struct TcpListener {
  inner: tokio::net::TcpListener,
}

impl TcpListener {
  async fn accept(self: Rc<Self>) -> Result<TcpStream, std::io::Error> {
    let stream = self.inner.accept().await?.0.into();
    Ok(stream)
  }
}

impl Resource for TcpListener {
  fn close(self: Rc<Self>) {}
}

impl TryFrom<std::net::TcpListener> for TcpListener {
  type Error = std::io::Error;
  fn try_from(
    std_listener: std::net::TcpListener,
  ) -> Result<Self, Self::Error> {
    tokio::net::TcpListener::try_from(std_listener).map(|tokio_listener| Self {
      inner: tokio_listener,
    })
  }
}

struct TcpStream {
  rd: AsyncRefCell<tokio::net::tcp::OwnedReadHalf>,
  wr: AsyncRefCell<tokio::net::tcp::OwnedWriteHalf>,
}

impl TcpStream {
  async fn read(self: Rc<Self>, data: &mut [u8]) -> Result<usize, Error> {
    let mut rd = RcRef::map(&self, |r| &r.rd).borrow_mut().await;
    let nread = rd.read(data).await?;
    Ok(nread)
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let mut wr = RcRef::map(self, |r| &r.wr).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    Ok(nwritten)
  }

  fn try_write(self: Rc<Self>, data: &[u8]) -> Result<usize, Error> {
    let wr = RcRef::map(self, |r| &r.wr)
      .try_borrow_mut()
      .ok_or_else(|| Error::msg("Failed to acquire lock on TcpStream"))?;
    let nwritten = wr.try_write(data)?;
    Ok(nwritten)
  }
}

impl Resource for TcpStream {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {}
}

impl From<tokio::net::TcpStream> for TcpStream {
  fn from(s: tokio::net::TcpStream) -> Self {
    let (rd, wr) = s.into_split();
    Self {
      rd: rd.into(),
      wr: wr.into(),
    }
  }
}

fn create_js_runtime() -> JsRuntimeForSnapshot {
  let ext = deno_core::Extension::builder("my_ext")
    .ops(vec![
      op_listen::decl(),
      op_accept::decl(),
      op_try_write::decl(),
      op_read_socket::decl(),
    ])
    .build();

  JsRuntimeForSnapshot::new(
    deno_core::RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    },
    Default::default(),
  )
}

#[op]
async fn op_read_socket(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  mut data: JsBuffer,
) -> Result<u32, Error> {
  let resource = state.borrow_mut().resource_table.get::<TcpStream>(rid)?;
  let nread = resource.read(&mut data).await?;
  Ok(nread as u32)
}

#[op]
fn op_listen(state: &mut OpState) -> Result<ResourceId, Error> {
  let addr = "127.0.0.1:4570".parse::<SocketAddr>().unwrap();
  let std_listener = std::net::TcpListener::bind(addr)?;
  std_listener.set_nonblocking(true)?;
  let listener = TcpListener::try_from(std_listener)?;
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

  let mut js_runtime = create_js_runtime();
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .build()
    .unwrap();
  let future = async move {
    js_runtime
      .execute_script(
        "http_bench_json_ops.js",
        include_ascii_string!("http_bench_json_ops.js"),
      )
      .unwrap();
    js_runtime.run_event_loop(false).await
  };
  runtime.block_on(future).unwrap();
}
