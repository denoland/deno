// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::resolve_addr::resolve_addr;
use crate::state::ThreadSafeState;
use deno::Resource;
use deno::*;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use std;
use std::convert::From;
use std::net::Shutdown;
use std::net::SocketAddr;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("accept", s.core_op(json_op(s.stateful_op(op_accept))));
  i.register_op("dial", s.core_op(json_op(s.stateful_op(op_dial))));
  i.register_op("shutdown", s.core_op(json_op(s.stateful_op(op_shutdown))));
  i.register_op("listen", s.core_op(json_op(s.stateful_op(op_listen))));
}

/// Simply accepts a connection.
pub async fn accept(
  state: &ThreadSafeState,
  rid: ResourceId,
) -> Result<(TcpStream, SocketAddr), ErrBox> {
  let mut table = state.lock_resource_table_async().await;
  let listener_resource =
    table.get_mut::<TcpListenerResource>(rid).ok_or_else(|| {
      let e = std::io::Error::new(
        std::io::ErrorKind::Other,
        "Listener has been closed",
      );
      ErrBox::from(e)
    })?;

  let mut incoming = listener_resource.listener.incoming();
  match incoming.next().await {
    Some(Ok(stream)) => {
      let addr = stream.peer_addr().unwrap();
      Ok((stream, addr))
    }
    Some(Err(e)) => Err(e.into()),
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
}

fn op_accept(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let state_ = state.clone();

  let op = async move {
    let table = state_.lock_resource_table_async().await;
    table
      .get::<TcpListenerResource>(rid)
      .ok_or_else(bad_resource)?;
    let (tcp_stream, _socket_addr) = accept(&state_, rid).await?;
    let local_addr = match tcp_stream.local_addr() {
      Ok(v) => v,
      Err(e) => return Err(ErrBox::from(e)),
    };
    let remote_addr = match tcp_stream.peer_addr() {
      Ok(v) => v,
      Err(e) => return Err(ErrBox::from(e)),
    };
    let mut table = state_.lock_resource_table_async().await;
    let rid =
      table.add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
    Ok(json!({
      "rid": rid,
      "localAddr": local_addr.to_string(),
      "remoteAddr": remote_addr.to_string(),
    }))
  };

  Ok(JsonOp::Async(op.boxed()))
}

#[derive(Deserialize)]
struct DialArgs {
  transport: String,
  hostname: String,
  port: u16,
}

fn op_dial(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: DialArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp"); // TODO Support others.
  let state_ = state.clone();
  state.check_net(&args.hostname, args.port)?;

  let op = Box::pin(async move {
    let addr = resolve_addr(&args.hostname, args.port).await?;
    let tcp_stream = TcpStream::connect(&addr).await?;
    let local_addr = match tcp_stream.local_addr() {
      Ok(v) => v,
      Err(e) => return Err(ErrBox::from(e)),
    };
    let remote_addr = match tcp_stream.peer_addr() {
      Ok(v) => v,
      Err(e) => return Err(ErrBox::from(e)),
    };
    let mut table = state_.lock_resource_table_async().await;
    let rid =
      table.add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
    Ok(json!({
      "rid": rid,
      "localAddr": local_addr.to_string(),
      "remoteAddr": remote_addr.to_string(),
    }))
  });

  Ok(JsonOp::Async(op))
}

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

fn op_shutdown(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let shutdown_mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  let mut table = state.lock_resource_table();
  let resource = table
    .get_mut::<StreamResource>(rid)
    .ok_or_else(bad_resource)?;
  match resource {
    StreamResource::TcpStream(ref mut stream) => {
      TcpStream::shutdown(stream, shutdown_mode).map_err(ErrBox::from)?;
    }
    _ => return Err(bad_resource()),
  }

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct ListenArgs {
  transport: String,
  hostname: String,
  port: u16,
}

#[allow(dead_code)]
struct TcpListenerResource {
  listener: TcpListener,
  local_addr: SocketAddr,
}

impl Resource for TcpListenerResource {}

fn op_listen(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  state.check_net(&args.hostname, args.port)?;
  let (rid, local_addr_str) = futures::executor::block_on(async {
    let addr = resolve_addr(&args.hostname, args.port).await?;
    let listener = TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;
    let local_addr_str = local_addr.to_string();
    let listener_resource = TcpListenerResource {
      listener,
      local_addr,
    };
    let mut table = state.lock_resource_table_async().await;
    let rid = table.add("tcpListener", Box::new(listener_resource));
    Ok::<_, ErrBox>((rid, local_addr_str))
  })?;
  debug!("New listener {} {}", rid, local_addr_str);

  Ok(JsonOp::Sync(json!({
    "rid": rid,
    "localAddr": local_addr_str,
  })))
}
