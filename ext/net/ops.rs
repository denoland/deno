// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use hickory_proto::rr::rdata::caa::Value;
use hickory_proto::rr::record_data::RData;
use hickory_proto::rr::record_type::RecordType;
use hickory_proto::ProtoError;
use hickory_proto::ProtoErrorKind;
use hickory_resolver::config::NameServerConfigGroup;
use hickory_resolver::config::ResolverConfig;
use hickory_resolver::config::ResolverOpts;
use hickory_resolver::system_conf;
use hickory_resolver::ResolveError;
use hickory_resolver::ResolveErrorKind;
use serde::Deserialize;
use serde::Serialize;
use socket2::Domain;
use socket2::Protocol;
use socket2::Socket;
use socket2::Type;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;

use crate::io::TcpStreamResource;
use crate::raw::NetworkListenerResource;
use crate::resolve_addr::resolve_addr;
use crate::resolve_addr::resolve_addr_sync;
use crate::tcp::TcpListener;
use crate::NetPermissions;

pub type Fd = u32;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TlsHandshakeInfo {
  pub alpn_protocol: Option<ByteString>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IpAddr {
  pub hostname: String,
  pub port: u16,
}

impl From<SocketAddr> for IpAddr {
  fn from(addr: SocketAddr) -> Self {
    Self {
      hostname: addr.ip().to_string(),
      port: addr.port(),
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NetError {
  #[class("BadResource")]
  #[error("Listener has been closed")]
  ListenerClosed,
  #[class("Busy")]
  #[error("Listener already in use")]
  ListenerBusy,
  #[class("BadResource")]
  #[error("Socket has been closed")]
  SocketClosed,
  #[class("NotConnected")]
  #[error("Socket has been closed")]
  SocketClosedNotConnected,
  #[class("Busy")]
  #[error("Socket already in use")]
  SocketBusy,
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[class("Busy")]
  #[error("Another accept task is ongoing")]
  AcceptTaskOngoing,
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error("{0}")]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(generic)]
  #[error("No resolved address found")]
  NoResolvedAddress,
  #[class(generic)]
  #[error("{0}")]
  AddrParse(#[from] std::net::AddrParseError),
  #[class(inherit)]
  #[error("{0}")]
  Map(crate::io::MapError),
  #[class(inherit)]
  #[error("{0}")]
  Canceled(#[from] deno_core::Canceled),
  #[class("NotFound")]
  #[error("{0}")]
  DnsNotFound(ResolveError),
  #[class("NotConnected")]
  #[error("{0}")]
  DnsNotConnected(ResolveError),
  #[class("TimedOut")]
  #[error("{0}")]
  DnsTimedOut(ResolveError),
  #[class(generic)]
  #[error("{0}")]
  Dns(#[from] ResolveError),
  #[class("NotSupported")]
  #[error("Provided record type is not supported")]
  UnsupportedRecordType,
  #[class("InvalidData")]
  #[error("File name or path {0:?} is not valid UTF-8")]
  InvalidUtf8(std::ffi::OsString),
  #[class(generic)]
  #[error("unexpected key type")]
  UnexpectedKeyType,
  #[class(type)]
  #[error("Invalid hostname: '{0}'")]
  InvalidHostname(String),
  #[class("Busy")]
  #[error("TCP stream is currently in use")]
  TcpStreamBusy,
  #[class(generic)]
  #[error("{0}")]
  Rustls(#[from] deno_tls::rustls::Error),
  #[class(inherit)]
  #[error("{0}")]
  Tls(#[from] deno_tls::TlsError),
  #[class("InvalidData")]
  #[error("Error creating TLS certificate: Deno.listenTls requires a key")]
  ListenTlsRequiresKey,
  #[class(inherit)]
  #[error("{0}")]
  RootCertStore(deno_error::JsErrorBox),
  #[class(generic)]
  #[error("{0}")]
  Reunite(tokio::net::tcp::ReuniteError),
  #[class(generic)]
  #[error("VSOCK is not supported on this platform")]
  VsockUnsupported,
}

pub(crate) fn accept_err(e: std::io::Error) -> NetError {
  if let std::io::ErrorKind::Interrupted = e.kind() {
    NetError::ListenerClosed
  } else {
    NetError::Io(e)
  }
}

#[op2(async)]
#[serde]
pub async fn op_net_accept_tcp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, IpAddr, IpAddr, Option<Fd>), NetError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<TcpListener>>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| NetError::AcceptTaskOngoing)?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (tcp_stream, _socket_addr) = listener
    .accept()
    .try_or_cancel(cancel)
    .await
    .map_err(accept_err)?;
  let mut _fd_raw: Option<Fd> = None;
  #[cfg(not(windows))]
  {
    use std::os::fd::AsFd;
    use std::os::fd::AsRawFd;
    let fd = tcp_stream.as_fd();
    _fd_raw = Some(fd.as_raw_fd() as u32);
  }
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut state = state.borrow_mut();
  let rid = state
    .resource_table
    .add(TcpStreamResource::new(tcp_stream.into_split()));
  Ok((
    rid,
    IpAddr::from(local_addr),
    IpAddr::from(remote_addr),
    _fd_raw,
  ))
}

#[op2(async)]
#[serde]
pub async fn op_net_recv_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] mut buf: JsBuffer,
) -> Result<(usize, IpAddr), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;
  let cancel_handle = RcRef::map(&resource, |r| &r.cancel);
  let (nread, remote_addr) = socket
    .recv_from(&mut buf)
    .try_or_cancel(cancel_handle)
    .await?;
  Ok((nread, IpAddr::from(remote_addr)))
}

#[op2(async, stack_trace)]
#[number]
pub async fn op_net_send_udp<NP>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[serde] addr: IpAddr,
  #[buffer] zero_copy: JsBuffer,
) -> Result<usize, NetError>
where
  NP: NetPermissions + 'static,
{
  {
    let mut s = state.borrow_mut();
    s.borrow_mut::<NP>().check_net(
      &(&addr.hostname, Some(addr.port)),
      "Deno.DatagramConn.send()",
    )?;
  }
  let addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or(NetError::NoResolvedAddress)?;

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;
  let nwritten = socket.send_to(&zero_copy, &addr).await?;

  Ok(nwritten)
}

