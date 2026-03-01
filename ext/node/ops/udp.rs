// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_permissions::PermissionsContainer;
use socket2::Domain;
use socket2::Protocol;
use socket2::Socket;
use socket2::Type;
use tokio::net::UdpSocket;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NodeUdpError {
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error("{0}")]
  AddrParse(#[from] std::net::AddrParseError),
  #[class(inherit)]
  #[error("{0}")]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error("{0}")]
  Canceled(#[from] deno_core::Canceled),
  #[class(generic)]
  #[error("No resolved address found")]
  NoResolvedAddress,
  #[class(type)]
  #[error("Invalid hostname: '{0}'")]
  InvalidHostname(String),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
}

pub struct NodeUdpSocketResource {
  pub socket: UdpSocket,
  pub cancel: CancelHandle,
}

impl Resource for NodeUdpSocketResource {
  fn name(&self) -> Cow<'_, str> {
    "nodeUdpSocket".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

#[op2]
#[serde]
pub fn op_node_udp_bind(
  state: &mut OpState,
  #[string] hostname: &str,
  #[smi] port: u16,
  reuse_address: bool,
) -> Result<(ResourceId, String, u16), NodeUdpError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_net(&(hostname, Some(port)), "dgram.createSocket()")?;

  let addr = deno_net::resolve_addr::resolve_addr_sync(hostname, port)?
    .next()
    .ok_or(NodeUdpError::NoResolvedAddress)?;

  let domain = if addr.is_ipv4() {
    Domain::IPV4
  } else {
    Domain::IPV6
  };
  let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
  if reuse_address {
    #[cfg(any(
      target_os = "windows",
      target_os = "android",
      target_os = "linux"
    ))]
    sock.set_reuse_address(true)?;
    #[cfg(all(unix, not(any(target_os = "android", target_os = "linux"))))]
    sock.set_reuse_port(true)?;
  }
  let socket_addr = socket2::SockAddr::from(addr);
  sock.bind(&socket_addr)?;
  sock.set_nonblocking(true)?;

  let std_socket: std::net::UdpSocket = sock.into();
  let socket = UdpSocket::from_std(std_socket)?;
  let local_addr = socket.local_addr()?;

  let resource = NodeUdpSocketResource {
    socket,
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);

  Ok((rid, local_addr.ip().to_string(), local_addr.port()))
}

#[op2]
pub fn op_node_udp_join_multi_v4(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[string] multi_iface: Option<String>,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv4Addr::from_str(address)?;
  let iface = multi_iface
    .as_deref()
    .map(Ipv4Addr::from_str)
    .transpose()?
    .unwrap_or(Ipv4Addr::UNSPECIFIED);

  resource.socket.join_multicast_v4(addr, iface)?;
  Ok(())
}

#[op2]
pub fn op_node_udp_leave_multi_v4(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[string] multi_iface: Option<String>,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv4Addr::from_str(address)?;
  let iface = multi_iface
    .as_deref()
    .map(Ipv4Addr::from_str)
    .transpose()?
    .unwrap_or(Ipv4Addr::UNSPECIFIED);

  resource.socket.leave_multicast_v4(addr, iface)?;
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_join_multi_v6(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[smi] multi_iface: u32,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv6Addr::from_str(address)?;
  resource.socket.join_multicast_v6(&addr, multi_iface)?;
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_leave_multi_v6(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[smi] multi_iface: u32,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv6Addr::from_str(address)?;
  resource.socket.leave_multicast_v6(&addr, multi_iface)?;
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_set_broadcast(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  on: bool,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  resource.socket.set_broadcast(on)?;
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_set_multicast_loopback(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  is_v4: bool,
  on: bool,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  if is_v4 {
    resource.socket.set_multicast_loop_v4(on)?;
  } else {
    resource.socket.set_multicast_loop_v6(on)?;
  }
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_set_multicast_ttl(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[smi] ttl: u32,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  resource.socket.set_multicast_ttl_v4(ttl)?;
  Ok(())
}

#[op2]
#[smi]
pub async fn op_node_udp_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] buf: JsBuffer,
  #[string] hostname: String,
  #[smi] port: u16,
) -> Result<usize, NodeUdpError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NodeUdpSocketResource>(rid)?;

  let addr: SocketAddr =
    deno_net::resolve_addr::resolve_addr_sync(&hostname, port)?
      .next()
      .ok_or(NodeUdpError::NoResolvedAddress)?;

  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let nwritten = resource
    .socket
    .send_to(&buf, &addr)
    .or_cancel(cancel)
    .await??;

  Ok(nwritten)
}

#[derive(serde::Serialize)]
pub struct RecvResult {
  pub nread: usize,
  pub hostname: String,
  pub port: u16,
}

#[op2]
#[serde]
pub async fn op_node_udp_recv(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] mut buf: JsBuffer,
) -> Result<RecvResult, NodeUdpError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NodeUdpSocketResource>(rid)?;

  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let (nread, remote_addr) = resource
    .socket
    .recv_from(&mut buf)
    .or_cancel(cancel)
    .await??;

  Ok(RecvResult {
    nread,
    hostname: remote_addr.ip().to_string(),
    port: remote_addr.port(),
  })
}
