// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::rc::Rc;
#[cfg(target_family = "windows")]
use std::sync::OnceLock;

use deno_core::OpState;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_error::JsError;
use deno_net::ops::NetPermToken;
use deno_permissions::PermissionCheckError;
use deno_permissions::PermissionsContainer;
use socket2::SockAddr;

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
  #[property("uv_errcode" = self.code())]
  RawUvErr(i32),
  #[cfg(not(any(unix, windows)))]
  #[class(generic)]
  #[error("Unsupported platform.")]
  UnsupportedPlatform,
}

impl DnsError {
  fn code(&self) -> i32 {
    match self {
      Self::RawUvErr(code) => *code,
      _ => 0,
    }
  }
}

#[cfg(target_family = "windows")]
static WINSOCKET_INIT: OnceLock<i32> = OnceLock::new();

#[op2(stack_trace)]
#[cppgc]
pub async fn op_node_getaddrinfo(
  state: Rc<RefCell<OpState>>,
  #[string] hostname: String,
  port: Option<u16>,
  #[smi] family: i32,
) -> Result<NetPermToken, DnsError> {
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<PermissionsContainer>();
    permissions.check_net(&(hostname.as_str(), port), "node:dns.lookup()")?;
  }

  let hostname_clone = hostname.clone();
  let resolved_ips =
    spawn_blocking(move || getaddrinfo_inner(&hostname_clone, family))
      .await
      .map_err(|e| DnsError::Io(e.into()))??;

  Ok(NetPermToken {
    hostname,
    port,
    resolved_ips,
  })
}

fn getaddrinfo_inner(
  hostname: &str,
  family: i32,
) -> Result<Vec<String>, DnsError> {
  #[cfg(unix)]
  {
    use std::ffi::CString;

    let ai_family = match family {
      4 => libc::AF_INET,
      6 => libc::AF_INET6,
      _ => libc::AF_UNSPEC,
    };

    let c_hostname = CString::new(hostname)
      .map_err(|_| DnsError::Resolution(hostname.to_string()))?;

    let hints = libc::addrinfo {
      ai_flags: 0,
      ai_family,
      ai_socktype: libc::SOCK_STREAM,
      ai_protocol: 0,
      ai_addrlen: 0,
      ai_addr: std::ptr::null_mut(),
      ai_canonname: std::ptr::null_mut(),
      ai_next: std::ptr::null_mut(),
    };

    let mut result: *mut libc::addrinfo = std::ptr::null_mut();

    // SAFETY: Calling getaddrinfo with valid pointers
    let code = unsafe {
      libc::getaddrinfo(
        c_hostname.as_ptr(),
        std::ptr::null(),
        &hints,
        &mut result,
      )
    };

    if code != 0 {
      return Err(assert_success_err(code));
    }

    let mut ips = Vec::new();
    let mut current = result;
    while !current.is_null() {
      // SAFETY: current is a valid pointer from getaddrinfo linked list
      let addr = unsafe { &*current };
      let sock_addr =
        // SAFETY: ai_addr/ai_addrlen are valid from getaddrinfo
        unsafe { SockAddr::new(
          *(addr.ai_addr as *const libc::sockaddr_storage),
          addr.ai_addrlen,
        ) };
      if let Some(ip) = sock_addr.as_socket() {
        ips.push(ip.ip().to_string());
      }
      // SAFETY: Advancing to next node in getaddrinfo linked list
      current = unsafe { (*current).ai_next };
    }

    // SAFETY: Freeing the result from getaddrinfo
    unsafe { libc::freeaddrinfo(result) };

    if ips.is_empty() {
      return Err(DnsError::Resolution(hostname.to_string()));
    }

    Ok(ips)
  }

  #[cfg(windows)]
  {
    use winapi::shared::minwindef::MAKEWORD;
    use windows_sys::Win32::Networking::WinSock;

    let ai_family = match family {
      4 => WinSock::AF_INET as i32,
      6 => WinSock::AF_INET6 as i32,
      _ => WinSock::AF_UNSPEC as i32,
    };

    // SAFETY: winapi call
    let wsa_startup_code = *WINSOCKET_INIT.get_or_init(|| unsafe {
      let mut wsa_data: WinSock::WSADATA = std::mem::zeroed();
      WinSock::WSAStartup(MAKEWORD(2, 2), &mut wsa_data)
    });
    if wsa_startup_code != 0 {
      return Err(DnsError::Resolution(hostname.to_string()));
    }

    let c_hostname: Vec<u8> =
      hostname.bytes().chain(std::iter::once(0)).collect();

    let hints = WinSock::ADDRINFOA {
      ai_flags: 0,
      ai_family,
      ai_socktype: WinSock::SOCK_STREAM as _,
      ai_protocol: 0,
      ai_addrlen: 0,
      ai_canonname: std::ptr::null_mut(),
      ai_addr: std::ptr::null_mut(),
      ai_next: std::ptr::null_mut(),
    };

    let mut result: *mut WinSock::ADDRINFOA = std::ptr::null_mut();

    // SAFETY: Calling getaddrinfo with valid pointers
    let code = unsafe {
      WinSock::getaddrinfo(
        c_hostname.as_ptr() as _,
        std::ptr::null(),
        &hints,
        &mut result,
      )
    };

    if code != 0 {
      return Err(assert_success_err(code));
    }

    let mut ips = Vec::new();
    let mut current = result;
    // SAFETY: Iterating linked list returned by getaddrinfo
    while !current.is_null() {
      let addr = unsafe { &*current };
      if let Some(sock_addr) = SockAddr::try_init(|storage, len| {
        std::ptr::copy_nonoverlapping(
          addr.ai_addr as *const u8,
          storage as *mut u8,
          addr.ai_addrlen as usize,
        );
        *len = addr.ai_addrlen as _;
        Ok(())
      })
      .ok()
      .and_then(|(_, sa)| sa.as_socket())
      {
        ips.push(sock_addr.ip().to_string());
      }
      current = unsafe { (*current).ai_next };
    }

    // SAFETY: Freeing the result from getaddrinfo
    unsafe { WinSock::freeaddrinfo(result) };

    if ips.is_empty() {
      return Err(DnsError::Resolution(hostname.to_string()));
    }

    Ok(ips)
  }

  #[cfg(not(any(unix, windows)))]
  {
    let _ = (hostname, family);
    Err(DnsError::UnsupportedPlatform)
  }
}

