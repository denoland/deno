// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::serde::Serialize;
use deno_core::uv_compat;
use deno_core::v8;
use deno_permissions::PermissionsContainer;
use socket2::Domain;
use socket2::Protocol;
use socket2::Socket;
use socket2::Type;
use tokio::net::UdpSocket;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::ProviderType;

#[derive(CppgcBase, CppgcInherits)]
#[cppgc_inherits_from(AsyncWrap)]
#[repr(C)]
pub struct SendWrap {
  base: AsyncWrap,
}

// SAFETY: SendWrap is a CppGC object whose fields are traced by AsyncWrap.
unsafe impl GarbageCollected for SendWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"SendWrap"
  }

  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}
}

#[op2(base, inherit = AsyncWrap)]
impl SendWrap {
  #[constructor]
  #[cppgc]
  fn constructor(state: &mut OpState) -> SendWrap {
    SendWrap {
      base: AsyncWrap::create(state, ProviderType::UdpSendWrap as i32),
    }
  }
}

#[derive(CppgcBase, CppgcInherits)]
#[cppgc_inherits_from(HandleWrap)]
#[repr(C)]
pub struct UDP {
  base: HandleWrap,
  rid: Cell<Option<ResourceId>>,
  address: RefCell<Option<String>>,
  family: Cell<Option<UdpFamily>>,
  port: Cell<Option<u16>>,
  remote_address: RefCell<Option<String>>,
  remote_family: Cell<Option<UdpFamily>>,
  remote_port: Cell<Option<u16>>,
  recv_buffer_size: Cell<usize>,
  send_buffer_size: Cell<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UdpFamily {
  Ipv4,
  Ipv6,
}

impl UdpFamily {
  fn from_addr(addr: &str) -> Self {
    if addr.contains(':') {
      Self::Ipv6
    } else {
      Self::Ipv4
    }
  }

  fn from_family(family: i32) -> Self {
    if family == 10 { Self::Ipv6 } else { Self::Ipv4 }
  }

  fn as_str(self) -> &'static str {
    match self {
      Self::Ipv4 => "IPv4",
      Self::Ipv6 => "IPv6",
    }
  }

  fn is_ipv6(self) -> bool {
    self == Self::Ipv6
  }

  fn is_ipv4(self) -> bool {
    self == Self::Ipv4
  }
}

// SAFETY: UDP owns no V8 handles directly; all JS-visible state is held by HandleWrap.
unsafe impl GarbageCollected for UDP {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"UDP"
  }

  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}
}

impl UDP {
  fn new(state: &mut OpState) -> UDP {
    UDP {
      base: HandleWrap::create(
        AsyncWrap::create(state, ProviderType::UdpWrap as i32),
        None,
      ),
      rid: Cell::new(None),
      address: RefCell::new(None),
      family: Cell::new(None),
      port: Cell::new(None),
      remote_address: RefCell::new(None),
      remote_family: Cell::new(None),
      remote_port: Cell::new(None),
      recv_buffer_size: Cell::new(64 * 1024),
      send_buffer_size: Cell::new(64 * 1024),
    }
  }
}

const UV_UNKNOWN: i32 = -4094;

fn io_error_to_uv(err: &std::io::Error) -> i32 {
  match err.raw_os_error() {
    #[cfg(windows)]
    Some(code) if code == libc::EINVAL || code == 10022 => uv_compat::UV_EINVAL,
    #[cfg(windows)]
    Some(10040) => -4065,
    Some(code @ (40 | 90)) => -code,
    Some(code) => -code,
    None => uv_compat::UV_EINVAL,
  }
}

fn udp_error_to_uv(err: &NodeUdpError) -> i32 {
  match err {
    NodeUdpError::Io(err) => io_error_to_uv(err),
    NodeUdpError::Resource(_) => uv_compat::UV_EBADF,
    NodeUdpError::AddrParse(_)
    | NodeUdpError::NoResolvedAddress
    | NodeUdpError::InvalidHostname(_)
    | NodeUdpError::InvalidSendBuffer => uv_compat::UV_EINVAL,
    NodeUdpError::Canceled(_) => uv_compat::UV_ECANCELED,
    NodeUdpError::Permission(_) => UV_UNKNOWN,
  }
}