#[op2(async)]
pub async fn op_net_join_multi_v4_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address: String,
  #[string] multi_interface: String,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  let addr = Ipv4Addr::from_str(address.as_str())?;
  let interface_addr = Ipv4Addr::from_str(multi_interface.as_str())?;

  socket.join_multicast_v4(addr, interface_addr)?;

  Ok(())
}

#[op2(async)]
pub async fn op_net_join_multi_v6_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address: String,
  #[smi] multi_interface: u32,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  let addr = Ipv6Addr::from_str(address.as_str())?;

  socket.join_multicast_v6(&addr, multi_interface)?;

  Ok(())
}

#[op2(async)]
pub async fn op_net_leave_multi_v4_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address: String,
  #[string] multi_interface: String,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  let addr = Ipv4Addr::from_str(address.as_str())?;
  let interface_addr = Ipv4Addr::from_str(multi_interface.as_str())?;

  socket.leave_multicast_v4(addr, interface_addr)?;

  Ok(())
}

#[op2(async)]
pub async fn op_net_leave_multi_v6_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] address: String,
  #[smi] multi_interface: u32,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  let addr = Ipv6Addr::from_str(address.as_str())?;

  socket.leave_multicast_v6(&addr, multi_interface)?;

  Ok(())
}

#[op2(async)]
pub async fn op_net_set_multi_loopback_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  is_v4_membership: bool,
  loopback: bool,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  if is_v4_membership {
    socket.set_multicast_loop_v4(loopback)?;
  } else {
    socket.set_multicast_loop_v6(loopback)?;
  }

  Ok(())
}

#[op2(async)]
pub async fn op_net_set_multi_ttl_udp(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[smi] ttl: u32,
) -> Result<(), NetError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<UdpSocketResource>(rid)
    .map_err(|_| NetError::SocketClosed)?;
  let socket = RcRef::map(&resource, |r| &r.socket).borrow().await;

  socket.set_multicast_ttl_v4(ttl)?;

  Ok(())
}

/// If this token is present in op_net_connect_tcp call and
/// the hostname matches with one of the resolved IPs, then
/// the permission check is performed against the original hostname.
pub struct NetPermToken {
  pub hostname: String,
  pub port: Option<u16>,
  pub resolved_ips: Vec<String>,
}

impl deno_core::GarbageCollected for NetPermToken {}

impl NetPermToken {
  /// Checks if the given address is included in the resolved IPs.
  pub fn includes(&self, addr: &str) -> bool {
    self.resolved_ips.iter().any(|ip| ip == addr)
  }
}

