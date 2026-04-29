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
  ipv6_only: bool,
) -> Result<(ResourceId, String, u16), NodeUdpError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_net(&(hostname, Some(port)), "dgram.createSocket()")?;

  let addr = deno_net::resolve_addr::resolve_addr_sync(hostname, port)?
    .next()
    .ok_or(NodeUdpError::NoResolvedAddress)?;
  state
    .borrow_mut::<PermissionsContainer>()
    .check_net_resolved(&addr.ip(), addr.port(), "dgram.createSocket()")?;

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
  if addr.is_ipv6() && ipv6_only {
    sock.set_only_v6(true)?;
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

/// Resolve an IPv6 interface address to an interface index.
/// If the address contains a zone ID (e.g. "fe80::1%eth0"), use that.
/// Otherwise, try to find the interface by matching the address.
fn resolve_ipv6_interface(
  interface_addr: Option<&str>,
) -> Result<u32, NodeUdpError> {
  let Some(addr_str) = interface_addr else {
    return Ok(0);
  };

  // Check if the address contains a zone ID (e.g. "fe80::1%eth0" or "::1%1")
  if let Some(zone_idx) = addr_str.find('%') {
    let zone_id = &addr_str[zone_idx + 1..];
    // Try parsing as numeric first
    if let Ok(idx) = zone_id.parse::<u32>() {
      return Ok(idx);
    }
    // Otherwise try as interface name
    #[cfg(unix)]
    {
      use std::ffi::CString;
      if let Ok(c_name) = CString::new(zone_id) {
        // SAFETY: if_nametoindex is safe to call with a valid C string
        let idx = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
        if idx != 0 {
          return Ok(idx);
        }
      }
    }
    #[cfg(windows)]
    {
      // On Windows, try parsing as numeric index
      if let Ok(idx) = zone_id.parse::<u32>() {
        return Ok(idx);
      }
    }
  }

  // Try to find interface by matching the address
  #[cfg(unix)]
  {
    use std::ffi::CStr;
    let target_addr =
      Ipv6Addr::from_str(addr_str.split('%').next().unwrap_or(addr_str))?;

    // Get all interfaces and find one with matching address
    let mut addrs: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs is safe to call with a valid pointer
    if unsafe { libc::getifaddrs(&mut addrs) } == 0 {
      let mut current = addrs;
      while !current.is_null() {
        // SAFETY: we checked current is not null
        let ifa = unsafe { &*current };
        if !ifa.ifa_addr.is_null() {
          // SAFETY: we checked ifa_addr is not null
          let family = unsafe { (*ifa.ifa_addr).sa_family };
          if family == libc::AF_INET6 as libc::sa_family_t {
            // SAFETY: we verified this is AF_INET6
            let sockaddr_in6 =
              unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in6) };
            let addr = Ipv6Addr::from(sockaddr_in6.sin6_addr.s6_addr);
            if addr == target_addr {
              // SAFETY: ifa_name is a valid C string
              let name = unsafe { CStr::from_ptr(ifa.ifa_name) };
              if let Ok(name_str) = name.to_str()
                && let Ok(c_name) = std::ffi::CString::new(name_str)
              {
                // SAFETY: if_nametoindex is safe with valid C string
                let idx = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
                // SAFETY: freeifaddrs is safe to call with the pointer from getifaddrs
                unsafe { libc::freeifaddrs(addrs) };
                if idx != 0 {
                  return Ok(idx);
                }
              }
            }
          }
        }
        current = ifa.ifa_next;
      }
      // SAFETY: freeifaddrs is safe to call with the pointer from getifaddrs
      unsafe { libc::freeifaddrs(addrs) };
    }
  }

  // Default to 0 if we couldn't resolve
  Ok(0)
}

#[op2]
pub fn op_node_udp_join_multi_v6(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[string] interface_addr: Option<String>,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv6Addr::from_str(address)?;
  let iface = resolve_ipv6_interface(interface_addr.as_deref())?;
  resource.socket.join_multicast_v6(&addr, iface)?;
  Ok(())
}

#[op2]
pub fn op_node_udp_leave_multi_v6(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] address: &str,
  #[string] interface_addr: Option<String>,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let addr = Ipv6Addr::from_str(address)?;
  let iface = resolve_ipv6_interface(interface_addr.as_deref())?;
  resource.socket.leave_multicast_v6(&addr, iface)?;
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

#[op2(fast)]
pub fn op_node_udp_set_ttl(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[smi] ttl: u32,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  let sock_ref = socket2::SockRef::from(&resource.socket);
  sock_ref.set_ttl(ttl)?;
  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_set_multicast_interface(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  is_ipv6: bool,
  #[string] interface_address: &str,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  let sock_ref = socket2::SockRef::from(&resource.socket);
  if is_ipv6 {
    let index = ipv6_interface_index(interface_address)?;
    sock_ref.set_multicast_if_v6(index)?;
  } else {
    let addr: Ipv4Addr = interface_address.parse().map_err(|_| {
      NodeUdpError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "invalid IPv4 address",
      ))
    })?;
    sock_ref.set_multicast_if_v4(&addr)?;
  }
  Ok(())
}