#[op2(stack_trace)]
pub async fn op_node_getnameinfo(
  state: Rc<RefCell<OpState>>,
  #[string] ip: String,
  #[smi] port: u16,
) -> Result<(String, String), DnsError> {
  {
    let mut state_ = state.borrow_mut();
    let permissions = state_.borrow_mut::<PermissionsContainer>();
    permissions
      .check_net(&(ip.as_str(), Some(port)), "node:dns.lookupService()")?;
  }

  let ip_addr: IpAddr = ip.parse()?;
  let socket_addr = SocketAddr::new(ip_addr, port);

  match spawn_blocking(move || getnameinfo(socket_addr)).await {
    Ok(result) => result,
    Err(err) => Err(DnsError::Io(err.into())),
  }
}

fn getnameinfo(socket_addr: SocketAddr) -> Result<(String, String), DnsError> {
  let sock: SockAddr = socket_addr.into();
  let c_sockaddr = sock.as_ptr();
  let c_sockaddr_len = sock.len();
  #[cfg(unix)]
  {
    const NI_MAXSERV: u32 = 32;

    let mut c_host = [0_u8; libc::NI_MAXHOST as usize];
    let mut c_service = [0_u8; NI_MAXSERV as usize];

    // SAFETY: Calling getnameinfo
    let code = unsafe {
      libc::getnameinfo(
        c_sockaddr,
        c_sockaddr_len,
        c_host.as_mut_ptr() as _,
        c_host.len() as _,
        c_service.as_mut_ptr() as _,
        c_service.len() as _,
        libc::NI_NAMEREQD as _,
      )
    };
    assert_success(code)?;

    // SAFETY: c_host is initialized by getnameinfo on success.
    let host_cstr = unsafe { std::ffi::CStr::from_ptr(c_host.as_ptr() as _) };
    // SAFETY: c_service is initialized by getnameinfo on success.
    let service_cstr =
      unsafe { std::ffi::CStr::from_ptr(c_service.as_ptr() as _) };

    Ok((
      host_cstr.to_string_lossy().into_owned(),
      service_cstr.to_string_lossy().into_owned(),
    ))
  }
  #[cfg(windows)]
  {
    use std::os::windows::ffi::OsStringExt;

    use winapi::shared::minwindef::MAKEWORD;
    use windows_sys::Win32::Networking::WinSock;

    // SAFETY: winapi call
    let wsa_startup_code = *WINSOCKET_INIT.get_or_init(|| unsafe {
      let mut wsa_data: WinSock::WSADATA = std::mem::zeroed();
      WinSock::WSAStartup(MAKEWORD(2, 2), &mut wsa_data)
    });
    assert_success(wsa_startup_code)?;

    let mut c_host = [0_u16; WinSock::NI_MAXHOST as usize];
    let mut c_service = [0_u16; WinSock::NI_MAXSERV as usize];

    // SAFETY: Calling getnameinfo
    let code = unsafe {
      WinSock::GetNameInfoW(
        c_sockaddr as _,
        c_sockaddr_len,
        c_host.as_mut_ptr() as _,
        c_host.len() as _,
        c_service.as_mut_ptr() as _,
        c_service.len() as _,
        WinSock::NI_NAMEREQD as _,
      )
    };
    assert_success(code)?;

    let host_str_len = c_host.iter().take_while(|&&c| c != 0).count();
    let host_str = std::ffi::OsString::from_wide(&c_host[..host_str_len])
      .to_string_lossy()
      .into_owned();

    let service_str_len = c_service.iter().take_while(|&&c| c != 0).count();
    let service_str =
      std::ffi::OsString::from_wide(&c_service[..service_str_len])
        .to_string_lossy()
        .into_owned();

    Ok((host_str, service_str))
  }
  #[cfg(not(any(unix, windows)))]
  {
    Err(DnsError::UnsupportedPlatform)
  }
}