#[op2]
#[serde]
pub fn op_net_get_ips_from_perm_token(
  #[cppgc] token: &NetPermToken,
) -> Vec<String> {
  token.resolved_ips.clone()
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_net_connect_tcp<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] addr: IpAddr,
  #[cppgc] net_perm_token: Option<&NetPermToken>,
) -> Result<(ResourceId, IpAddr, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  op_net_connect_tcp_inner::<NP>(state, addr, net_perm_token).await
}

#[inline]
pub async fn op_net_connect_tcp_inner<NP>(
  state: Rc<RefCell<OpState>>,
  addr: IpAddr,
  net_perm_token: Option<&NetPermToken>,
) -> Result<(ResourceId, IpAddr, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  {
    let mut state_ = state.borrow_mut();
    // If token exists and the address matches to its resolved ips,
    // then we can check net permission against token.hostname, instead of addr.hostname
    let hostname_to_check = match net_perm_token {
      Some(token) if token.includes(&addr.hostname) => token.hostname.clone(),
      _ => addr.hostname.clone(),
    };
    state_
      .borrow_mut::<NP>()
      .check_net(&(&hostname_to_check, Some(addr.port)), "Deno.connect()")?;
  }

  let addr = resolve_addr(&addr.hostname, addr.port)
    .await?
    .next()
    .ok_or_else(|| NetError::NoResolvedAddress)?;
  let tcp_stream = TcpStream::connect(&addr).await?;
  let local_addr = tcp_stream.local_addr()?;
  let remote_addr = tcp_stream.peer_addr()?;

  let mut state_ = state.borrow_mut();
  let rid = state_
    .resource_table
    .add(TcpStreamResource::new(tcp_stream.into_split()));

  Ok((rid, IpAddr::from(local_addr), IpAddr::from(remote_addr)))
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

#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_tcp<NP>(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  reuse_port: bool,
  load_balanced: bool,
) -> Result<(ResourceId, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  if reuse_port {
    super::check_unstable(state, "Deno.listen({ reusePort: true })");
  }
  state
    .borrow_mut::<NP>()
    .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listen()")?;
  let addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or_else(|| NetError::NoResolvedAddress)?;

  let listener = if load_balanced {
    TcpListener::bind_load_balanced(addr)
  } else {
    TcpListener::bind_direct(addr, reuse_port)
  }?;
  let local_addr = listener.local_addr()?;
  let listener_resource = NetworkListenerResource::new(listener);
  let rid = state.resource_table.add(listener_resource);

  Ok((rid, IpAddr::from(local_addr)))
}

fn net_listen_udp<NP>(
  state: &mut OpState,
  addr: IpAddr,
  reuse_address: bool,
  loopback: bool,
) -> Result<(ResourceId, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  state
    .borrow_mut::<NP>()
    .check_net(&(&addr.hostname, Some(addr.port)), "Deno.listenDatagram()")?;
  let addr = resolve_addr_sync(&addr.hostname, addr.port)?
    .next()
    .ok_or_else(|| NetError::NoResolvedAddress)?;

  let domain = if addr.is_ipv4() {
    Domain::IPV4
  } else {
    Domain::IPV6
  };
  let socket_tmp = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
  if reuse_address {
    // This logic is taken from libuv:
    //
    // On the BSDs, SO_REUSEPORT implies SO_REUSEADDR but with some additional
    // refinements for programs that use multicast.
    //
    // Linux as of 3.9 has a SO_REUSEPORT socket option but with semantics that
    // are different from the BSDs: it _shares_ the port rather than steal it
    // from the current listener. While useful, it's not something we can
    // emulate on other platforms so we don't enable it.
    #[cfg(any(
      target_os = "windows",
      target_os = "android",
      target_os = "linux"
    ))]
    socket_tmp.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "linux")))]
    socket_tmp.set_reuse_port(true)?;
  }
  let socket_addr = socket2::SockAddr::from(addr);
  socket_tmp.bind(&socket_addr)?;
  socket_tmp.set_nonblocking(true)?;

  // Enable messages to be sent to the broadcast address (255.255.255.255) by default
  socket_tmp.set_broadcast(true)?;

  if domain == Domain::IPV4 {
    socket_tmp.set_multicast_loop_v4(loopback)?;
  } else {
    socket_tmp.set_multicast_loop_v6(loopback)?;
  }

  let std_socket: std::net::UdpSocket = socket_tmp.into();

  let socket = UdpSocket::from_std(std_socket)?;
  let local_addr = socket.local_addr()?;
  let socket_resource = UdpSocketResource {
    socket: AsyncRefCell::new(socket),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(socket_resource);

  Ok((rid, IpAddr::from(local_addr)))
}

