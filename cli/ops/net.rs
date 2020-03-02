// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::op_error::OpError;
use crate::resolve_addr::resolve_addr;
use crate::state::State;
use deno_core::*;
use futures::future::poll_fn;
use futures::future::FutureExt;
use std;
use std::convert::From;
use std::fs::remove_file;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::path::Path;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;

#[cfg(unix)]
use std::os::unix;
#[cfg(unix)]
use tokio::net::UnixDatagram;
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio::net::UnixStream;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_accept", s.stateful_json_op(op_accept));
  i.register_op("op_connect", s.stateful_json_op(op_connect));
  i.register_op("op_shutdown", s.stateful_json_op(op_shutdown));
  i.register_op("op_listen", s.stateful_json_op(op_listen));
  i.register_op("op_receive", s.stateful_json_op(op_receive));
  i.register_op("op_send", s.stateful_json_op(op_send));
}

#[derive(Deserialize)]
struct AcceptArgs {
  rid: i32,
  transport: String,
}

fn accept_tcp(
  state: &State,
  args: AcceptArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let rid = args.rid as u32;
  let state_ = state.clone();
  {
    let state = state.borrow();
    state
      .resource_table
      .get::<TcpListenerResource>(rid)
      .ok_or_else(OpError::bad_resource)?;
  }

  let state = state.clone();

  let op = async move {
    let accept_fut = poll_fn(|cx| {
      let resource_table = &mut state.borrow_mut().resource_table;
      let listener_resource = resource_table
        .get_mut::<TcpListenerResource>(rid)
        .ok_or_else(|| {
          OpError::other("Listener has been closed".to_string())
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
    let mut state = state_.borrow_mut();
    let rid = state
      .resource_table
      .add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
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

fn accept_unix(
  state: &State,
  args: AcceptArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let rid = args.rid as u32;
  let state_ = state.clone();
  {
    let state = state.borrow();
    state
      .resource_table
      .get::<UnixListenerResource>(rid)
      .ok_or_else(OpError::bad_resource)?;
  }
  let op = async move {
    let mut state = state_.borrow_mut();
    let listener_resource = state
      .resource_table
      .get_mut::<UnixListenerResource>(rid)
      .ok_or_else(|| OpError::other("Listener has been closed".to_string()))?;
    let (unix_stream, _socket_addr) =
      listener_resource.listener.accept().await?;
    let local_addr = unix_stream.local_addr()?;
    let remote_addr = unix_stream.peer_addr()?;
    let rid = state.resource_table.add(
      "unixStream",
      Box::new(StreamResource::UnixStream(unix_stream)),
    );
    Ok(json!({
      "rid": rid,
      "localAddr": {
        "address": local_addr.as_pathname(),
        "transport": args.transport,
      },
      "remoteAddr": {
        "address": remote_addr.as_pathname(),
        "transport": args.transport,
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_accept(
  state: &State,
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "tcp" => accept_tcp(state, args, zero_copy),
    "unix" => accept_unix(state, args, zero_copy),
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
  state: &State,
  args: ReceiveArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let mut buf = zero_copy.unwrap();

  let rid = args.rid as u32;

  let state_ = state.clone();

  let op = async move {
    let receive_fut = poll_fn(|cx| {
      let resource_table = &mut state_.borrow_mut().resource_table;
      let resource = resource_table
        .get_mut::<UdpSocketResource>(rid)
        .ok_or_else(|| OpError::other("Socket has been closed".to_string()))?;
      let socket = &mut resource.socket;
      socket.poll_recv_from(cx, &mut buf).map_err(OpError::from)
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

fn receive_unix_packet(
  state: &State,
  args: ReceiveArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let mut buf = zero_copy.unwrap();

  let rid = args.rid as u32;

  let state_ = state.clone();

  let op = async move {
    let mut state = state_.borrow_mut();
    let resource = state
      .resource_table
      .get_mut::<UnixDatagramResource>(rid)
      .ok_or_else(|| OpError::other("Socket has been closed".to_string()))?;
    let (size, remote_addr) = resource.socket.recv_from(&mut buf).await?;
    Ok(json!({
      "size": size,
      "remoteAddr": {
        "address": remote_addr.as_pathname(),
        "transport": args.transport,
      }
    }))
  };

  Ok(JsonOp::Async(op.boxed_local()))
}

fn op_receive(
  state: &State,
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  assert!(zero_copy.is_some());
  let args: ReceiveArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "udp" => receive_udp(state, args, zero_copy),
    "unixpacket" => receive_unix_packet(state, args, zero_copy),
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
  state: &State,
  args: Value,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  assert!(zero_copy.is_some());
  let buf = zero_copy.unwrap();
  let state_ = state.clone();
  match serde_json::from_value(args)? {
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "udp" => {
      state.check_net(&args.hostname, args.port)?;

      let op = async move {
        let mut state = state_.borrow_mut();
        let resource = state
          .resource_table
          .get_mut::<UdpSocketResource>(rid as u32)
          .ok_or_else(|| {
            OpError::other("Socket has been closed".to_string())
          })?;

        let socket = &mut resource.socket;
        let addr = resolve_addr(&args.hostname, args.port).await?;
        socket.send_to(&buf, addr).await?;

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
      let address_path = Path::new(&args.address);
      state.check_read(&address_path)?;
      let op = async move {
        let mut state = state_.borrow_mut();
        let resource = state
          .resource_table
          .get_mut::<UnixDatagramResource>(rid as u32)
          .ok_or_else(|| {
            OpError::other("Socket has been closed".to_string())
          })?;

        let socket = &mut resource.socket;
        socket
          .send_to(&buf, &resource.local_addr.as_pathname().unwrap())
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  match serde_json::from_value(args)? {
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "tcp" => {
      let state_ = state.clone();
      state.check_net(&args.hostname, args.port)?;
      let op = async move {
        let addr = resolve_addr(&args.hostname, args.port).await?;
        let tcp_stream = TcpStream::connect(&addr).await?;
        let local_addr = tcp_stream.local_addr()?;
        let remote_addr = tcp_stream.peer_addr()?;
        let mut state = state_.borrow_mut();
        let rid = state
          .resource_table
          .add("tcpStream", Box::new(StreamResource::TcpStream(tcp_stream)));
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
      let address_path = Path::new(&args.address);
      let state_ = state.clone();
      state.check_read(&address_path)?;
      let op = async move {
        let address = args.address;
        let unix_stream = UnixStream::connect(Path::new(&address)).await?;
        let local_addr = unix_stream.local_addr()?;
        let remote_addr = unix_stream.peer_addr()?;
        let mut state = state_.borrow_mut();
        let rid = state.resource_table.add(
          "unixStream",
          Box::new(StreamResource::UnixStream(unix_stream)),
        );
        Ok(json!({
          "rid": rid,
          "localAddr": {
            "address": local_addr.as_pathname(),
            "transport": transport,
          },
          "remoteAddr": {
            "address": remote_addr.as_pathname(),
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
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let shutdown_mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  let mut state = state.borrow_mut();
  let resource = state
    .resource_table
    .get_mut::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;
  match resource {
    StreamResource::TcpStream(ref mut stream) => {
      TcpStream::shutdown(stream, shutdown_mode).map_err(OpError::from)?;
    }
    #[cfg(unix)]
    StreamResource::UnixStream(ref mut stream) => {
      UnixStream::shutdown(stream, shutdown_mode).map_err(OpError::from)?;
    }
    _ => return Err(OpError::bad_resource()),
  }

  Ok(JsonOp::Sync(json!({})))
}

#[allow(dead_code)]
#[cfg(unix)]
struct UnixListenerResource {
  listener: UnixListener,
  local_addr: unix::net::SocketAddr,
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

#[cfg(unix)]
struct UnixDatagramResource {
  socket: UnixDatagram,
  local_addr: unix::net::SocketAddr,
}

struct UdpSocketResource {
  socket: UdpSocket,
}

#[derive(Deserialize)]
struct IpListenArgs {
  hostname: String,
  port: u16,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct UnixListenArgs {
  address: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ArgsEnum {
  Ip(IpListenArgs),
  #[cfg(unix)]
  Unix(UnixListenArgs),
}

#[derive(Deserialize)]
struct ListenArgs {
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

fn listen_tcp(
  state: &State,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), OpError> {
  let mut state = state.borrow_mut();
  let listener = futures::executor::block_on(TcpListener::bind(&addr))?;
  let local_addr = listener.local_addr()?;
  let listener_resource = TcpListenerResource {
    listener,
    waker: None,
    local_addr,
  };
  let rid = state
    .resource_table
    .add("tcpListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

fn listen_udp(
  state: &State,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), OpError> {
  let mut state = state.borrow_mut();
  let socket = futures::executor::block_on(UdpSocket::bind(&addr))?;
  let local_addr = socket.local_addr()?;
  let socket_resource = UdpSocketResource { socket };
  let rid = state
    .resource_table
    .add("udpSocket", Box::new(socket_resource));

  Ok((rid, local_addr))
}

#[cfg(unix)]
fn listen_unix(
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  let mut state = state.borrow_mut();
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let listener = UnixListener::bind(&addr)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = UnixListenerResource {
    listener,
    local_addr: local_addr.clone(),
  };
  let rid = state
    .resource_table
    .add("unixListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

#[cfg(unix)]
fn listen_unix_packet(
  state: &State,
  addr: &Path,
) -> Result<(u32, unix::net::SocketAddr), OpError> {
  let mut state = state.borrow_mut();
  if addr.exists() {
    remove_file(&addr).unwrap();
  }
  let socket = UnixDatagram::bind(&addr)?;
  let local_addr = socket.local_addr()?;
  let datagram_resource = UnixDatagramResource {
    socket,
    local_addr: local_addr.clone(),
  };
  let rid = state
    .resource_table
    .add("unixDatagram", Box::new(datagram_resource));

  Ok((rid, local_addr))
}

fn op_listen(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  match serde_json::from_value(args)? {
    ListenArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } => {
      state.check_net(&args.hostname, args.port)?;
      let addr =
        futures::executor::block_on(resolve_addr(&args.hostname, args.port))?;
      let (rid, local_addr) = if transport == "tcp" {
        listen_tcp(state, addr)?
      } else {
        listen_udp(state, addr)?
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
      let address_path = Path::new(&args.address);
      state.check_read(&address_path)?;
      let (rid, local_addr) = if transport == "unix" {
        listen_unix(state, &address_path)?
      } else {
        listen_unix_packet(state, &address_path)?
      };
      debug!(
        "New listener {} {}",
        rid,
        local_addr.as_pathname().unwrap().display(),
      );
      Ok(JsonOp::Sync(json!({
      "rid": rid,
      "localAddr": {
        "address": local_addr.as_pathname(),
        "transport": transport,
      },
      })))
    }
    _ => Err(OpError::other("Wrong argument format!".to_owned())),
  }
}
