// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::resolve_addr::resolve_addr;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::FutureExt;
use std::convert::From;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::task::Context;
use std::task::Poll;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;

#[cfg(unix)]
use super::net_unix;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_accept", s.stateful_json_op2(op_accept));
  i.register_op("op_connect", s.stateful_json_op2(op_connect));
  i.register_op("op_shutdown", s.stateful_json_op2(op_shutdown));
  i.register_op("op_listen", s.stateful_json_op2(op_listen));
  i.register_op("op_receive", s.stateful_json_op2(op_receive));
  i.register_op("op_send", s.stateful_json_op2(op_send));
}

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
  transport: String,
}

fn accept_tcp(
  isolate_state: &mut CoreIsolateState,
  args: AcceptArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let rid = args.rid as u32;
  let resource_table = isolate_state.resource_table.clone();

  let op = async move {
    let accept_fut = poll_fn(|cx| {
      let mut resource_table = resource_table.borrow_mut();
      let listener_resource = resource_table
        .get_mut::<TcpListenerResource>(rid)
        .ok_or_else(|| {
          OpError::bad_resource("Listener has been closed".to_string())
        })?;
      let listener = &mut listener_resource.listener;
      match listener.poll_accept(cx).map_err(OpError::from) {
        Poll::Ready(Ok((stream, addr))) => {
          listener_resource.untrack_task();
          Poll::Ready(Ok((stream, addr)))
        }
        Poll::Pending => {
          listener_resource.track_task(cx)?;
          Poll::Pending
        }
        Poll::Ready(Err(e)) => {
          listener_resource.untrack_task();
          Poll::Ready(Err(e))
        }
      }
    });
    let (tcp_stream, _socket_addr) = accept_fut.await?;
    let local_addr = tcp_stream.local_addr()?;
    let remote_addr = tcp_stream.peer_addr()?;
    let mut resource_table = resource_table.borrow_mut();
    let rid = resource_table.add(
      "tcpStream",
      Box::new(StreamResourceHolder::new(StreamResource::TcpStream(Some(
        tcp_stream,
      )))),
    );
    Ok(json!({
      "rid": rid,
      "localAddr": {
        "hostname": local_addr.ip().to_string(),
        "port": local_addr.port(),
        "transport": "tcp",
      },
      "remoteAddr": {
        "hostname": remote_addr.ip().to_string(),
        "port": remote_addr.port(),
        "transport": "tcp",
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_accept(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "tcp" => accept_tcp(isolate_state, args, zero_copy),
    #[cfg(unix)]
    "unix" => net_unix::accept_unix(isolate_state, args.rid as u32, zero_copy),
    _ => Err(OpError::other(format!(
      "Unsupported transport protocol {}",
      args.transport
    ))),
  }
}

#[derive(Deserialize)]
struct ReceiveArgs {
  rid: i32,
  transport: String,
}

fn receive_udp(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: ReceiveArgs,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let mut zero_copy = zero_copy[0].clone();

  let rid = args.rid as u32;

  let resource_table = isolate_state.resource_table.clone();

  let op = async move {
    let receive_fut = poll_fn(|cx| {
      let mut resource_table = resource_table.borrow_mut();
      let resource = resource_table
        .get_mut::<UdpSocketResource>(rid)
        .ok_or_else(|| {
          OpError::bad_resource("Socket has been closed".to_string())
        })?;
      let socket = &mut resource.socket;
      socket
        .poll_recv_from(cx, &mut zero_copy)
        .map_err(OpError::from)
    });
    let (size, remote_addr) = receive_fut.await?;
    Ok(json!({
      "size": size,
      "remoteAddr": {
        "hostname": remote_addr.ip().to_string(),
        "port": remote_addr.port(),
        "transport": "udp",
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_receive(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");

  let args: ReceiveArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "udp" => receive_udp(isolate_state, state, args, zero_copy),
    #[cfg(unix)]
    "unixpacket" => {
      net_unix::receive_unix_packet(isolate_state, args.rid as u32, zero_copy)
    }
    _ => Err(OpError::other(format!(
      "Unsupported transport protocol {}",
      args.transport
    ))),
  }
}

#[derive(Deserialize)]
struct SendArgs {
  rid: i32,
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

fn op_send(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let zero_copy = zero_copy[0].clone();

  let resource_table = isolate_state.resource_table.clone();
  match serde_json::from_value(args)? {
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "udp" => {
      state.check_net(&args.hostname, args.port)?;

      let op = async move {
        let mut resource_table = resource_table.borrow_mut();
        let resource = resource_table
          .get_mut::<UdpSocketResource>(rid as u32)
          .ok_or_else(|| {
            OpError::bad_resource("Socket has been closed".to_string())
          })?;
        let socket = &mut resource.socket;
        let addr = resolve_addr(&args.hostname, args.port)?;
        socket.send_to(&zero_copy, addr).await?;
        Ok(json!({}))
      };

      Ok(JsonOp::Async(op.boxed_local()))
    }
    #[cfg(unix)]
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unixpacket" => {
      let address_path = net_unix::Path::new(&args.path);
      state.check_read(&address_path)?;
      let op = async move {
        let mut resource_table = resource_table.borrow_mut();
        let resource = resource_table
          .get_mut::<net_unix::UnixDatagramResource>(rid as u32)
          .ok_or_else(|| {
            OpError::other("Socket has been closed".to_string())
          })?;

        let socket = &mut resource.socket;
        socket
          .send_to(&zero_copy, &resource.local_addr.as_pathname().unwrap())
          .await?;

        Ok(json!({}))
      };

      Ok(JsonOp::Async(op.boxed_local()))
    }
    _ => Err(OpError::other("Wrong argument format!".to_owned())),
  }
}

#[derive(Deserialize)]
struct ConnectArgs {
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

fn op_connect(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let resource_table = isolate_state.resource_table.clone();
  match serde_json::from_value(args)? {
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "tcp" => {
      state.check_net(&args.hostname, args.port)?;
      let op = async move {
        let addr = resolve_addr(&args.hostname, args.port)?;
        let tcp_stream = TcpStream::connect(&addr).await?;
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let mut resource_table = resource_table.borrow_mut();
        let rid = resource_table.add(
          "tcpStream",
          Box::new(StreamResourceHolder::new(StreamResource::TcpStream(Some(
            tcp_stream,
          )))),
        );
        Ok(json!({
          "rid": rid,
          "localAddr": {
            "hostname": local_addr.ip().to_string(),
            "port": local_addr.port(),
            "transport": transport,
          },
          "remoteAddr": {
            "hostname": remote_addr.ip().to_string(),
            "port": remote_addr.port(),
            "transport": transport,
          }
        }))
      };
      Ok(JsonOp::Async(op.boxed_local()))
    }
    #[cfg(unix)]
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unix" => {
      let address_path = net_unix::Path::new(&args.path);
      state.check_unstable("Deno.connect");
      state.check_read(&address_path)?;
      let op = async move {
        let path = args.path;
        let unix_stream =
          net_unix::UnixStream::connect(net_unix::Path::new(&path)).await?;
        let local_addr = unix_stream.local_addr()?;
        let remote_addr = unix_stream.peer_addr()?;
        let mut resource_table = resource_table.borrow_mut();
        let rid = resource_table.add(
          "unixStream",
          Box::new(StreamResourceHolder::new(StreamResource::UnixStream(
            unix_stream,
          ))),
        );
        Ok(json!({
          "rid": rid,
          "localAddr": {
            "path": local_addr.as_pathname(),
            "transport": transport,
          },
          "remoteAddr": {
            "path": remote_addr.as_pathname(),
            "transport": transport,
          }
        }))
      };
      Ok(JsonOp::Async(op.boxed_local()))
    }
    _ => Err(OpError::other("Wrong argument format!".to_owned())),
  }
}

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

fn op_shutdown(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.shutdown");

  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let shutdown_mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let resource_holder = resource_table
    .get_mut::<StreamResourceHolder>(rid)
    .ok_or_else(OpError::bad_resource_id)?;
  match resource_holder.resource {
    StreamResource::TcpStream(Some(ref mut stream)) => {
      TcpStream::shutdown(stream, shutdown_mode).map_err(OpError::from)?;
    }
    #[cfg(unix)]
    StreamResource::UnixStream(ref mut stream) => {
      net_unix::UnixStream::shutdown(stream, shutdown_mode)
        .map_err(OpError::from)?;
    }
    _ => return Err(OpError::bad_resource_id()),
  }

  Ok(JsonOp::Sync(json!({})))
}

#[allow(dead_code)]
struct TcpListenerResource {
  listener: TcpListener,
  waker: Option<futures::task::AtomicWaker>,
  local_addr: SocketAddr,
}

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
  pub fn track_task(&mut self, cx: &Context) -> Result<(), OpError> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      return Err(OpError::other("Another accept task is ongoing".to_string()));
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

struct UdpSocketResource {
  socket: UdpSocket,
}

#[derive(Deserialize)]
struct IpListenArgs {
  hostname: String,
  port: u16,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ArgsEnum {
  Ip(IpListenArgs),
  #[cfg(unix)]
  Unix(net_unix::UnixListenArgs),
}

#[derive(Deserialize)]
struct ListenArgs {
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

fn listen_tcp(
  resource_table: &mut ResourceTable,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), OpError> {
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = TcpListenerResource {
    listener,
    waker: None,
    local_addr,
  };
  let rid = resource_table.add("tcpListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

fn listen_udp(
  resource_table: &mut ResourceTable,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), OpError> {
  let std_socket = std::net::UdpSocket::bind(&addr)?;
  let socket = UdpSocket::from_std(std_socket)?;
  let local_addr = socket.local_addr()?;
  let socket_resource = UdpSocketResource { socket };
  let rid = resource_table.add("udpSocket", Box::new(socket_resource));

  Ok((rid, local_addr))
}

fn op_listen(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  match serde_json::from_value(args)? {
    ListenArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } => {
      if transport == "udp" {
        state.check_unstable("Deno.listenDatagram");
      }
      state.check_net(&args.hostname, args.port)?;
      let addr = resolve_addr(&args.hostname, args.port)?;
      let (rid, local_addr) = if transport == "tcp" {
        listen_tcp(&mut resource_table, addr)?
      } else {
        listen_udp(&mut resource_table, addr)?
      };
      debug!(
        "New listener {} {}:{}",
        rid,
        local_addr.ip().to_string(),
        local_addr.port()
      );
      Ok(JsonOp::Sync(json!({
      "rid": rid,
      "localAddr": {
        "hostname": local_addr.ip().to_string(),
        "port": local_addr.port(),
        "transport": transport,
      },
      })))
    }
    #[cfg(unix)]
    ListenArgs {
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unix" || transport == "unixpacket" => {
      if transport == "unix" {
        state.check_unstable("Deno.listen");
      }
      if transport == "unixpacket" {
        state.check_unstable("Deno.listenDatagram");
      }
      let address_path = net_unix::Path::new(&args.path);
      state.check_read(&address_path)?;
      state.check_write(&address_path)?;
      let (rid, local_addr) = if transport == "unix" {
        net_unix::listen_unix(&mut resource_table, &address_path)?
      } else {
        net_unix::listen_unix_packet(&mut resource_table, &address_path)?
      };
      debug!(
        "New listener {} {}",
        rid,
        local_addr.as_pathname().unwrap().display(),
      );
      Ok(JsonOp::Sync(json!({
      "rid": rid,
      "localAddr": {
        "path": local_addr.as_pathname(),
        "transport": transport,
      },
      })))
    }
    #[cfg(unix)]
    _ => Err(OpError::other("Wrong argument format!".to_owned())),
  }
}