#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_udp<NP>(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  reuse_address: bool,
  loopback: bool,
) -> Result<(ResourceId, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  super::check_unstable(state, "Deno.listenDatagram");
  net_listen_udp::<NP>(state, addr, reuse_address, loopback)
}

#[op2(stack_trace)]
#[serde]
pub fn op_node_unstable_net_listen_udp<NP>(
  state: &mut OpState,
  #[serde] addr: IpAddr,
  reuse_address: bool,
  loopback: bool,
) -> Result<(ResourceId, IpAddr), NetError>
where
  NP: NetPermissions + 'static,
{
  net_listen_udp::<NP>(state, addr, reuse_address, loopback)
}

#[cfg(unix)]
#[op2(async, stack_trace)]
#[serde]
pub async fn op_net_connect_vsock<NP>(
  state: Rc<RefCell<OpState>>,
  #[smi] cid: u32,
  #[smi] port: u32,
) -> Result<(ResourceId, (u32, u32), (u32, u32)), NetError>
where
  NP: NetPermissions + 'static,
{
  use tokio_vsock::VsockAddr;
  use tokio_vsock::VsockStream;

  state
    .borrow()
    .feature_checker
    .check_or_exit("vsock", "Deno.connect");

  state.borrow_mut().borrow_mut::<NP>().check_vsock(
    cid,
    port,
    "Deno.connect()",
  )?;

  let addr = VsockAddr::new(cid, port);
  let vsock_stream = VsockStream::connect(addr).await?;
  let local_addr = vsock_stream.local_addr()?;
  let remote_addr = vsock_stream.peer_addr()?;

  let rid =
    state
      .borrow_mut()
      .resource_table
      .add(crate::io::VsockStreamResource::new(
        vsock_stream.into_split(),
      ));

  Ok((
    rid,
    (local_addr.cid(), local_addr.port()),
    (remote_addr.cid(), remote_addr.port()),
  ))
}

#[cfg(not(unix))]
#[op2]
#[serde]
pub fn op_net_connect_vsock<NP>() -> Result<(), NetError>
where
  NP: NetPermissions + 'static,
{
  Err(NetError::VsockUnsupported)
}

#[cfg(unix)]
#[op2(stack_trace)]
#[serde]
pub fn op_net_listen_vsock<NP>(
  state: &mut OpState,
  #[smi] cid: u32,
  #[smi] port: u32,
) -> Result<(ResourceId, u32, u32), NetError>
where
  NP: NetPermissions + 'static,
{
  use tokio_vsock::VsockAddr;
  use tokio_vsock::VsockListener;

  state.feature_checker.check_or_exit("vsock", "Deno.listen");

  state
    .borrow_mut::<NP>()
    .check_vsock(cid, port, "Deno.listen()")?;

  let addr = VsockAddr::new(cid, port);
  let listener = VsockListener::bind(addr)?;
  let local_addr = listener.local_addr()?;
  let listener_resource = NetworkListenerResource::new(listener);
  let rid = state.resource_table.add(listener_resource);
  Ok((rid, local_addr.cid(), local_addr.port()))
}

#[cfg(not(unix))]
#[op2]
#[serde]
pub fn op_net_listen_vsock<NP>() -> Result<(), NetError>
where
  NP: NetPermissions + 'static,
{
  Err(NetError::VsockUnsupported)
}