#[cfg(any(unix, windows))]
fn assert_success_err(code: i32) -> DnsError {
  #[cfg(windows)]
  use windows_sys::Win32::Networking::WinSock;

  use crate::ops::constant;

  #[cfg(unix)]
  let err = match code {
    libc::EAI_AGAIN => DnsError::RawUvErr(constant::UV_EAI_AGAIN),
    libc::EAI_BADFLAGS => DnsError::RawUvErr(constant::UV_EAI_BADFLAGS),
    libc::EAI_FAIL => DnsError::RawUvErr(constant::UV_EAI_FAIL),
    libc::EAI_FAMILY => DnsError::RawUvErr(constant::UV_EAI_FAMILY),
    libc::EAI_MEMORY => DnsError::RawUvErr(constant::UV_EAI_MEMORY),
    libc::EAI_NONAME => DnsError::RawUvErr(constant::UV_EAI_NONAME),
    libc::EAI_OVERFLOW => DnsError::RawUvErr(constant::UV_EAI_OVERFLOW),
    libc::EAI_SYSTEM => DnsError::Io(std::io::Error::last_os_error()),
    _ => DnsError::Io(std::io::Error::from_raw_os_error(code)),
  };

  #[cfg(windows)]
  let err = match code {
    WinSock::WSATRY_AGAIN => DnsError::RawUvErr(constant::UV_EAI_AGAIN),
    WinSock::WSAEINVAL => DnsError::RawUvErr(constant::UV_EAI_BADFLAGS),
    WinSock::WSANO_RECOVERY => DnsError::RawUvErr(constant::UV_EAI_FAIL),
    WinSock::WSAEAFNOSUPPORT => DnsError::RawUvErr(constant::UV_EAI_FAMILY),
    WinSock::WSA_NOT_ENOUGH_MEMORY => {
      DnsError::RawUvErr(constant::UV_EAI_MEMORY)
    }
    WinSock::WSAHOST_NOT_FOUND => DnsError::RawUvErr(constant::UV_EAI_NONAME),
    WinSock::WSATYPE_NOT_FOUND => DnsError::RawUvErr(constant::UV_EAI_SERVICE),
    WinSock::WSAESOCKTNOSUPPORT => {
      DnsError::RawUvErr(constant::UV_EAI_SOCKTYPE)
    }
    _ => DnsError::Io(std::io::Error::from_raw_os_error(code)),
  };

  err
}

#[cfg(any(unix, windows))]
fn assert_success(code: i32) -> Result<(), DnsError> {
  if code == 0 {
    return Ok(());
  }
  Err(assert_success_err(code))
}