fn set_i32<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  name: &str,
  value: i32,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = v8::Integer::new(scope, value);
  obj.set(scope, key.into(), value.into());
}

fn set_str<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  name: &str,
  value: &str,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = v8::String::new(scope, value).unwrap();
  obj.set(scope, key.into(), value.into());
}

impl UDP {
  fn set_bound_state(
    &self,
    rid: ResourceId,
    address: String,
    port: u16,
    family: UdpFamily,
  ) {
    self.rid.set(Some(rid));
    *self.address.borrow_mut() = Some(address);
    self.port.set(Some(port));
    self.family.set(Some(family));
  }

  fn bind_inner(
    &self,
    state: &mut OpState,
    ip: &str,
    port: i32,
    flags: i32,
    family: UdpFamily,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    let Ok(port) = u16::try_from(port) else {
      return Ok(uv_compat::UV_EINVAL);
    };
    match node_udp_bind(state, ip, port, (flags & 4) != 0, (flags & 2) != 0) {
      Ok((rid, address, bound_port)) => {
        self.set_bound_state(rid, address, bound_port, family);
        Ok(0)
      }
      Err(NodeUdpError::Permission(err)) => Err(err),
      Err(err) => Ok(udp_error_to_uv(&err)),
    }
  }

  fn set_remote(&self, ip: &str, port: i32, family: UdpFamily) -> i32 {
    let Ok(port) = u16::try_from(port) else {
      return uv_compat::UV_EINVAL;
    };
    *self.remote_address.borrow_mut() = Some(ip.to_string());
    self.remote_port.set(Some(port));
    self.remote_family.set(Some(family));
    0
  }
}

#[op2(base, inherit = HandleWrap)]
impl UDP {
  #[constructor]
  #[cppgc]
  fn constructor(state: &mut OpState) -> UDP {
    UDP::new(state)
  }

  #[nofast]
  fn bind(
    &self,
    state: &mut OpState,
    #[string] ip: &str,
    #[smi] port: i32,
    #[smi] flags: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    self.bind_inner(state, ip, port, flags, UdpFamily::Ipv4)
  }

  #[nofast]
  fn bind6(
    &self,
    state: &mut OpState,
    #[string] ip: &str,
    #[smi] port: i32,
    #[smi] flags: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    self.bind_inner(state, ip, port, flags, UdpFamily::Ipv6)
  }

  #[fast]
  fn open(&self, state: &mut OpState, #[smi] fd: i32) -> i32 {
    match node_udp_open(state, fd) {
      Ok((rid, address, port)) => {
        let family = UdpFamily::from_addr(&address);
        self.set_bound_state(rid, address, port, family);
        0
      }
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[fast]
  #[rename("fdForIpc")]
  fn fd_for_ipc(&self, state: &mut OpState) -> i32 {
    let Some(rid) = self.rid.get() else {
      return -1;
    };
    let Ok(resource) = state.resource_table.get::<NodeUdpSocketResource>(rid)
    else {
      return -1;
    };
    #[cfg(unix)]
    {
      use std::os::unix::io::AsRawFd;
      let fd = resource.socket.as_raw_fd();
      if fd < 0 {
        return -1;
      }
      // SAFETY: fd is a valid open file descriptor. F_DUPFD_CLOEXEC
      // atomically dups and sets CLOEXEC, avoiding a race window.
      unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) }
    }
    #[cfg(not(unix))]
    {
      let _ = resource;
      -1
    }
  }

  #[fast]
  fn connect(&self, #[string] ip: &str, #[smi] port: i32) -> i32 {
    self.set_remote(ip, port, UdpFamily::Ipv4)
  }

  #[fast]
  fn connect6(&self, #[string] ip: &str, #[smi] port: i32) -> i32 {
    self.set_remote(ip, port, UdpFamily::Ipv6)
  }

  #[fast]
  fn disconnect(&self) -> i32 {
    *self.remote_address.borrow_mut() = None;
    self.remote_port.set(None);
    self.remote_family.set(None);
    0
  }

  #[fast]
  #[rename("_setRemote")]
  fn set_remote_from_js(
    &self,
    #[string] ip: &str,
    #[smi] port: i32,
    #[smi] family: i32,
  ) -> i32 {
    self.set_remote(ip, port, UdpFamily::from_family(family))
  }

