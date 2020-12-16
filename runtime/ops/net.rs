// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ops::io::TcpStreamResource;
use crate::permissions::Permissions;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use deno_core::error::bad_resource;
use deno_core::error::bad_resource_id;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::rc::Rc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;

#[cfg(unix)]
use super::net_unix;
#[cfg(unix)]
use crate::ops::io::StreamResource;
#[cfg(unix)]
use std::path::Path;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_async(rt, "op_accept", op_accept);
  super::reg_json_async(rt, "op_connect", op_connect);
  super::reg_json_async(rt, "op_shutdown", op_shutdown);
  super::reg_json_sync(rt, "op_listen", op_listen);
  super::reg_json_async(rt, "op_datagram_receive", op_datagram_receive);
  super::reg_json_async(rt, "op_datagram_send", op_datagram_send);
}

#[derive(Deserialize)]
pub(crate) struct AcceptArgs {
  pub rid: i32,
  pub transport: String,
}

async fn accept_tcp(
  state: Rc<RefCell<OpState>>,
  args: AcceptArgs,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let rid = args.rid as u32;

  let resource = state
    .borrow()
    .resource_table
    .get::<TcpListenerResource>(rid)
    .ok_or_else(|| bad_resource("Listener has been closed"))?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| custom_error("Busy", "Another accept task is ongoing"))?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (tcp_stream, _socket_addr) =
    listener.accept().try_or_cancel(cancel).await.map_err(|e| {
      // FIXME(bartlomieju): compatibility with current JS implementation
      if let std::io::ErrorKind::Interrupted = e.kind() {
        bad_resource("Listener has been closed")
      } else {
        e.into()
      }
    })?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut state = state.borrow_mut();
  let rid = state
    .resource_table
    .add(TcpStreamResource::new(tcp_stream.into_split()));
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
  state: Rc<RefCell<OpState>>,
  args: Value,
  bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: AcceptArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "tcp" => accept_tcp(state, args, bufs).await,
    #[cfg(unix)]
    "unix" => net_unix::accept_unix(state, args, bufs).await,
    _ => Err(generic_error(format!(
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
  state: Rc<RefCell<OpState>>,
  args: ReceiveArgs,
  zero_copy: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let mut zero_copy = zero_copy[0].clone();

  let rid = args.rid as u32;

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .ok_or_else(|| bad_resource("Socket has been closed"))?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;
  let cancel_handle = RcRef::map(&resource, |r| &r.cancel);
  let (size, remote_addr) = socket
    .recv_from(&mut zero_copy)
    .try_or_cancel(cancel_handle)
    .await?;
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
  state: Rc<RefCell<OpState>>,
  args: Value,
  zero_copy: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");

  let args: ReceiveArgs = serde_json::from_value(args)?;
  match args.transport.as_str() {
    "udp" => receive_udp(state, args, zero_copy).await,
    #[cfg(unix)]
    "unixpacket" => net_unix::receive_unix_packet(state, args, zero_copy).await,
    _ => Err(generic_error(format!(
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
  state: Rc<RefCell<OpState>>,
  args: Value,
  zero_copy: BufVec,
) -> Result<Value, AnyError> {
  assert_eq!(zero_copy.len(), 1, "Invalid number of arguments");
  let zero_copy = zero_copy[0].clone();

  match serde_json::from_value(args)? {
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "udp" => {
      {
        let s = state.borrow();
        s.borrow::<Permissions>()
          .check_net(&args.hostname, args.port)?;
      }
      let addr = resolve_addr(&args.hostname, args.port).await?;

      let resource = state
        .borrow_mut()
        .resource_table
        .get::<UdpSocketResource>(rid as u32)
        .ok_or_else(|| bad_resource("Socket has been closed"))?;
      let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;
      let byte_length = socket.send_to(&zero_copy, &addr).await?;
      Ok(json!(byte_length))
    }
    #[cfg(unix)]
    SendArgs {
      rid,
      transport,
      transport_args: ArgsEnum::Unix(args),
    } if transport == "unixpacket" => {
      let address_path = Path::new(&args.path);
      {
        let s = state.borrow();
        s.borrow::<Permissions>().check_write(&address_path)?;
      }
      let resource = state
        .borrow()
        .resource_table
        .get::<net_unix::UnixDatagramResource>(rid as u32)
        .ok_or_else(|| {
          custom_error("NotConnected", "Socket has been closed")
        })?;
      let socket = RcRef::map(&resource, |r| &r.socket)
        .try_borrow_mut()
        .ok_or_else(|| custom_error("Busy", "Socket already in use"))?;
      let byte_length = socket.send_to(&zero_copy, address_path).await?;
      Ok(json!(byte_length))
    }
    _ => Err(type_error("Wrong argument format!")),
  }
}

#[derive(Deserialize)]
struct ConnectArgs {
  transport: String,
  #[serde(flatten)]
  transport_args: ArgsEnum,
}

async fn op_connect(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  match serde_json::from_value(args)? {
    ConnectArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } if transport == "tcp" => {
      {
        let state_ = state.borrow();
        state_
          .borrow::<Permissions>()
          .check_net(&args.hostname, args.port)?;
      }
      let addr = resolve_addr(&args.hostname, args.port).await?;
      let tcp_stream = TcpStream::connect(&addr).await?;
      let local_addr = tcp_stream.local_addr()?;
      let remote_addr = tcp_stream.peer_addr()?;

      let mut state_ = state.borrow_mut();
      let rid = state_
        .resource_table
        .add(TcpStreamResource::new(tcp_stream.into_split()));
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
      let address_path = Path::new(&args.path);
      super::check_unstable2(&state, "Deno.connect");
      {
        let state_ = state.borrow();
        state_.borrow::<Permissions>().check_read(&address_path)?;
        state_.borrow::<Permissions>().check_write(&address_path)?;
      }
      let path = args.path;
      let unix_stream = net_unix::UnixStream::connect(Path::new(&path)).await?;
      let local_addr = unix_stream.local_addr()?;
      let remote_addr = unix_stream.peer_addr()?;

      let mut state_ = state.borrow_mut();
      let resource = StreamResource::unix_stream(unix_stream);
      let rid = state_.resource_table.add(resource);
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
    _ => Err(type_error("Wrong argument format!")),
  }
}

#[derive(Deserialize)]
struct ShutdownArgs {
  rid: i32,
  how: i32,
}

async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  super::check_unstable2(&state, "Deno.shutdown");

  let args: ShutdownArgs = serde_json::from_value(args)?;

  let rid = args.rid as u32;
  let how = args.how;

  let shutdown_mode = match how {
    0 => Shutdown::Read, // TODO: nonsense, remove me.
    1 => Shutdown::Write,
    _ => unimplemented!(),
  };

  let resource = state
    .borrow()
    .resource_table
    .get_any(rid)
    .ok_or_else(bad_resource_id)?;
  if let Some(stream) = resource.downcast_rc::<TcpStreamResource>() {
    let wr = stream.wr_borrow_mut().await;
    TcpStream::shutdown((*wr).as_ref(), shutdown_mode)?;
    return Ok(json!({}));
  }

  #[cfg(unix)]
  if let Some(stream) = resource.downcast_rc::<StreamResource>() {
    if stream.unix_stream.is_some() {
      let wr = RcRef::map(stream, |r| r.unix_stream.as_ref().unwrap())
        .borrow_mut()
        .await;
      net_unix::UnixStream::shutdown(&*wr, shutdown_mode)?;
      return Ok(json!({}));
    }
  }

  Err(bad_resource_id())
}

struct TcpListenerResource {
  listener: AsyncRefCell<TcpListener>,
  cancel: CancelHandle,
}

impl Resource for TcpListenerResource {
  fn name(&self) -> Cow<str> {
    "tcpListener".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

struct UdpSocketResource {
  socket: AsyncRefCell<UdpSocket>,
  cancel: CancelHandle,
}

impl Resource for UdpSocketResource {
  fn name(&self) -> Cow<str> {
    "udpSocket".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
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
  state: &mut OpState,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), AnyError> {
  let std_listener = std::net::TcpListener::bind(&addr)?;
  let listener = TcpListener::from_std(std_listener)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = TcpListenerResource {
    listener: AsyncRefCell::new(listener),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(listener_resource);

  Ok((rid, local_addr))
}

fn listen_udp(
  state: &mut OpState,
  addr: SocketAddr,
) -> Result<(u32, SocketAddr), AnyError> {
  let std_socket = std::net::UdpSocket::bind(&addr)?;
  let socket = UdpSocket::from_std(std_socket)?;
  let local_addr = socket.local_addr()?;
  let socket_resource = UdpSocketResource {
    socket: AsyncRefCell::new(socket),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(socket_resource);

  Ok((rid, local_addr))
}

fn op_listen(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let permissions = state.borrow::<Permissions>();
  match serde_json::from_value(args)? {
    ListenArgs {
      transport,
      transport_args: ArgsEnum::Ip(args),
    } => {
      {
        if transport == "udp" {
          super::check_unstable(state, "Deno.listenDatagram");
        }
        permissions.check_net(&args.hostname, args.port)?;
      }
      let addr = resolve_addr_sync(&args.hostname, args.port)?;
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
      let address_path = Path::new(&args.path);
      {
        if transport == "unix" {
          super::check_unstable(state, "Deno.listen");
        }
        if transport == "unixpacket" {
          super::check_unstable(state, "Deno.listenDatagram");
        }
        permissions.check_read(&address_path)?;
        permissions.check_write(&address_path)?;
      }
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
    _ => Err(type_error("Wrong argument format!")),
  }
}
