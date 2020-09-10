// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ops::io::{StreamResource, StreamResourceHolder};
use crate::resolve_addr::resolve_addr;
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use serde_derive::Deserialize;
use serde_json::Value;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;

#[cfg(unix)]
use super::net_unix;

pub fn init(s: &Rc<State>) {
  s.register_op_json_async("op_accept", op_accept);
  s.register_op_json_async("op_connect", op_connect);
  s.register_op_json_sync("op_shutdown", op_shutdown);
  s.register_op_json_sync("op_listen", op_listen);
  s.register_op_json_async("op_datagram_receive", op_datagram_receive);
  s.register_op_json_async("op_datagram_send", op_datagram_send);
}

#[derive(Deserialize)]
pub(crate) struct AcceptArgs {
  pub rid: i32,
  pub transport: String,
}

async fn accept_tcp(
  state: Rc<State>,
  args: AcceptArgs,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let rid = args.rid as u32;

  let accept_fut = poll_fn(|cx| {
    let mut resource_table = state.resource_table.borrow_mut();
    let listener_resource = resource_table
      .get_mut::<TcpListenerResource>(rid)
      .ok_or_else(|| ErrBox::bad_resource("Listener has been closed"))?;
    let listener = &mut listener_resource.listener;
    match listener.poll_accept(cx).map_err(ErrBox::from) {
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
  let rid = state.resource_table.borrow_mut().add(
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
}

async fn op_accept(
  state: Rc<State>,
  args: Value,
  bufs: BufVec,
) -> Result<Value, ErrBox> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "tcp" => accept_tcp(state, args, bufs).await,
    #[cfg(unix)]
    "unix" => net_unix::accept_unix(state, args, bufs).await,
    _ => Err(ErrBox::error(format!(
      "Unsupported transport protocol {}",
      args.transport
    ))),
  }
}

#[derive(Deserialize)]
pub(crate) struct ReceiveArgs {
  pub rid: i32,
  pub transport: String,
}

async fn receive_udp(
  state: Rc<State>,
  args: ReceiveArgs,
  zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let mut zero_copy = zero_copy[0].clone();

  let rid = args.rid as u32;

  let receive_fut = poll_fn(|cx| {
    let mut resource_table = state.resource_table.borrow_mut();
    let resource = resource_table
      .get_mut::<UdpSocketResource>(rid)
      .ok_or_else(|| ErrBox::bad_resource("Socket has been closed"))?;
    let socket = &mut resource.socket;
    socket
      .poll_recv_from(cx, &mut zero_copy)
      .map_err(ErrBox::from)
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
}

async fn op_datagram_receive(
  state: Rc<State>,
  args: Value,
  zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");

  let args: ReceiveArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "udp" => receive_udp(state, args, zero_copy).await,
    #[cfg(unix)]
    "unixpacket" => net_unix::receive_unix_packet(state, args, zero_copy).await,
    _ => Err(ErrBox::error(format!(
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

async fn op_datagram_send(
  state: Rc<State>,
  args: Value,
  zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let zero_copy = zero_copy[0].clone();

  match serde_json::from_value(args)? {
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "udp" => {
      state.check_net(&args.hostname, args.port)?;
      let addr = resolve_addr(&args.hostname, args.port)?;
      poll_fn(move |cx| {
        let mut resource_table = state.resource_table.borrow_mut();
        let resource = resource_table
          .get_mut::<UdpSocketResource>(rid as u32)
          .ok_or_else(|| ErrBox::bad_resource("Socket has been closed"))?;
        resource
          .socket
          .poll_send_to(cx, &zero_copy, &addr)
          .map_ok(|byte_length| json!(byte_length))
          .map_err(ErrBox::from)
      })
      .await
    }
    #[cfg(unix)]
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unixpacket" => {
      let address_path = net_unix::Path::new(&args.path);
      state.check_read(&address_path)?;
      let mut resource_table = state.resource_table.borrow_mut();
      let resource = resource_table
        .get_mut::<net_unix::UnixDatagramResource>(rid as u32)
        .ok_or_else(|| ErrBox::new("NotConnected", "Socket has been closed"))?;
      let socket = &mut resource.socket;
      let byte_length = socket
        .send_to(&zero_copy, &resource.local_addr.as_pathname().unwrap())
        .await?;

      Ok(json!(byte_length))
    }
    _ => Err(ErrBox::type_error("Wrong argument format!")),
  }
}

#[derive(Deserialize)]
struct ConnectArgs {
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

async fn op_connect(
  state: Rc<State>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  match serde_json::from_value(args)? {
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "tcp" => {
      state.check_net(&args.hostname, args.port)?;
      let addr = resolve_addr(&args.hostname, args.port)?;
      let tcp_stream = TcpStream::connect(&addr).await?;
      let local_addr = tcp_stream.local_addr()?;
      let remote_addr = tcp_stream.peer_addr()?;
      let rid = state.resource_table.borrow_mut().add(
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
    }
    #[cfg(unix)]
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unix" => {
      let address_path = net_unix::Path::new(&args.path);
      state.check_unstable("Deno.connect");
      state.check_read(&address_path)?;
      let path = args.path;
      let unix_stream =
        net_unix::UnixStream::connect(net_unix::Path::new(&path)).await?;
      let local_addr = unix_stream.local_addr()?;
      let remote_addr = unix_stream.peer_addr()?;
      let rid = state.resource_table.borrow_mut().add(
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
    }
    _ => Err(ErrBox::type_error("Wrong argument format!")),
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
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.shutdown");

  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let shutdown_mode = match how {
    0 => Shutdown::Read,
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  let mut resource_table = state.resource_table.borrow_mut();
  let resource_holder = resource_table
    .get_mut::<StreamResourceHolder>(rid)
    .ok_or_else(ErrBox::bad_resource_id)?;
  match resource_holder.resource {
    StreamResource::TcpStream(Some(ref mut stream)) => {
      TcpStream::shutdown(stream, shutdown_mode)?;
    }
    #[cfg(unix)]
    StreamResource::UnixStream(ref mut stream) => {
      net_unix::UnixStream::shutdown(stream, shutdown_mode)?;
    }
    _ => return Err(ErrBox::bad_resource_id()),
  }

  Ok(json!({}))
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
  pub fn track_task(&mut self, cx: &Context) -> Result<(), ErrBox> {
    // Currently, we only allow tracking a single accept task for a listener.
    // This might be changed in the future with multiple workers.
    // Caveat: TcpListener by itself also only tracks an accept task at a time.
    // See https://github.com/tokio-rs/tokio/issues/846#issuecomment-454208883
    if self.waker.is_some() {
      return Err(ErrBox::new("Busy", "Another accept task is ongoing"));
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
  state: &State,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), ErrBox> {
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = TcpListenerResource {
    listener,
    waker: None,
    local_addr,
  };
  let rid = state
    .resource_table
    .borrow_mut()
    .add("tcpListener", Box::new(listener_resource));

  Ok((rid, local_addr))
}

fn listen_udp(
  state: &State,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), ErrBox> {
  let std_socket = std::net::UdpSocket::bind(&addr)?;
  let socket = UdpSocket::from_std(std_socket)?;
  let local_addr = socket.local_addr()?;
  let socket_resource = UdpSocketResource { socket };
  let rid = state
    .resource_table
    .borrow_mut()
    .add("udpSocket", Box::new(socket_resource));

  Ok((rid, local_addr))
}

fn op_listen(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
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
      Ok(json!({
      "rid": rid,
      "localAddr": {
        "hostname": local_addr.ip().to_string(),
        "port": local_addr.port(),
        "transport": transport,
      },
      }))
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
        net_unix::listen_unix(state, &address_path)?
      } else {
        net_unix::listen_unix_packet(state, &address_path)?
      };
      debug!(
        "New listener {} {}",
        rid,
        local_addr.as_pathname().unwrap().display(),
      );
      Ok(json!({
      "rid": rid,
      "localAddr": {
        "path": local_addr.as_pathname(),
        "transport": transport,
      },
      }))
    }
    #[cfg(unix)]
    _ => Err(ErrBox::type_error("Wrong argument format!")),
  }
}