  #[nofast]
  fn getsockname<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    sockname: v8::Local<'s, v8::Object>,
  ) -> i32 {
    let address = self.address.borrow();
    let Some(address) = address.as_deref() else {
      return uv_compat::UV_EBADF;
    };
    let Some(port) = self.port.get() else {
      return uv_compat::UV_EBADF;
    };
    let Some(family) = self.family.get() else {
      return uv_compat::UV_EBADF;
    };
    set_str(scope, sockname, "address", address);
    set_i32(scope, sockname, "port", port.into());
    set_str(scope, sockname, "family", family.as_str());
    0
  }

  #[nofast]
  fn getpeername<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    peername: v8::Local<'s, v8::Object>,
  ) -> i32 {
    let address = self.remote_address.borrow();
    let Some(address) = address.as_deref() else {
      return uv_compat::UV_EBADF;
    };
    let Some(port) = self.remote_port.get() else {
      return uv_compat::UV_EBADF;
    };
    let Some(family) = self.remote_family.get() else {
      return uv_compat::UV_EBADF;
    };
    set_str(scope, peername, "address", address);
    set_i32(scope, peername, "port", port.into());
    set_str(scope, peername, "family", family.as_str());
    0
  }

  #[rename("bufferSize")]
  fn buffer_size<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[smi] size: i32,
    buffer: bool,
    ctx: v8::Local<'s, v8::Object>,
  ) -> v8::Local<'s, v8::Value> {
    if self.address.borrow().is_none() {
      #[cfg(windows)]
      let (code, errno, message) = (
        "ENOTSOCK",
        uv_compat::UV_ENOTSOCK,
        "socket operation on non-socket",
      );
      #[cfg(not(windows))]
      let (code, errno, message) =
        ("EBADF", uv_compat::UV_EBADF, "bad file descriptor");
      set_i32(scope, ctx, "errno", errno);
      set_str(scope, ctx, "code", code);
      set_str(scope, ctx, "message", message);
      set_str(
        scope,
        ctx,
        "syscall",
        if buffer {
          "uv_recv_buffer_size"
        } else {
          "uv_send_buffer_size"
        },
      );
      return v8::undefined(scope).into();
    }

    if size != 0 {
      let size = if cfg!(target_os = "linux") {
        size * 2
      } else {
        size
      };
      if buffer {
        self.recv_buffer_size.set(size as usize);
      } else {
        self.send_buffer_size.set(size as usize);
      }
      return v8::Integer::new(scope, size).into();
    }

    let size = if buffer {
      self.recv_buffer_size.get()
    } else {
      self.send_buffer_size.get()
    };
    v8::Integer::new(scope, size as i32).into()
  }

  #[fast]
  #[rename("setBroadcast")]
  fn set_broadcast(&self, state: &mut OpState, #[smi] on: i32) -> i32 {
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        resource.socket.set_broadcast(on == 1)?;
        Ok(())
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[rename("addMembership")]
  fn add_membership(
    &self,
    state: &mut OpState,
    #[string] multicast_address: &str,
    #[string] interface_address: Option<String>,
  ) -> i32 {
    let ipv4_addr = Ipv4Addr::from_str(multicast_address).ok();
    let ipv6_addr = Ipv6Addr::from_str(multicast_address).ok();
    if ipv4_addr.is_none() && ipv6_addr.is_none() {
      return uv_compat::UV_EINVAL;
    }
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = if self.family.get().is_some_and(UdpFamily::is_ipv6) {
      state
        .resource_table
        .get::<NodeUdpSocketResource>(rid)
        .map_err(NodeUdpError::from)
        .and_then(|resource| {
          let addr = ipv6_addr.ok_or_else(invalid_input)?;
          let iface = resolve_ipv6_interface(interface_address.as_deref())?;
          resource.socket.join_multicast_v6(&addr, iface)?;
          Ok(())
        })
    } else {
      state
        .resource_table
        .get::<NodeUdpSocketResource>(rid)
        .map_err(NodeUdpError::from)
        .and_then(|resource| {
          let addr = ipv4_addr.ok_or_else(invalid_input)?;
          let iface = interface_address
            .as_deref()
            .map(Ipv4Addr::from_str)
            .transpose()?
            .unwrap_or(Ipv4Addr::UNSPECIFIED);
          resource.socket.join_multicast_v4(addr, iface)?;
          Ok(())
        })
    };
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[rename("dropMembership")]
  fn drop_membership(
    &self,
    state: &mut OpState,
    #[string] multicast_address: &str,
    #[string] interface_address: Option<String>,
  ) -> i32 {
    let ipv4_addr = Ipv4Addr::from_str(multicast_address).ok();
    let ipv6_addr = Ipv6Addr::from_str(multicast_address).ok();
    if ipv4_addr.is_none() && ipv6_addr.is_none() {
      return uv_compat::UV_EINVAL;
    }
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = if self.family.get().is_some_and(UdpFamily::is_ipv6) {
      state
        .resource_table
        .get::<NodeUdpSocketResource>(rid)
        .map_err(NodeUdpError::from)
        .and_then(|resource| {
          let addr = ipv6_addr.ok_or_else(invalid_input)?;
          let iface = resolve_ipv6_interface(interface_address.as_deref())?;
          resource.socket.leave_multicast_v6(&addr, iface)?;
          Ok(())
        })
    } else {
      state
        .resource_table
        .get::<NodeUdpSocketResource>(rid)
        .map_err(NodeUdpError::from)
        .and_then(|resource| {
          let addr = ipv4_addr.ok_or_else(invalid_input)?;
          let iface = interface_address
            .as_deref()
            .map(Ipv4Addr::from_str)
            .transpose()?
            .unwrap_or(Ipv4Addr::UNSPECIFIED);
          resource.socket.leave_multicast_v4(addr, iface)?;
          Ok(())
        })
    };
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[rename("addSourceSpecificMembership")]
  fn add_source_specific_membership(
    &self,
    state: &mut OpState,
    #[string] source_address: &str,
    #[string] group_address: &str,
    #[string] interface_address: Option<String>,
  ) -> i32 {
    let Ok(source_addr) = Ipv4Addr::from_str(source_address) else {
      return uv_compat::UV_EINVAL;
    };
    let Ok(group_addr) = Ipv4Addr::from_str(group_address) else {
      return uv_compat::UV_EINVAL;
    };
    let Ok(interface_addr) =
      Ipv4Addr::from_str(interface_address.as_deref().unwrap_or("0.0.0.0"))
    else {
      return uv_compat::UV_EINVAL;
    };
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        source_specific_multicast(
          &resource.socket,
          source_addr,
          group_addr,
          interface_addr,
          {
            #[cfg(unix)]
            {
              libc::IP_ADD_SOURCE_MEMBERSHIP
            }
            #[cfg(windows)]
            {
              windows_sys::Win32::Networking::WinSock::IP_ADD_SOURCE_MEMBERSHIP
            }
          },
        )
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[rename("dropSourceSpecificMembership")]
  fn drop_source_specific_membership(
    &self,
    state: &mut OpState,
    #[string] source_address: &str,
    #[string] group_address: &str,
    #[string] interface_address: Option<String>,
  ) -> i32 {
    let Ok(source_addr) = Ipv4Addr::from_str(source_address) else {
      return uv_compat::UV_EINVAL;
    };
    let Ok(group_addr) = Ipv4Addr::from_str(group_address) else {
      return uv_compat::UV_EINVAL;
    };
    let Ok(interface_addr) =
      Ipv4Addr::from_str(interface_address.as_deref().unwrap_or("0.0.0.0"))
    else {
      return uv_compat::UV_EINVAL;
    };
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        source_specific_multicast(
          &resource.socket,
          source_addr,
          group_addr,
          interface_addr,
          {
            #[cfg(unix)]
            {
              libc::IP_DROP_SOURCE_MEMBERSHIP
            }
            #[cfg(windows)]
            {
              windows_sys::Win32::Networking::WinSock::IP_DROP_SOURCE_MEMBERSHIP
            }
          },
        )
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[nofast]
  #[rename("setMulticastInterface")]
  fn set_multicast_interface(
    &self,
    state: &mut OpState,
    #[string] interface_address: &str,
  ) -> i32 {
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let is_ipv6 = self.family.get().is_some_and(UdpFamily::is_ipv6);
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
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
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[fast]
  #[rename("setMulticastLoopback")]
  fn set_multicast_loopback(&self, state: &mut OpState, #[smi] on: i32) -> i32 {
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let is_ipv4 = self.family.get().is_some_and(UdpFamily::is_ipv4);
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        if is_ipv4 {
          resource.socket.set_multicast_loop_v4(on == 1)?;
        } else {
          resource.socket.set_multicast_loop_v6(on == 1)?;
        }
        Ok(())
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[fast]
  #[rename("setMulticastTTL")]
  fn set_multicast_ttl(&self, state: &mut OpState, #[smi] ttl: i32) -> i32 {
    if !(1..=255).contains(&ttl) {
      return uv_compat::UV_EINVAL;
    }
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    if !self.family.get().is_some_and(UdpFamily::is_ipv4) {
      return 0;
    }
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        resource.socket.set_multicast_ttl_v4(ttl as u32)?;
        Ok(())
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[fast]
  #[rename("setTTL")]
  fn set_ttl(&self, state: &mut OpState, #[smi] ttl: i32) -> i32 {
    if !(1..=255).contains(&ttl) {
      return uv_compat::UV_EINVAL;
    }
    let Some(rid) = self.rid.get() else {
      return uv_compat::UV_EBADF;
    };
    let result = state
      .resource_table
      .get::<NodeUdpSocketResource>(rid)
      .map_err(NodeUdpError::from)
      .and_then(|resource| {
        let sock_ref = socket2::SockRef::from(&resource.socket);
        sock_ref.set_ttl(ttl as u32)?;
        Ok(())
      });
    match result {
      Ok(()) => 0,
      Err(err) => udp_error_to_uv(&err),
    }
  }

  #[fast]
  #[rename("_rid")]
  fn rid(&self) -> i32 {
    self.rid.get().map(|rid| rid as i32).unwrap_or(-1)
  }

  #[fast]
  #[rename("_recvBufferSize")]
  fn recv_buffer_size(&self) -> i32 {
    self.recv_buffer_size.get() as i32
  }

  #[fast]
  #[rename("_remotePort")]
  fn remote_port(&self) -> i32 {
    self.remote_port.get().map(i32::from).unwrap_or(-1)
  }

  #[string]
  #[rename("_remoteAddress")]
  fn remote_address(&self) -> Option<String> {
    self.remote_address.borrow().clone()
  }

  #[fast]
  #[rename("_closeResource")]
  fn close_resource(&self, state: &mut OpState) {
    self.address.borrow_mut().take();
    self.family.set(None);
    self.port.set(None);
    self.remote_address.borrow_mut().take();
    self.remote_family.set(None);
    self.remote_port.set(None);
    if let Some(rid) = self.rid.take() {
      #[allow(
        deprecated,
        reason = "ResourceTable::close is used for sync close"
      )]
      let _ = state.resource_table.close(rid);
    }
  }
}

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
  #[class(type)]
  #[error("Invalid UDP send buffer")]
  InvalidSendBuffer,
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
}

fn invalid_input() -> NodeUdpError {
  NodeUdpError::Io(std::io::Error::new(
    std::io::ErrorKind::InvalidInput,
    "invalid address",
  ))
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

fn node_udp_bind(
  state: &mut OpState,
  hostname: &str,
  port: u16,
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
#[serde]
pub fn op_node_udp_bind(
  state: &mut OpState,
  #[string] hostname: &str,
  #[smi] port: u16,
  reuse_address: bool,
  ipv6_only: bool,
) -> Result<(ResourceId, String, u16), NodeUdpError> {
  node_udp_bind(state, hostname, port, reuse_address, ipv6_only)
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
///
/// This mirrors libuv's `uv_ip6_addr()`, which Node.js uses to translate the
/// `multicastInterface` argument of `addMembership()`/`dropMembership()` into
/// an interface index (the `sin6_scope_id` of the parsed address):
///
/// - The interface is selected by the zone id following `%`. On Windows the
///   zone is a numeric index (`atoi`), while on Unix it is an interface *name*
///   resolved via `if_nametoindex()`.
/// - An unknown zone id resolves to index `0`, which the kernel treats as
///   "use the default interface". Node.js silently accepts such values rather
///   than failing, so `"::%12"` (no interface literally named `12`) joins on
///   the default interface instead of raising `EINVAL`.
///
/// As an extension over libuv, when no zone id is supplied we additionally try
/// to map a bare interface address (e.g. `"fe80::1"`) to its index by scanning
/// the local interfaces, falling back to the default interface.
fn resolve_ipv6_interface(
  interface_addr: Option<&str>,
) -> Result<u32, NodeUdpError> {
  let Some(addr_str) = interface_addr else {
    return Ok(0);
  };

  // Check if the address contains a zone ID (e.g. "fe80::1%eth0" or "::1%12")
  if let Some(zone_idx) = addr_str.find('%') {
    // The address part must be a valid IPv6 address (libuv calls
    // `uv_inet_pton` and reports `EINVAL` when it fails to parse).
    Ipv6Addr::from_str(&addr_str[..zone_idx])?;

    let zone_id = &addr_str[zone_idx + 1..];
    #[cfg(windows)]
    {
      // `atoi`-style parsing: a non-numeric zone yields the default interface.
      return Ok(zone_id.parse::<u32>().unwrap_or(0));
    }
    #[cfg(unix)]
    {
      use std::ffi::CString;
      // Resolve the zone as an interface name. `if_nametoindex` returns 0 for
      // unknown interfaces (including numeric strings that aren't names),
      // which is silently treated as the default interface.
      let idx = CString::new(zone_id)
        .ok()
        // SAFETY: if_nametoindex is safe to call with a valid C string
        .map(|name| unsafe { libc::if_nametoindex(name.as_ptr()) })
        .unwrap_or(0);
      return Ok(idx);
    }
    #[cfg(not(any(unix, windows)))]
    return Ok(0);
  }

  // No zone id was provided. Try to find interface by matching the address.
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

fn array_buffer_view_to_vec(view: v8::Local<v8::ArrayBufferView>) -> Vec<u8> {
  let mut buf = vec![0u8; view.byte_length()];
  let copied = view.copy_contents(&mut buf);
  debug_assert_eq!(copied, buf.len());
  buf
}

fn string_to_utf8(
  scope: &mut v8::PinScope,
  value: v8::Local<v8::String>,
) -> Vec<u8> {
  let len = value.utf8_length(scope);
  let mut buf = Vec::with_capacity(len);
  let written = value.write_utf8_uninit_v2(
    scope,
    buf.spare_capacity_mut(),
    v8::WriteFlags::kReplaceInvalidUtf8,
    None,
  );
  // SAFETY: write_utf8_uninit_v2 initialized exactly `written` bytes.
  unsafe {
    buf.set_len(written);
  }
  buf
}

fn udp_send_buffers_to_vec(
  scope: &mut v8::PinScope,
  bufs: v8::Local<v8::Array>,
  count: u32,
) -> Result<Vec<u8>, NodeUdpError> {
  let count = count.min(bufs.length());
  let mut payload = Vec::new();
  for i in 0..count {
    let Some(value) = bufs.get_index(scope, i) else {
      continue;
    };
    if let Ok(string) = v8::Local::<v8::String>::try_from(value) {
      payload.extend_from_slice(&string_to_utf8(scope, string));
      continue;
    }
    if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
      payload.extend_from_slice(&array_buffer_view_to_vec(view));
      continue;
    }
    return Err(NodeUdpError::InvalidSendBuffer);
  }
  Ok(payload)
}

async fn node_udp_send(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Vec<u8>,
  hostname: String,
  port: u16,
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

#[derive(Serialize)]
pub struct SendResult {
  pub err: Option<i32>,
  pub sent: usize,
}

#[op2]
#[serde]
pub fn op_node_udp_send<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  bufs: v8::Local<'a, v8::Array>,
  #[smi] count: u32,
  #[string] hostname: String,
  #[smi] port: u16,
) -> impl Future<Output = SendResult> + use<> {
  let payload = udp_send_buffers_to_vec(scope, bufs, count);
  async move {
    match payload {
      Ok(payload) => {
        match node_udp_send(state, rid, payload, hostname, port).await {
          Ok(sent) => SendResult { err: None, sent },
          Err(err) => SendResult {
            err: Some(udp_error_to_uv(&err)),
            sent: 0,
          },
        }
      }
      Err(err) => SendResult {
        err: Some(udp_error_to_uv(&err)),
        sent: 0,
      },
    }
  }
}

#[derive(serde::Serialize)]
pub struct RecvResult {
  pub nread: i32,
  pub hostname: Option<String>,
  pub port: Option<u16>,
  pub family: Option<&'static str>,
}

#[op2]
#[serde]
pub fn op_node_udp_recv(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] mut buf: JsBuffer,
) -> impl Future<Output = RecvResult> + use<> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NodeUdpSocketResource>(rid);
  async move {
    let resource = match resource {
      Ok(resource) => resource,
      Err(_) => {
        return RecvResult {
          nread: 0,
          hostname: None,
          port: None,
          family: None,
        };
      }
    };
    let cancel = RcRef::map(&resource, |r| &r.cancel);
    let result = resource.socket.recv_from(&mut buf).or_cancel(cancel).await;

    match result {
      Ok(Ok((nread, remote_addr))) => RecvResult {
        nread: nread as i32,
        hostname: Some(remote_addr.ip().to_string()),
        port: Some(remote_addr.port()),
        family: Some(if remote_addr.is_ipv6() {
          "IPv6"
        } else {
          "IPv4"
        }),
      },
      Ok(Err(_)) => RecvResult {
        nread: UV_UNKNOWN,
        hostname: None,
        port: None,
        family: None,
      },
      Err(_) => RecvResult {
        nread: 0,
        hostname: None,
        port: None,
        family: None,
      },
    }
  }
}

/// Return an owned dup of the bound UDP socket's file descriptor, for use as
/// the payload of an SCM_RIGHTS cmsg on an IPC channel.  The caller owns the
/// returned fd and must close it after `sendmsg` has attached it to the IPC
/// message.  Returns -1 on platforms that don't support fd-passing (Windows).
#[op2(fast)]
#[smi]
pub fn op_node_udp_fd_for_ipc(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<i32, NodeUdpError> {
  let resource = state.resource_table.get::<NodeUdpSocketResource>(rid)?;
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    let fd = resource.socket.as_raw_fd();
    if fd < 0 {
      return Ok(-1);
    }
    // SAFETY: fd is a valid open file descriptor. F_DUPFD_CLOEXEC
    // atomically dups and sets CLOEXEC, avoiding a race window.
    let dup = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    Ok(dup)
  }
  #[cfg(not(unix))]
  {
    let _ = resource;
    Ok(-1)
  }
}

/// Adopt an existing file descriptor as a UDP socket resource.  Used on the
/// receiving side of IPC handle passing.
fn node_udp_open(
  state: &mut OpState,
  fd: i32,
) -> Result<(ResourceId, String, u16), NodeUdpError> {
  #[cfg(unix)]
  {
    use std::os::unix::io::FromRawFd;
    // SAFETY: The fd was received via SCM_RIGHTS and is a valid, open socket.
    let std_socket = unsafe { std::net::UdpSocket::from_raw_fd(fd) };
    std_socket.set_nonblocking(true)?;
    let local_addr = std_socket.local_addr()?;
    let socket = UdpSocket::from_std(std_socket)?;
    let resource = NodeUdpSocketResource {
      socket,
      cancel: Default::default(),
    };
    let rid = state.resource_table.add(resource);
    Ok((rid, local_addr.ip().to_string(), local_addr.port()))
  }
  #[cfg(not(unix))]
  {
    let _ = (state, fd);
    Err(NodeUdpError::Io(std::io::Error::new(
      std::io::ErrorKind::Unsupported,
      "UDP socket IPC handle passing is not supported on this platform",
    )))
  }
}

/// Adopt an existing file descriptor as a UDP socket resource.  Used on the
/// receiving side of IPC handle passing.
#[op2]
#[serde]
pub fn op_node_udp_open(
  state: &mut OpState,
  #[smi] fd: i32,
) -> Result<(ResourceId, String, u16), NodeUdpError> {
  node_udp_open(state, fd)
}
