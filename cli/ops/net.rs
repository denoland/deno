// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::resources::DenoResource;
use crate::resources::ResourceId;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Async;
use futures::Future;
use futures::Poll;
use std;
use std::convert::From;
use std::mem;
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

// Since TcpListener might be closed while there is a pending accept task,
// we need to track the task so that when the listener is closed,
// this pending task could be notified and die.
// Currently TcpListener itself does not take care of this issue.
// See: https://github.com/tokio-rs/tokio/issues/846
struct ResourceTcpListener(
  tokio::net::TcpListener,
  Option<futures::task::Task>,
);

impl DenoResource for ResourceTcpListener {
  fn close(&self) {
    // If TcpListener, we must kill all pending accepts!
    if let Some(t) = &self.1 {
      // Call notify on the tracked task, so that they would error out.
      t.notify();
    }
  }

  fn inspect_repr(&self) -> &str {
    "tcpListener"
  }
}

#[derive(Debug)]
enum AcceptState {
  Eager,
  Pending,
  Empty,
}

/// Simply accepts a connection.
pub fn accept(rid: ResourceId) -> Accept {
  Accept {
    state: AcceptState::Eager,
    rid,
  }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Accept {
  state: AcceptState,
  rid: ResourceId,
}

impl Accept {
  pub fn poll_accept(
    rid: &ResourceId,
  ) -> Poll<(TcpStream, SocketAddr), ErrBox> {
    resources::with_mut_resource(rid, move |resource| {
      let resource = resource
        .downcast_mut::<ResourceTcpListener>()
        .ok_or_else(bad_resource)?;
      let stream = &mut resource.0;
      stream.poll_accept().map_err(ErrBox::from)
    })
  }

  /// Track the current task (for TcpListener resource).
  /// Throws an error if another task is already tracked.
  pub fn track_task(&self) -> Result<(), ErrBox> {
    resources::with_mut_resource(&self.rid, move |resource| {
      let resource = resource
        .downcast_mut::<ResourceTcpListener>()
        .ok_or_else(bad_resource)?;

      let t = &mut resource.1;
      // Currently, we only allow tracking a single accept task for a listener.
      // This might be changed in the future with multiple workers.
      // Caveat: TcpListener by itself also only tracks an accept task at a time.
      // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
      if t.is_some() {
        let e = std::io::Error::new(
          std::io::ErrorKind::Other,
          "Another accept task is ongoing",
        );
        return Err(ErrBox::from(e));
      }
      t.replace(futures::task::current());
      Ok(())
    })
  }

  /// Stop tracking a task (for TcpListener resource).
  /// Happens when the task is done and thus no further tracking is needed.
  pub fn untrack_task(&self) {
    let res = resources::with_mut_resource(&self.rid, move |resource| {
      let resource = resource
        .downcast_mut::<ResourceTcpListener>()
        .ok_or_else(bad_resource)?;

      // If TcpListener, we must kill all pending accepts!
      let task = &mut resource.1;
      if task.is_some() {
        task.take();
      }
      Ok(())
    });
    // TODO: why don't we return result?
    res.unwrap();
  }
}

impl Future for Accept {
  type Item = (TcpStream, SocketAddr);
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let (stream, addr) = match self.state {
      // Similar to try_ready!, but also track/untrack accept task
      // in TcpListener resource.
      // In this way, when the listener is closed, the task can be
      // notified to error out (instead of stuck forever).
      AcceptState::Eager => match Accept::poll_accept(&self.rid) {
        Ok(Async::Ready(t)) => t,
        Ok(Async::NotReady) => {
          self.state = AcceptState::Pending;
          return Ok(Async::NotReady);
        }
        Err(e) => return Err(ErrBox::from(e)),
      },
      AcceptState::Pending => match Accept::poll_accept(&self.rid) {
        Ok(Async::Ready(t)) => {
          self.untrack_task();
          t
        }
        Ok(Async::NotReady) => {
          // Would error out if another accept task is being tracked.
          self.track_task().map_err(ErrBox::from)?;
          return Ok(Async::NotReady);
        }
        Err(e) => {
          self.untrack_task();
          return Err(ErrBox::from(e));
        }
      },
      AcceptState::Empty => panic!("poll Accept after it's done"),
    };

    match mem::replace(&mut self.state, AcceptState::Empty) {
      AcceptState::Empty => panic!("invalid internal state"),
      _ => Ok((stream, addr).into()),
    }
  }
}

struct ResourceTcpStream(tokio::net::TcpStream);

impl DenoResource for ResourceTcpStream {
  fn inspect_repr(&self) -> &str {
    "tcpStream"
  }
}

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
}

fn op_accept(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  let server_rid = args.rid as u32;

  let op = accept(server_rid)
    .and_then(move |(tcp_stream, _socket_addr)| {
      let local_addr = tcp_stream.local_addr()?;
      let remote_addr = tcp_stream.peer_addr()?;
      let r = Box::new(ResourceTcpStream(tcp_stream));
      let tcp_stream_resource = resources::add_resource(r);
      Ok((tcp_stream_resource, local_addr, remote_addr))
    })
    .map_err(ErrBox::from)
    .and_then(move |(tcp_stream_resource, local_addr, remote_addr)| {
      futures::future::ok(json!({
        "rid": tcp_stream_resource.rid,
        "localAddr": local_addr.to_string(),
        "remoteAddr": remote_addr.to_string(),
      }))
    });

  Ok(JsonOp::Async(Box::new(op)))
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

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let op = resolve_addr(&address).and_then(move |addr| {
    TcpStream::connect(&addr)
      .map_err(ErrBox::from)
      .and_then(move |tcp_stream| {
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let r = Box::new(ResourceTcpStream(tcp_stream));
        let tcp_stream_resource = resources::add_resource(r);
        Ok((tcp_stream_resource, local_addr, remote_addr))
      })
      .map_err(ErrBox::from)
      .and_then(move |(tcp_stream_resource, local_addr, remote_addr)| {
        futures::future::ok(json!({
          "rid": tcp_stream_resource.rid,
          "localAddr": local_addr.to_string(),
          "remoteAddr": remote_addr.to_string(),
        }))
      })
  });

  Ok(JsonOp::Async(Box::new(op)))
}

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

fn op_shutdown(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  resources::with_resource(&rid, |resource| {
    let stream = &resource
      .downcast_ref::<ResourceTcpStream>()
      .ok_or_else(bad_resource)?;
    TcpStream::shutdown(&stream.0, mode).map_err(ErrBox::from)
  })?;

  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct ListenArgs {
  transport: String,
  hostname: String,
  port: u16,
}

fn op_listen(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ListenArgs = serde_json::from_value(args)?;
  assert_eq!(args.transport, "tcp");

  // TODO(ry) Using format! is suboptimal here. Better would be if
  // state.check_net and resolve_addr() took hostname and port directly.
  let address = format!("{}:{}", args.hostname, args.port);

  state.check_net(&address)?;

  let addr = resolve_addr(&address).wait()?;
  let listener = TcpListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let resource =
    resources::add_resource(Box::new(ResourceTcpListener(listener, None)));

  Ok(JsonOp::Sync(json!({
    "rid": resource.rid,
    "localAddr": local_addr.to_string()
  })))
}