/// Parse an IPv6 interface address string to a network interface index.
/// Matches libuv's `uv__udp_set_multicast_interface6` behavior:
/// - Parses IPv6 address strings like `::%lo0`, `::%1`, `::`
/// - Extracts scope_id and resolves interface names via if_nametoindex
/// - Returns EINVAL for empty or unparseable addresses
fn ipv6_interface_index(interface_address: &str) -> Result<u32, NodeUdpError> {
  let einval =
    || NodeUdpError::Io(std::io::Error::from_raw_os_error(libc::EINVAL));

  if interface_address.is_empty() {
    return Err(einval());
  }

  // Check for scope ID separator
  if let Some(pos) = interface_address.rfind('%') {
    // Validate the address part before % is a valid IPv6 address
    let addr_part = &interface_address[..pos];
    if addr_part.parse::<Ipv6Addr>().is_err() {
      return Err(einval());
    }

    let scope_id = &interface_address[pos + 1..];
    if scope_id.is_empty() {
      return Err(einval());
    }

    // Try numeric scope ID first
    if let Ok(index) = scope_id.parse::<u32>() {
      return Ok(index);
    }

    // Resolve interface name to index
    #[cfg(unix)]
    {
      let name = std::ffi::CString::new(scope_id).map_err(|_| einval())?;
      // SAFETY: name is a valid CString
      let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
      // if_nametoindex returns 0 for unknown interfaces, which is
      // acceptable as "default selection" (matches libuv behavior)
      return Ok(index);
    }
    #[cfg(windows)]
    {
      let name = std::ffi::CString::new(scope_id).map_err(|_| einval())?;
      // SAFETY: name is a valid CString
      let index = unsafe {
        windows_sys::Win32::NetworkManagement::IpHelper::if_nametoindex(
          name.as_ptr() as *const u8,
        )
      };
      return Ok(index);
    }
    #[cfg(not(any(unix, windows)))]
    return Ok(0);
  }

  // No scope ID separator — try parsing as plain IPv6 address
  if interface_address.parse::<Ipv6Addr>().is_ok() {
    return Ok(0);
  }

  Err(einval())
}

fn source_specific_multicast(
  socket: &UdpSocket,
  source_addr: Ipv4Addr,
  group_addr: Ipv4Addr,
  interface_addr: Ipv4Addr,
  option: i32,
) -> Result<(), NodeUdpError> {
  #[cfg(unix)]
  {
    let mreq = libc::ip_mreq_source {
      imr_multiaddr: libc::in_addr {
        s_addr: u32::from(group_addr).to_be(),
      },
      imr_sourceaddr: libc::in_addr {
        s_addr: u32::from(source_addr).to_be(),
      },
      imr_interface: libc::in_addr {
        s_addr: u32::from(interface_addr).to_be(),
      },
    };

    // SAFETY: We pass a valid socket fd, level, option, and correctly-sized struct.
    let ret = unsafe {
      libc::setsockopt(
        std::os::fd::AsRawFd::as_raw_fd(socket),
        libc::IPPROTO_IP,
        option,
        &mreq as *const libc::ip_mreq_source as *const libc::c_void,
        std::mem::size_of::<libc::ip_mreq_source>() as libc::socklen_t,
      )
    };
    if ret != 0 {
      return Err(std::io::Error::last_os_error().into());
    }
  }

  #[cfg(windows)]
  {
    use std::os::windows::io::AsRawSocket;

    #[repr(C)]
    struct IpMreqSource {
      imr_multiaddr: u32,
      imr_sourceaddr: u32,
      imr_interface: u32,
    }

    let mreq = IpMreqSource {
      imr_multiaddr: u32::from(group_addr).to_be(),
      imr_sourceaddr: u32::from(source_addr).to_be(),
      imr_interface: u32::from(interface_addr).to_be(),
    };

    // SAFETY: We pass a valid socket, level, option, and correctly-sized struct.
    let ret = unsafe {
      windows_sys::Win32::Networking::WinSock::setsockopt(
        socket.as_raw_socket() as usize,
        windows_sys::Win32::Networking::WinSock::IPPROTO_IP,
        option,
        &mreq as *const IpMreqSource as *const u8,
        std::mem::size_of::<IpMreqSource>() as i32,
      )
    };
    if ret != 0 {
      return Err(std::io::Error::last_os_error().into());
    }
  }

  Ok(())
}

#[op2(fast)]
pub fn op_node_udp_join_source_specific(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] source_address: &str,
  #[string] group_address: &str,
  #[string] interface_address: &str,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let source_addr = Ipv4Addr::from_str(source_address)?;
  let group_addr = Ipv4Addr::from_str(group_address)?;
  let interface_addr = Ipv4Addr::from_str(interface_address)?;

  #[cfg(unix)]
  let option = libc::IP_ADD_SOURCE_MEMBERSHIP;
  #[cfg(windows)]
  let option =
    windows_sys::Win32::Networking::WinSock::IP_ADD_SOURCE_MEMBERSHIP;

  source_specific_multicast(
    &resource.socket,
    source_addr,
    group_addr,
    interface_addr,
    option,
  )
}

#[op2(fast)]
pub fn op_node_udp_leave_source_specific(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] source_address: &str,
  #[string] group_address: &str,
  #[string] interface_address: &str,
) -> Result<(), NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;

  let source_addr = Ipv4Addr::from_str(source_address)?;
  let group_addr = Ipv4Addr::from_str(group_address)?;
  let interface_addr = Ipv4Addr::from_str(interface_address)?;

  #[cfg(unix)]
  let option = libc::IP_DROP_SOURCE_MEMBERSHIP;
  #[cfg(windows)]
  let option =
    windows_sys::Win32::Networking::WinSock::IP_DROP_SOURCE_MEMBERSHIP;

  source_specific_multicast(
    &resource.socket,
    source_addr,
    group_addr,
    interface_addr,
    option,
  )
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
  {
    state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(&hostname, Some(port)), "socket.send()")?;
  }

  let resource = state
    .borrow()
    .resource_table
    .get::<NodeUdpSocketResource>(rid)?;

  let addr: SocketAddr =
    deno_net::resolve_addr::resolve_addr_sync(&hostname, port)?
      .next()
      .ok_or(NodeUdpError::NoResolvedAddress)?;
  {
    state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net_resolved(&addr.ip(), addr.port(), "socket.send()")?;
  }

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
