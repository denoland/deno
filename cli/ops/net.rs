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
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use futures::stream::TryStreamExt;
use std;
use std::convert::From;
use std::future::Future;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::tcp::Incoming;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("accept", s.core_op(json_op(s.stateful_op(op_accept))));
  i.register_op("dial", s.core_op(json_op(s.stateful_op(op_dial))));
  i.register_op("shutdown", s.core_op(json_op(s.stateful_op(op_shutdown))));
  i.register_op("listen", s.core_op(json_op(s.stateful_op(op_listen))));
}

#[derive(Debug, PartialEq)]
enum AcceptState {
  Eager,
  Pending,
  Done,
}

/// Simply accepts a connection.
pub fn accept(state: &ThreadSafeState, rid: ResourceId) -> Accept {
  Accept {
    accept_state: AcceptState::Eager,
    rid,
    state: state.clone(),
  }
}

/// A future representing state of accepting a TCP connection.
pub struct Accept {
  accept_state: AcceptState,
  rid: ResourceId,
  state: ThreadSafeState,
}

impl Future for Accept {
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

    let mut listener =
      futures::compat::Compat01As03::new(&mut listener_resource.listener)
        .map_err(ErrBox::from);

    if inner.accept_state == AcceptState::Eager {
      // Similar to try_ready!, but also track/untrack accept task
      // in TcpListener resource.
      // In this way, when the listener is closed, the task can be
      // notified to error out (instead of stuck forever).
      match listener.poll_next_unpin(cx) {
        Poll::Ready(Some(Ok(stream))) => {
          inner.accept_state = AcceptState::Done;
          let addr = stream.peer_addr().unwrap();
          return Poll::Ready(Ok((stream, addr)));
        }
        Poll::Pending => {
          inner.accept_state = AcceptState::Pending;
          return Poll::Pending;
        }
        Poll::Ready(Some(Err(e))) => {
          inner.accept_state = AcceptState::Done;
          return Poll::Ready(Err(e));
        }
        _ => unreachable!(),
      }
    }

    match listener.poll_next_unpin(cx) {
      Poll::Ready(Some(Ok(stream))) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptState::Done;
        let addr = stream.peer_addr().unwrap();
        Poll::Ready(Ok((stream, addr)))
      }
      Poll::Pending => {
        listener_resource.track_task(cx)?;
        Poll::Pending
      }
      Poll::Ready(Some(Err(e))) => {
        listener_resource.untrack_task();
        inner.accept_state = AcceptState::Done;
        Poll::Ready(Err(e))
      }
      _ => unreachable!(),
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

  let op = accept(state, rid)
    .and_then(move |(tcp_stream, _socket_addr)| {
      let local_addr = match tcp_stream.local_addr() {
        Ok(v) => v,
        Err(e) => return futures::future::err(ErrBox::from(e)),
      };
      let remote_addr = match tcp_stream.peer_addr() {
        Ok(v) => v,
        Err(e) => return futures::future::err(ErrBox::from(e)),
      };
      let mut table = state_.lock_resource_table();
      let rid =
        table.add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
      futures::future::ok((rid, local_addr, remote_addr))
    })
    .map_err(ErrBox::from)
    .and_then(move |(rid, local_addr, remote_addr)| {
      futures::future::ok(json!({
        "rid": rid,
        "localAddr": local_addr.to_string(),
        "remoteAddr": remote_addr.to_string(),
      }))
    });

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

  let op = resolve_addr(&args.hostname, args.port).and_then(move |addr| {
    futures::compat::Compat01As03::new(TcpStream::connect(&addr))
      .map_err(ErrBox::from)
      .and_then(move |tcp_stream| {
        let local_addr = match tcp_stream.local_addr() {
          Ok(v) => v,
          Err(e) => return futures::future::err(ErrBox::from(e)),
        };
        let remote_addr = match tcp_stream.peer_addr() {
          Ok(v) => v,
          Err(e) => return futures::future::err(ErrBox::from(e)),
        };
        let mut table = state_.lock_resource_table();
        let rid = table
          .add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
        futures::future::ok((rid, local_addr, remote_addr))
      })
      .map_err(ErrBox::from)
      .and_then(move |(rid, local_addr, remote_addr)| {
        futures::future::ok(json!({
          "rid": rid,
          "localAddr": local_addr.to_string(),
          "remoteAddr": remote_addr.to_string(),
        }))
      })
  });

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
  listener: Incoming,
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
  let listener = TcpListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let local_addr_str = local_addr.to_string();
  let listener_resource = TcpListenerResource {
    listener: listener.incoming(),
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
