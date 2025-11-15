// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::rc::Rc;
use std::str::FromStr;

use deno_core::OpState;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_error::JsError;
use deno_net::ops::NetPermToken;
use deno_permissions::PermissionCheckError;
use hyper_util::client::legacy::connect::dns::GaiResolver;
use hyper_util::client::legacy::connect::dns::Name;
use socket2::SockAddr;
use tower_service::Service;

// https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/uv/include/uv/errno.h#L35-L48
#[allow(dead_code)]
const UV_EAI_ADDRFAMILY: i32 = -3000;
const UV_EAI_AGAIN: i32 = -3001;
const UV_EAI_BADFLAGS: i32 = -3002;
#[allow(dead_code)]
const UV_EAI_CANCELED: i32 = -3003;
const UV_EAI_FAIL: i32 = -3004;
const UV_EAI_FAMILY: i32 = -3005;
const UV_EAI_MEMORY: i32 = -3006;
#[allow(dead_code)]
const UV_EAI_NODATA: i32 = -3007;
const UV_EAI_NONAME: i32 = -3008;
const UV_EAI_OVERFLOW: i32 = -3009;
#[allow(dead_code)]
const UV_EAI_SERVICE: i32 = -3010;
#[allow(dead_code)]
const UV_EAI_SOCKTYPE: i32 = -3011;
#[allow(dead_code)]
const UV_EAI_BADHINTS: i32 = -3013;
#[allow(dead_code)]
const UV_EAI_PROTOCOL: i32 = -3014;

#[derive(Debug, thiserror::Error, JsError)]
pub enum DnsError {
  #[class(type)]
  #[error(transparent)]
  AddrParseError(#[from] std::net::AddrParseError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] PermissionCheckError),
  #[class(type)]
  #[error("Could not resolve the hostname \"{0}\"")]
  Resolution(String),
  #[class(inherit)]
  #[error("{0}")]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
  #[class(generic)]
  #[error("{0}")]
  #[property("code" = self.code())]
  RawSysErr(i32),
}

impl DnsError {
  fn code(&self) -> i32 {
    match self {
      Self::RawSysErr(code) => *code,
      _ => 0,
    }
  }
}

#[op2(async, stack_trace)]
#[cppgc]
pub async fn op_node_getaddrinfo<P>(
  state: Rc<RefCell<OpState>>,
  #[string] hostname: String,
  port: Option<u16>,
) -> Result<NetPermToken, DnsError>
where
  P: crate::NodePermissions + 'static,
{
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<P>();
    permissions.check_net((hostname.as_str(), port), "node:dns.lookup()")?;
  }

  let mut resolver = GaiResolver::new();
  let name = Name::from_str(&hostname)
    .map_err(|_| DnsError::Resolution(hostname.clone()))?;
  let resolved_ips = resolver
    .call(name)
    .await
    .map_err(|_| DnsError::Resolution(hostname.clone()))?
    .map(|addr| addr.ip().to_string())
    .collect::<Vec<_>>();
  Ok(NetPermToken {
    hostname,
    port,
    resolved_ips,
  })
}

#[op2(async, stack_trace)]
#[serde]
pub async fn op_node_getnameinfo<P>(
  state: Rc<RefCell<OpState>>,
  #[string] ip: String,
  #[smi] port: u16,
) -> Result<(String, String), DnsError>
where
  P: crate::NodePermissions + 'static,
{
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<P>();
    permissions
      .check_net((ip.as_str(), Some(port)), "node:dns.lookupService()")?;
  }

  let ip_addr: IpAddr = ip.parse()?;
  let socket_addr = SocketAddr::new(ip_addr, port);

  match spawn_blocking(move || getnameinfo(&socket_addr)).await {
    Ok(result) => result,
    Err(err) => Err(DnsError::Io(err.into())),
  }
}

fn getnameinfo(socket_addr: &SocketAddr) -> Result<(String, String), DnsError> {
  #[cfg(unix)]
  use libc::getnameinfo as libc_getnameinfo;
  
  let sock: SockAddr = (*socket_addr).into();
  let c_sockaddr = sock.as_ptr();
  let c_sockaddr_len = sock.len();

  let mut c_host = [0_u8; libc::NI_MAXHOST as usize];
  let mut c_service = [0_u8; 32];

  // SAFETY: Calling getnameinfo with valid parameters.
  let code = unsafe {
    libc_getnameinfo(
      c_sockaddr,
      c_sockaddr_len,
      c_host.as_mut_ptr() as *mut i8,
      c_host.len() as u32,
      c_service.as_mut_ptr() as *mut i8,
      c_service.len() as u32,
      libc::NI_NAMEREQD,
    )
  };

  assert_success(code)?;

  // SAFETY: c_host is initialized by getnameinfo on success.
  let host_cstr =
    unsafe { std::ffi::CStr::from_ptr(c_host.as_ptr() as *const i8) };
  // SAFETY: c_service is initialized by getnameinfo on success.
  let service_cstr =
    unsafe { std::ffi::CStr::from_ptr(c_service.as_ptr() as *const i8) };

  Ok((
    host_cstr.to_string_lossy().into_owned(),
    service_cstr.to_string_lossy().into_owned(),
  ))
}

#[cfg(unix)]
fn assert_success(code: i32) -> Result<(), DnsError> {
  if code == 0 {
    return Ok(());
  }

  let err = match code {
    libc::EAI_AGAIN => DnsError::RawSysErr(UV_EAI_AGAIN),
    libc::EAI_BADFLAGS => DnsError::RawSysErr(UV_EAI_BADFLAGS),
    libc::EAI_FAIL => DnsError::RawSysErr(UV_EAI_FAIL),
    libc::EAI_FAMILY => DnsError::RawSysErr(UV_EAI_FAMILY),
    libc::EAI_MEMORY => DnsError::RawSysErr(UV_EAI_MEMORY),
    libc::EAI_NONAME => DnsError::RawSysErr(UV_EAI_NONAME),
    libc::EAI_OVERFLOW => DnsError::RawSysErr(UV_EAI_OVERFLOW),
    libc::EAI_SYSTEM => DnsError::Io(std::io::Error::last_os_error()),
    _ => DnsError::RawSysErr(code),
  };

  Err(err)
}
