// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::resolve_addr::resolve_addr;
use crate::state::ThreadSafeState;
use deno_core::Resource;
use deno_core::*;
use futures::future::FutureExt;
use std;
use std::convert::From;
use std::future::Future;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("accept", s.core_op(json_op(s.stateful_op(op_accept))));
  i.register_op("connect", s.core_op(json_op(s.stateful_op(op_connect))));
  i.register_op("shutdown", s.core_op(json_op(s.stateful_op(op_shutdown))));
  i.register_op("listen", s.core_op(json_op(s.stateful_op(op_listen))));
}

#[derive(Debug, PartialEq)]
enum AcceptState {
  Pending,
  Done,
}

/// Simply accepts a connection.
pub fn accept(state: &ThreadSafeState, rid: ResourceId) -> Accept {
  Accept {
    accept_state: AcceptState::Pending,
    rid,
    state,
  }
}

/// A future representing state of accepting a TCP connection.
pub struct Accept<'a> {
  accept_state: AcceptState,
  rid: ResourceId,
  state: &'a ThreadSafeState,
}

impl Future for Accept<'_> {
  type Output = Result<(TcpStream, SocketAddr), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    if inner.accept_state == AcceptState::Done {
      panic!("poll Accept after it's done");
    }

    let mut table = inner.state.lock_resource_table();
    let listener_resource = table
      .get_mut::<TcpListenerResource>(inner.rid)
      .ok_or_else(|| {
        let e = std::io::Error::new(
          std::io::ErrorKind::Other,
          "Listener has been closed",
        );
        ErrBox::from(e)
      })?;

    let listener = &mut listener_resource.listener;

    match listener.poll_accept(cx).map_err(ErrBox::from) {
      Poll::Ready(Ok((stream, addr))) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptState::Done;
        Poll::Ready(Ok((stream, addr)))
      }
      Poll::Pending => {
        listener_resource.track_task(cx)?;
        Poll::Pending
      }
      Poll::Ready(Err(e)) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptState::Done;
        Poll::Ready(Err(e))
      }
    }
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
  let table = state.lock_resource_table();
  table
    .get::<TcpListenerResource>(rid)
    .ok_or_else(bad_resource)?;

  let op = async move {
    let (tcp_stream, _socket_addr) = accept(&state_, rid).await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let mut table = state_.lock_resource_table();
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
struct ConnectArgs {
  transport: String,
  hostname: String,
  port: u16,
}

fn op_connect(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ConnectArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp"); // TODO Support others.
  let state_ = state.clone();
  state.check_net(&args.hostname, args.port)?;

  let op = async move {
    let addr = resolve_addr(&args.hostname, args.port).await?;
    let tcp_stream = TcpStream::connect(&addr).await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let mut table = state_.lock_resource_table();
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
  waker: Option<futures::task::AtomicWaker>,
  local_addr: SocketAddr,
}

impl Resource for TcpListenerResource {}

impl Drop for TcpListenerResource {
  fn drop(&mut self) {
    self.wake_task();
  }
}

impl TcpListenerResource {
  /// Track the current task so future awaiting for connection
  /// can be notified when listener is closed.
  ///
  /// Throws an error if another task is already tracked.
  pub fn track_task(&mut self, cx: &Context) -> Result<(), ErrBox> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      let e = std::io::Error::new(
        std::io::ErrorKind::Other,
        "Another accept task is ongoing",
      );
      return Err(ErrBox::from(e));
    }

    let waker = futures::task::AtomicWaker::new();
    waker.register(cx.waker());
    self.waker.replace(waker);
    Ok(())
  }

  /// Notifies a task when listener is closed so accept future can resolve.
  pub fn wake_task(&mut self) {
    if let Some(waker) = self.waker.as_ref() {
      waker.wake();
    }
  }

  /// Stop tracking a task.
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&mut self) {
    if self.waker.is_some() {
      self.waker.take();
    }
  }
}

fn op_listen(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  state.check_net(&args.hostname, args.port)?;

  let addr =
    futures::executor::block_on(resolve_addr(&args.hostname, args.port))?;
  let listener = futures::executor::block_on(TcpListener::bind(&addr))?;
  let local_addr = listener.local_addr()?;
  let local_addr_str = local_addr.to_string();
  let listener_resource = TcpListenerResource {
    listener,
    waker: None,
    local_addr,
  };
  let mut table = state.lock_resource_table();
  let rid = table.add("tcpListener", Box::new(listener_resource));
  debug!("New listener {} {}", rid, local_addr_str);

  Ok(JsonOp::Sync(json!({
    "rid": rid,
    "localAddr": local_addr_str,
  })))
}