#[cfg(unix)]
#[op2(async)]
#[serde]
pub async fn op_net_accept_vsock(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(ResourceId, (u32, u32), (u32, u32)), NetError> {
  use tokio_vsock::VsockListener;

  let resource = state
    .borrow()
    .resource_table
    .get::<NetworkListenerResource<VsockListener>>(rid)
    .map_err(|_| NetError::ListenerClosed)?;
  let listener = RcRef::map(&resource, |r| &r.listener)
    .try_borrow_mut()
    .ok_or_else(|| NetError::AcceptTaskOngoing)?;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let (vsock_stream, _socket_addr) = listener
    .accept()
    .try_or_cancel(cancel)
    .await
    .map_err(accept_err)?;
  let local_addr = vsock_stream.local_addr()?;
  let remote_addr = vsock_stream.peer_addr()?;

  let mut state = state.borrow_mut();
  let rid = state
    .resource_table
    .add(crate::io::VsockStreamResource::new(
      vsock_stream.into_split(),
    ));
  Ok((
    rid,
    (local_addr.cid(), local_addr.port()),
    (remote_addr.cid(), remote_addr.port()),
  ))
}

#[cfg(not(unix))]
#[op2]
#[serde]
pub fn op_net_accept_vsock() -> Result<(), NetError> {
  Err(NetError::VsockUnsupported)
}

#[derive(Serialize, Eq, PartialEq, Debug)]
#[serde(untagged)]
pub enum DnsReturnRecord {
  A(String),
  Aaaa(String),
  Aname(String),
  Caa {
    critical: bool,
    tag: String,
    value: String,
  },
  Cname(String),
  Mx {
    preference: u16,
    exchange: String,
  },
  Naptr {
    order: u16,
    preference: u16,
    flags: String,
    services: String,
    regexp: String,
    replacement: String,
  },
  Ns(String),
  Ptr(String),
  Soa {
    mname: String,
    rname: String,
    serial: u32,
    refresh: i32,
    retry: i32,
    expire: i32,
    minimum: u32,
  },
  Srv {
    priority: u16,
    weight: u16,
    port: u16,
    target: String,
  },
  Txt(Vec<String>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveAddrArgs {
  cancel_rid: Option<ResourceId>,
  query: String,
  record_type: RecordType,
  options: Option<ResolveDnsOption>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveDnsOption {
  name_server: Option<NameServer>,
}

fn default_port() -> u16 {
  53
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameServer {
  ip_addr: String,
  #[serde(default = "default_port")]
  port: u16,
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_dns_resolve<NP>(
  state: Rc<RefCell<OpState>>,
  #[serde] args: ResolveAddrArgs,
) -> Result<Vec<DnsReturnRecord>, NetError>
where
  NP: NetPermissions + 'static,
{
  let ResolveAddrArgs {
    query,
    record_type,
    options,
    cancel_rid,
  } = args;

  let (config, opts) = if let Some(name_server) =
    options.as_ref().and_then(|o| o.name_server.as_ref())
  {
    let group = NameServerConfigGroup::from_ips_clear(
      &[name_server.ip_addr.parse()?],
      name_server.port,
      true,
    );
    (
      ResolverConfig::from_parts(None, vec![], group),
      ResolverOpts::default(),
    )
  } else {
    system_conf::read_system_conf()?
  };

  {
    let mut s = state.borrow_mut();
    let perm = s.borrow_mut::<NP>();

    // Checks permission against the name servers which will be actually queried.
    for ns in config.name_servers() {
      let socker_addr = &ns.socket_addr;
      let ip = socker_addr.ip().to_string();
      let port = socker_addr.port();
      perm.check_net(&(ip, Some(port)), "Deno.resolveDns()")?;
    }
  }

  let resolver = hickory_resolver::Resolver::tokio(config, opts);

  let lookup_fut = resolver.lookup(query, record_type);

  let cancel_handle = cancel_rid.and_then(|rid| {
    state
      .borrow_mut()
      .resource_table
      .get::<CancelHandle>(rid)
      .ok()
  });

  let lookup = if let Some(cancel_handle) = cancel_handle {
    let lookup_rv = lookup_fut.or_cancel(cancel_handle).await;

    if let Some(cancel_rid) = cancel_rid {
      if let Ok(res) = state.borrow_mut().resource_table.take_any(cancel_rid) {
        res.close();
      }
    };

    lookup_rv?
  } else {
    lookup_fut.await
  };

  lookup
    .map_err(|e| match e.kind() {
      ResolveErrorKind::Proto(ProtoError { kind, .. })
        if matches!(**kind, ProtoErrorKind::NoRecordsFound { .. }) =>
      {
        NetError::DnsNotFound(e)
      }
      ResolveErrorKind::Proto(ProtoError { kind, .. })
        if matches!(**kind, ProtoErrorKind::NoConnections { .. }) =>
      {
        NetError::DnsNotConnected(e)
      }
      ResolveErrorKind::Proto(ProtoError { kind, .. })
        if matches!(**kind, ProtoErrorKind::Timeout { .. }) =>
      {
        NetError::DnsTimedOut(e)
      }
      _ => NetError::Dns(e),
    })?
    .iter()
    .filter_map(|rdata| rdata_to_return_record(record_type)(rdata).transpose())
    .collect::<Result<Vec<DnsReturnRecord>, NetError>>()
}

#[op2(fast)]
pub fn op_set_nodelay(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  nodelay: bool,
) -> Result<(), NetError> {
  op_set_nodelay_inner(state, rid, nodelay)
}

#[inline]
pub fn op_set_nodelay_inner(
  state: &mut OpState,
  rid: ResourceId,
  nodelay: bool,
) -> Result<(), NetError> {
  let resource: Rc<TcpStreamResource> =
    state.resource_table.get::<TcpStreamResource>(rid)?;
  resource.set_nodelay(nodelay).map_err(NetError::Map)
}

#[op2(fast)]
pub fn op_set_keepalive(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  keepalive: bool,
) -> Result<(), NetError> {
  op_set_keepalive_inner(state, rid, keepalive)
}

#[inline]
pub fn op_set_keepalive_inner(
  state: &mut OpState,
  rid: ResourceId,
  keepalive: bool,
) -> Result<(), NetError> {
  let resource: Rc<TcpStreamResource> =
    state.resource_table.get::<TcpStreamResource>(rid)?;
  resource.set_keepalive(keepalive).map_err(NetError::Map)
}

fn rdata_to_return_record(
  ty: RecordType,
) -> impl Fn(&RData) -> Result<Option<DnsReturnRecord>, NetError> {
  use RecordType::*;
  move |r: &RData| -> Result<Option<DnsReturnRecord>, NetError> {
    let record = match ty {
      A => r.as_a().map(ToString::to_string).map(DnsReturnRecord::A),
      AAAA => r
        .as_aaaa()
        .map(ToString::to_string)
        .map(DnsReturnRecord::Aaaa),
      ANAME => r
        .as_aname()
        .map(ToString::to_string)
        .map(DnsReturnRecord::Aname),
      CAA => r.as_caa().map(|caa| DnsReturnRecord::Caa {
        critical: caa.issuer_critical(),
        tag: caa.tag().to_string(),
        value: match caa.value() {
          Value::Issuer(name, key_values) => {
            let mut s = String::new();

            if let Some(name) = name {
              s.push_str(&name.to_string());
            } else if name.is_none() && key_values.is_empty() {
              s.push(';');
            }

            for key_value in key_values {
              s.push_str("; ");
              s.push_str(&key_value.to_string());
            }

            s
          }
          Value::Url(url) => url.to_string(),
          Value::Unknown(data) => String::from_utf8(data.to_vec()).unwrap(),
        },
      }),
      CNAME => r
        .as_cname()
        .map(ToString::to_string)
        .map(DnsReturnRecord::Cname),
      MX => r.as_mx().map(|mx| DnsReturnRecord::Mx {
        preference: mx.preference(),
        exchange: mx.exchange().to_string(),
      }),
      NAPTR => r.as_naptr().map(|naptr| DnsReturnRecord::Naptr {
        order: naptr.order(),
        preference: naptr.preference(),
        flags: String::from_utf8(naptr.flags().to_vec()).unwrap(),
        services: String::from_utf8(naptr.services().to_vec()).unwrap(),
        regexp: String::from_utf8(naptr.regexp().to_vec()).unwrap(),
        replacement: naptr.replacement().to_string(),
      }),
      NS => r.as_ns().map(ToString::to_string).map(DnsReturnRecord::Ns),
      PTR => r
        .as_ptr()
        .map(ToString::to_string)
        .map(DnsReturnRecord::Ptr),
      SOA => r.as_soa().map(|soa| DnsReturnRecord::Soa {
        mname: soa.mname().to_string(),
        rname: soa.rname().to_string(),
        serial: soa.serial(),
        refresh: soa.refresh(),
        retry: soa.retry(),
        expire: soa.expire(),
        minimum: soa.minimum(),
      }),
      SRV => r.as_srv().map(|srv| DnsReturnRecord::Srv {
        priority: srv.priority(),
        weight: srv.weight(),
        port: srv.port(),
        target: srv.target().to_string(),
      }),
      TXT => r.as_txt().map(|txt| {
        let texts: Vec<String> = txt
          .iter()
          .map(|bytes| {
            // Tries to parse these bytes as Latin-1
            bytes.iter().map(|&b| b as char).collect::<String>()
          })
          .collect();
        DnsReturnRecord::Txt(texts)
      }),
      _ => return Err(NetError::UnsupportedRecordType),
    };
    Ok(record)
  }
}

#[cfg(test)]
mod tests {
  use std::net::Ipv4Addr;
  use std::net::Ipv6Addr;
  use std::net::ToSocketAddrs;
  use std::path::Path;
  use std::path::PathBuf;
  use std::sync::Arc;
  use std::sync::Mutex;

  use deno_core::futures::FutureExt;
  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;
  use deno_permissions::PermissionCheckError;
  use hickory_proto::rr::rdata::a::A;
  use hickory_proto::rr::rdata::aaaa::AAAA;
  use hickory_proto::rr::rdata::caa::KeyValue;
  use hickory_proto::rr::rdata::caa::CAA;
  use hickory_proto::rr::rdata::mx::MX;
  use hickory_proto::rr::rdata::name::ANAME;
  use hickory_proto::rr::rdata::name::CNAME;
  use hickory_proto::rr::rdata::name::NS;
  use hickory_proto::rr::rdata::name::PTR;
  use hickory_proto::rr::rdata::naptr::NAPTR;
  use hickory_proto::rr::rdata::srv::SRV;
  use hickory_proto::rr::rdata::txt::TXT;
  use hickory_proto::rr::rdata::SOA;
  use hickory_proto::rr::record_data::RData;
  use hickory_proto::rr::Name;
  use socket2::SockRef;

  use super::*;

  #[test]
  fn rdata_to_return_record_a() {
    let func = rdata_to_return_record(RecordType::A);
    let rdata = RData::A(A(Ipv4Addr::new(127, 0, 0, 1)));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::A("127.0.0.1".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_aaaa() {
    let func = rdata_to_return_record(RecordType::AAAA);
    let rdata = RData::AAAA(AAAA(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Aaaa("::1".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_aname() {
    let func = rdata_to_return_record(RecordType::ANAME);
    let rdata = RData::ANAME(ANAME(Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Aname("".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_caa() {
    let func = rdata_to_return_record(RecordType::CAA);
    let rdata = RData::CAA(CAA::new_issue(
      false,
      Some(Name::parse("example.com", None).unwrap()),
      vec![KeyValue::new("account", "123456")],
    ));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Caa {
        critical: false,
        tag: "issue".to_string(),
        value: "example.com; account=123456".to_string(),
      })
    );
  }

  #[test]
  fn rdata_to_return_record_cname() {
    let func = rdata_to_return_record(RecordType::CNAME);
    let rdata = RData::CNAME(CNAME(Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Cname("".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_mx() {
    let func = rdata_to_return_record(RecordType::MX);
    let rdata = RData::MX(MX::new(10, Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Mx {
        preference: 10,
        exchange: "".to_string()
      })
    );
  }

  #[test]
  fn rdata_to_return_record_naptr() {
    let func = rdata_to_return_record(RecordType::NAPTR);
    let rdata = RData::NAPTR(NAPTR::new(
      1,
      2,
      <Box<[u8]>>::default(),
      <Box<[u8]>>::default(),
      <Box<[u8]>>::default(),
      Name::new(),
    ));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Naptr {
        order: 1,
        preference: 2,
        flags: "".to_string(),
        services: "".to_string(),
        regexp: "".to_string(),
        replacement: "".to_string()
      })
    );
  }

  #[test]
  fn rdata_to_return_record_ns() {
    let func = rdata_to_return_record(RecordType::NS);
    let rdata = RData::NS(NS(Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Ns("".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_ptr() {
    let func = rdata_to_return_record(RecordType::PTR);
    let rdata = RData::PTR(PTR(Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Ptr("".to_string()))
    );
  }

  #[test]
  fn rdata_to_return_record_soa() {
    let func = rdata_to_return_record(RecordType::SOA);
    let rdata = RData::SOA(SOA::new(
      Name::new(),
      Name::new(),
      0,
      i32::MAX,
      i32::MAX,
      i32::MAX,
      0,
    ));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Soa {
        mname: "".to_string(),
        rname: "".to_string(),
        serial: 0,
        refresh: i32::MAX,
        retry: i32::MAX,
        expire: i32::MAX,
        minimum: 0,
      })
    );
  }

  #[test]
  fn rdata_to_return_record_srv() {
    let func = rdata_to_return_record(RecordType::SRV);
    let rdata = RData::SRV(SRV::new(1, 2, 3, Name::new()));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Srv {
        priority: 1,
        weight: 2,
        port: 3,
        target: "".to_string()
      })
    );
  }

  #[test]
  fn rdata_to_return_record_txt() {
    let func = rdata_to_return_record(RecordType::TXT);
    let rdata = RData::TXT(TXT::from_bytes(vec![
      "foo".as_bytes(),
      "bar".as_bytes(),
      &[0xa3],             // "£" in Latin-1
      &[0xe3, 0x81, 0x82], // "あ" in UTF-8
    ]));
    assert_eq!(
      func(&rdata).unwrap(),
      Some(DnsReturnRecord::Txt(vec![
        "foo".to_string(),
        "bar".to_string(),
        "£".to_string(),
        "ã\u{81}\u{82}".to_string(),
      ]))
    );
  }

  struct TestPermission {}

  impl NetPermissions for TestPermission {
    fn check_net<T: AsRef<str>>(
      &mut self,
      _host: &(T, Option<u16>),
      _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
      Ok(())
    }

    fn check_read(
      &mut self,
      p: &str,
      _api_name: &str,
    ) -> Result<PathBuf, PermissionCheckError> {
      Ok(PathBuf::from(p))
    }

    fn check_write(
      &mut self,
      p: &str,
      _api_name: &str,
    ) -> Result<PathBuf, PermissionCheckError> {
      Ok(PathBuf::from(p))
    }

    fn check_write_path<'a>(
      &mut self,
      p: &'a Path,
      _api_name: &str,
    ) -> Result<Cow<'a, Path>, PermissionCheckError> {
      Ok(Cow::Borrowed(p))
    }

    fn check_vsock(
      &mut self,
      _cid: u32,
      _port: u32,
      _api_name: &str,
    ) -> Result<(), PermissionCheckError> {
      Ok(())
    }
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn tcp_set_no_delay() {
    let set_nodelay = Box::new(|state: &mut OpState, rid| {
      op_set_nodelay_inner(state, rid, true).unwrap();
    });
    let test_fn = Box::new(|socket: SockRef| {
      assert!(socket.nodelay().unwrap());
      assert!(!socket.keepalive().unwrap());
    });
    check_sockopt(String::from("127.0.0.1:4145"), set_nodelay, test_fn).await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
  async fn tcp_set_keepalive() {
    let set_keepalive = Box::new(|state: &mut OpState, rid| {
      op_set_keepalive_inner(state, rid, true).unwrap();
    });
    let test_fn = Box::new(|socket: SockRef| {
      assert!(!socket.nodelay().unwrap());
      assert!(socket.keepalive().unwrap());
    });
    check_sockopt(String::from("127.0.0.1:4146"), set_keepalive, test_fn).await;
  }

  #[allow(clippy::type_complexity)]
  async fn check_sockopt(
    addr: String,
    set_sockopt_fn: Box<dyn Fn(&mut OpState, u32)>,
    test_fn: Box<dyn FnOnce(SockRef)>,
  ) {
    let sockets = Arc::new(Mutex::new(vec![]));
    let clone_addr = addr.clone();
    let addr = addr.to_socket_addrs().unwrap().next().unwrap();
    let listener = TcpListener::bind_direct(addr, false).unwrap();
    let accept_fut = listener.accept().boxed_local();
    let store_fut = async move {
      let socket = accept_fut.await.unwrap();
      sockets.lock().unwrap().push(socket);
    }
    .boxed_local();

    deno_core::extension!(
      test_ext,
      state = |state| {
        state.put(TestPermission {});
      }
    );

    let runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      feature_checker: Some(Arc::new(Default::default())),
      ..Default::default()
    });

    let conn_state = runtime.op_state();

    let server_addr: Vec<&str> = clone_addr.split(':').collect();
    let ip_addr = IpAddr {
      hostname: String::from(server_addr[0]),
      port: server_addr[1].parse().unwrap(),
    };

    let mut connect_fut =
      op_net_connect_tcp_inner::<TestPermission>(conn_state, ip_addr, None)
        .boxed_local();
    let mut rid = None;

    tokio::select! {
      _ = store_fut => {
        let result = connect_fut.await;
        let vals = result.unwrap();
        rid = rid.or(Some(vals.0));
      },
      result = &mut connect_fut => {
        let vals = result.unwrap();
        rid = rid.or(Some(vals.0));
      }
    }
    let rid = rid.unwrap();

    let state = runtime.op_state();
    set_sockopt_fn(&mut state.borrow_mut(), rid);

    let resource = state
      .borrow_mut()
      .resource_table
      .get::<TcpStreamResource>(rid)
      .unwrap();

    let wr = resource.wr_borrow_mut().await;
    let stream = wr.as_ref().as_ref();
    let socket = socket2::SockRef::from(stream);
    test_fn(socket);
  }
}
