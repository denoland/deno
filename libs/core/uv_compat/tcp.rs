// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::VecDeque;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ffi::c_void;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

#[cfg(unix)]
pub(crate) use libc::AF_INET;
#[cfg(unix)]
use libc::AF_INET6;
#[cfg(unix)]
pub(crate) use libc::sockaddr_in;
#[cfg(unix)]
use libc::sockaddr_in6;
#[cfg(unix)]
type sa_family_t = libc::sa_family_t;
#[cfg(windows)]
pub(crate) use win_sock::AF_INET;
#[cfg(windows)]
use win_sock::AF_INET6;
#[cfg(windows)]
pub(crate) use win_sock::sockaddr_in;
#[cfg(windows)]
use win_sock::sockaddr_in6;

use crate::uv_compat::UV_EADDRINUSE;
use crate::uv_compat::UV_EAGAIN;
use crate::uv_compat::UV_EALREADY;
use crate::uv_compat::UV_ECANCELED;
use crate::uv_compat::UV_EINVAL;
use crate::uv_compat::UV_ENOBUFS;
use crate::uv_compat::UV_ENOTCONN;
use crate::uv_compat::UV_EOF;
use crate::uv_compat::UV_EPIPE;
use crate::uv_compat::UV_HANDLE_ACTIVE;
use crate::uv_compat::UV_HANDLE_REF;
use crate::uv_compat::get_inner;
use crate::uv_compat::uv_alloc_cb;
use crate::uv_compat::uv_buf_t;
use crate::uv_compat::uv_connect_cb;
use crate::uv_compat::uv_connect_t;
use crate::uv_compat::uv_connection_cb;
use crate::uv_compat::uv_handle_t;
use crate::uv_compat::uv_handle_type;
use crate::uv_compat::uv_loop_t;
use crate::uv_compat::uv_read_cb;
use crate::uv_compat::uv_shutdown_cb;
use crate::uv_compat::uv_shutdown_t;
use crate::uv_compat::uv_stream_t;
use crate::uv_compat::uv_write_cb;
use crate::uv_compat::uv_write_t;
#[cfg(windows)]
type sa_family_t = win_sock::sa_family_t;

// libc doesn't export socket structs on Windows.
#[cfg(windows)]
mod win_sock {
  #[repr(C)]
  pub struct in_addr {
    pub s_addr: u32,
  }
  #[repr(C)]
  pub struct sockaddr_in {
    pub sin_family: u16,
    pub sin_port: u16,
    pub sin_addr: in_addr,
    pub sin_zero: [u8; 8],
  }
  #[repr(C)]
  pub struct in6_addr {
    pub s6_addr: [u8; 16],
  }
  #[repr(C)]
  pub struct sockaddr_in6 {
    pub sin6_family: u16,
    pub sin6_port: u16,
    pub sin6_flowinfo: u32,
    pub sin6_addr: in6_addr,
    pub sin6_scope_id: u32,
  }
  pub const AF_INET: i32 = 2;
  pub const AF_INET6: i32 = 23;
  pub type sa_family_t = u16;
}
#[repr(C)]
pub struct uv_tcp_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  #[cfg(unix)]
  pub(crate) internal_fd: Option<std::os::unix::io::RawFd>,
  #[cfg(windows)]
  pub(crate) internal_fd: Option<std::os::windows::io::RawSocket>,
  pub(crate) internal_bind_addr: Option<SocketAddr>,
  /// Pre-created socket from `uv_tcp_bind`. Consumed by `uv_listen` or
  /// `uv_tcp_connect` so the same fd is used throughout the lifecycle,
  /// preserving any socket options set between bind and listen/connect.
  pub(crate) internal_socket: Option<tokio::net::TcpSocket>,
  /// Deferred bind error (e.g. EADDRINUSE). Reported from listen/connect,
  /// matching libuv's `delayed_error` semantics.
  pub(crate) internal_delayed_error: c_int,
  pub(crate) internal_stream: Option<tokio::net::TcpStream>,
  pub(crate) internal_listener: Option<tokio::net::TcpListener>,
  pub(crate) internal_listener_addr: Option<SocketAddr>,
  pub(crate) internal_nodelay: bool,
  pub(crate) internal_alloc_cb: Option<uv_alloc_cb>,
  pub(crate) internal_read_cb: Option<uv_read_cb>,
  pub(crate) internal_reading: bool,
  pub(crate) internal_connect: Option<ConnectPending>,
  pub(crate) internal_write_queue: VecDeque<WritePending>,
  pub(crate) internal_connection_cb: Option<uv_connection_cb>,
  pub(crate) internal_backlog: VecDeque<tokio::net::TcpStream>,
  pub(crate) internal_shutdown: Option<ShutdownPending>,
}

/// In-flight TCP connect operation.
///
/// # Safety
/// `req` is a raw pointer to a caller-owned `uv_connect_t`. The caller must
/// ensure it remains valid until the connect callback fires (at which point
/// `ConnectPending` is consumed). This struct is `!Send` -- it lives on the
/// event loop thread alongside `UvLoopInner`.
pub(crate) struct ConnectPending {
  pub(crate) future:
    Pin<Box<dyn Future<Output = std::io::Result<tokio::net::TcpStream>>>>,
  pub(crate) req: *mut uv_connect_t,
  pub(crate) cb: Option<uv_connect_cb>,
}

/// Queued write operation waiting for the socket to become writable.
///
/// # Safety
/// `req` is a raw pointer to a caller-owned `uv_write_t`. The caller must
/// ensure it remains valid until the write callback fires (at which point
/// `WritePending` is consumed). This struct is `!Send`.
pub(crate) struct WritePending {
  pub(crate) req: *mut uv_write_t,
  pub(crate) data: Vec<u8>,
  pub(crate) offset: usize,
  pub(crate) cb: Option<uv_write_cb>,
  /// Pre-determined completion status. When `Some`, the write is already
  /// complete and the callback should be fired with this status without
  /// attempting any I/O. This is used to defer synchronous write
  /// completions to the event loop, preventing re-entrancy issues.
  pub(crate) status: Option<c_int>,
}

/// Pending shutdown request, deferred until the write queue drains.
///
/// # Safety
/// `req` is a raw pointer to a caller-owned `uv_shutdown_t`. The caller must
/// ensure it remains valid until the shutdown callback fires.
pub(crate) struct ShutdownPending {
  pub(crate) req: *mut uv_shutdown_t,
  pub(crate) cb: Option<uv_shutdown_cb>,
}

// Re-export from parent module for backwards compatibility within crate.
pub(crate) use super::io_error_to_uv;

/// ### Safety
/// `addr` must point to a valid `sockaddr_in` or `sockaddr_in6` with correct `sa_family`.
unsafe fn sockaddr_to_std(addr: *const c_void) -> Option<SocketAddr> {
  let sa = addr as *const libc::sockaddr;
  // SAFETY: Caller guarantees addr points to a valid sockaddr.
  let family = unsafe { (*sa).sa_family as i32 };
  if family == AF_INET {
    // SAFETY: Family is AF_INET so addr is a valid sockaddr_in.
    let sin = unsafe { &*(addr as *const sockaddr_in) };
    let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
    let port = u16::from_be(sin.sin_port);
    Some(SocketAddr::from((ip, port)))
  } else if family == AF_INET6 {
    // SAFETY: Family is AF_INET6 so addr is a valid sockaddr_in6.
    let sin6 = unsafe { &*(addr as *const sockaddr_in6) };
    let ip = std::net::Ipv6Addr::from(sin6.sin6_addr.s6_addr);
    let port = u16::from_be(sin6.sin6_port);
    Some(SocketAddr::from((ip, port)))
  } else {
    None
  }
}

/// ### Safety
/// `out` must be writable and large enough for `sockaddr_in` or `sockaddr_in6`.
/// `len` must be a valid, writable pointer.
unsafe fn std_to_sockaddr(addr: SocketAddr, out: *mut c_void, len: *mut c_int) {
  match addr {
    SocketAddr::V4(v4) => {
      let sin = out as *mut sockaddr_in;
      // SAFETY: Caller guarantees out is large enough for sockaddr_in.
      unsafe {
        std::ptr::write_bytes(sin, 0, 1);
        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        {
          (*sin).sin_len = std::mem::size_of::<sockaddr_in>() as u8;
        }
        (*sin).sin_family = AF_INET as sa_family_t;
        (*sin).sin_port = v4.port().to_be();
        (*sin).sin_addr.s_addr = u32::from(*v4.ip()).to_be();
        *len = std::mem::size_of::<sockaddr_in>() as c_int;
      }
    }
    SocketAddr::V6(v6) => {
      let sin6 = out as *mut sockaddr_in6;
      // SAFETY: Caller guarantees out is large enough for sockaddr_in6.
      unsafe {
        std::ptr::write_bytes(sin6, 0, 1);
        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        {
          (*sin6).sin6_len = std::mem::size_of::<sockaddr_in6>() as u8;
        }
        (*sin6).sin6_family = AF_INET6 as sa_family_t;
        (*sin6).sin6_port = v6.port().to_be();
        (*sin6).sin6_addr.s6_addr = v6.ip().octets();
        (*sin6).sin6_scope_id = v6.scope_id();
        *len = std::mem::size_of::<sockaddr_in6>() as c_int;
      }
    }
  }
}

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `tcp` must be a valid, writable pointer.
pub unsafe fn uv_tcp_init(loop_: *mut uv_loop_t, tcp: *mut uv_tcp_t) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid.
  unsafe {
    use std::ptr::addr_of_mut;
    use std::ptr::write;
    write(addr_of_mut!((*tcp).r#type), uv_handle_type::UV_TCP);
    write(addr_of_mut!((*tcp).loop_), loop_);
    write(addr_of_mut!((*tcp).data), std::ptr::null_mut());
    write(addr_of_mut!((*tcp).flags), UV_HANDLE_REF);
    write(addr_of_mut!((*tcp).internal_fd), None);
    write(addr_of_mut!((*tcp).internal_bind_addr), None);
    write(addr_of_mut!((*tcp).internal_socket), None);
    write(addr_of_mut!((*tcp).internal_delayed_error), 0);
    write(addr_of_mut!((*tcp).internal_stream), None);
    write(addr_of_mut!((*tcp).internal_listener), None);
    write(addr_of_mut!((*tcp).internal_listener_addr), None);
    write(addr_of_mut!((*tcp).internal_nodelay), false);
    write(addr_of_mut!((*tcp).internal_alloc_cb), None);
    write(addr_of_mut!((*tcp).internal_read_cb), None);
    write(addr_of_mut!((*tcp).internal_reading), false);
    write(addr_of_mut!((*tcp).internal_connect), None);
    write(addr_of_mut!((*tcp).internal_write_queue), VecDeque::new());
    write(addr_of_mut!((*tcp).internal_connection_cb), None);
    write(addr_of_mut!((*tcp).internal_backlog), VecDeque::new());
    write(addr_of_mut!((*tcp).internal_shutdown), None);
  }
  0
}

/// ### Safety
/// `tcp` must be a valid pointer to a `uv_tcp_t` initialized by `uv_tcp_init`.
/// `fd` must be a valid, open file descriptor / socket.
pub unsafe fn uv_tcp_open(tcp: *mut uv_tcp_t, fd: c_int) -> c_int {
  // SAFETY: Caller guarantees tcp is initialized and fd is valid.
  unsafe {
    #[cfg(unix)]
    let std_stream = {
      use std::os::unix::io::FromRawFd;
      let s = std::net::TcpStream::from_raw_fd(fd);
      (*tcp).internal_fd = Some(fd);
      s
    };
    #[cfg(windows)]
    let std_stream = {
      use std::os::windows::io::FromRawSocket;
      let sock = fd as std::os::windows::io::RawSocket;
      let s = std::net::TcpStream::from_raw_socket(sock);
      (*tcp).internal_fd = Some(sock);
      s
    };
    std_stream.set_nonblocking(true).ok();
    match tokio::net::TcpStream::from_std(std_stream) {
      Ok(stream) => {
        if (*tcp).internal_nodelay {
          stream.set_nodelay(true).ok();
        }
        (*tcp).internal_stream = Some(stream);
        0
      }
      Err(_) => UV_EINVAL,
    }
  }
}

/// ### Safety
/// `tcp` must be initialized by `uv_tcp_init`. `addr` must point to a valid sockaddr.
pub unsafe fn uv_tcp_bind(
  tcp: *mut uv_tcp_t,
  addr: *const c_void,
  _addrlen: u32,
  _flags: u32,
) -> c_int {
  // SAFETY: Caller guarantees addr points to a valid sockaddr.
  let sock_addr = unsafe { sockaddr_to_std(addr) };
  let sa = match sock_addr {
    Some(sa) => sa,
    None => return UV_EINVAL,
  };

  // SAFETY: Caller guarantees tcp is valid and initialized.
  unsafe {
    // Match libuv: create the real socket and bind immediately rather than
    // deferring. This preserves socket identity so options set between bind
    // and listen/connect are retained on the same fd.
    let socket = if sa.is_ipv4() {
      match tokio::net::TcpSocket::new_v4() {
        Ok(s) => s,
        Err(ref e) => return io_error_to_uv(e),
      }
    } else {
      match tokio::net::TcpSocket::new_v6() {
        Ok(s) => s,
        Err(ref e) => return io_error_to_uv(e),
      }
    };

    // Match libuv: on Unix, set SO_REUSEADDR before bind so TIME_WAIT
    // sockets don't block rebinding.
    #[cfg(unix)]
    socket.set_reuseaddr(true).ok();

    // Match libuv: on Windows, set SO_EXCLUSIVEADDRUSE to prevent other
    // sockets from binding to the same port. This is the Windows equivalent
    // of the default Unix behavior (without SO_REUSEADDR's Windows semantics
    // which would allow port sharing).
    #[cfg(windows)]
    {
      use std::os::windows::io::AsRawSocket;
      unsafe extern "system" {
        fn setsockopt(
          s: usize,
          level: c_int,
          optname: c_int,
          optval: *const c_void,
          optlen: c_int,
        ) -> c_int;
      }
      const SOL_SOCKET: c_int = 0xffff;
      const SO_EXCLUSIVEADDRUSE: c_int = -5; // ~SO_REUSEADDR
      let one: c_int = 1;
      setsockopt(
        socket.as_raw_socket() as usize,
        SOL_SOCKET,
        SO_EXCLUSIVEADDRUSE,
        &one as *const c_int as *const c_void,
        std::mem::size_of::<c_int>() as c_int,
      );
    }

    // Store the raw fd so uv_tcp_nodelay etc. can
    // access the socket before it becomes a stream/listener.
    #[cfg(unix)]
    {
      use std::os::unix::io::AsRawFd;
      (*tcp).internal_fd = Some(socket.as_raw_fd());
    }
    #[cfg(windows)]
    {
      use std::os::windows::io::AsRawSocket;
      (*tcp).internal_fd = Some(socket.as_raw_socket());
    }

    match socket.bind(sa) {
      Ok(()) => {
        (*tcp).internal_delayed_error = 0;
      }
      Err(ref e) => {
        // Match libuv: EADDRINUSE is deferred (reported from listen/connect).
        // Other bind errors are returned immediately.
        if e.kind() == std::io::ErrorKind::AddrInUse {
          (*tcp).internal_delayed_error = UV_EADDRINUSE;
        } else {
          // Drop the socket (closes the fd) on real error.
          (*tcp).internal_fd = None;
          return io_error_to_uv(e);
        }
      }
    }

    (*tcp).internal_bind_addr = Some(sa);
    (*tcp).internal_socket = Some(socket);
  }
  0
}

/// ### Safety
/// `req` must be a valid, writable pointer. `tcp` must be initialized by `uv_tcp_init`.
/// `addr` must point to a valid sockaddr. `req` must remain valid until the connect callback fires.
pub unsafe fn uv_tcp_connect(
  req: *mut uv_connect_t,
  tcp: *mut uv_tcp_t,
  addr: *const c_void,
  cb: Option<uv_connect_cb>,
) -> c_int {
  // SAFETY: Caller guarantees addr points to a valid sockaddr.
  let sock_addr = unsafe { sockaddr_to_std(addr) };
  let sock_addr = match sock_addr {
    Some(sa) => sa,
    None => return UV_EINVAL,
  };

  // SAFETY: Caller guarantees req and tcp are valid.
  unsafe {
    (*req).handle = tcp as *mut uv_stream_t;
  }

  // SAFETY: tcp was initialized by uv_tcp_init which set loop_.
  let inner = unsafe { get_inner((*tcp).loop_) };

  // SAFETY: Caller guarantees tcp is valid and initialized.
  unsafe {
    // Match libuv: reject if a connect is already in progress.
    if (*tcp).internal_connect.is_some() {
      return UV_EALREADY;
    }

    // Match libuv: report deferred bind errors (e.g. EADDRINUSE).
    if (*tcp).internal_delayed_error != 0 {
      let err = (*tcp).internal_delayed_error;
      (*tcp).internal_delayed_error = 0;
      return err;
    }

    (*tcp).flags |= UV_HANDLE_ACTIVE;
    let mut handles = inner.tcp_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
      handles.push(tcp);
    }

    // Take the pre-created socket from bind (if any). This preserves
    // socket identity so options set between bind and connect are retained
    // on the same fd, matching libuv's behavior.
    let socket = (*tcp).internal_socket.take();
    (*tcp).internal_connect = Some(ConnectPending {
      future: Box::pin(async move {
        let socket = match socket {
          Some(s) => s,
          None => {
            // No prior bind — create a fresh socket matching the
            // target address family.
            if sock_addr.is_ipv4() {
              tokio::net::TcpSocket::new_v4()?
            } else {
              tokio::net::TcpSocket::new_v6()?
            }
          }
        };
        socket.connect(sock_addr).await
      }),
      req,
      cb,
    });
  }

  0
}

/// ### Safety
/// `tcp` must be a valid pointer to a `uv_tcp_t` initialized by `uv_tcp_init`.
pub unsafe fn uv_tcp_nodelay(tcp: *mut uv_tcp_t, enable: c_int) -> c_int {
  // SAFETY: Caller guarantees tcp is valid and initialized.
  unsafe {
    let enabled = enable != 0;
    (*tcp).internal_nodelay = enabled;
    if let Some(ref stream) = (*tcp).internal_stream {
      if stream.set_nodelay(enabled).is_err() {
        return UV_EINVAL;
      }
    } else if (*tcp).internal_fd.is_some() {
      // Socket exists from bind but isn't a stream yet. Apply nodelay
      // on the raw fd so the option is preserved, matching libuv's
      // uv__stream_open which applies TCP_NODELAY on the socket fd.
      let on: c_int = if enabled { 1 } else { 0 };
      #[cfg(unix)]
      {
        let fd = (*tcp).internal_fd.unwrap();
        if libc::setsockopt(
          fd,
          libc::IPPROTO_TCP,
          libc::TCP_NODELAY,
          &on as *const c_int as *const c_void,
          std::mem::size_of::<c_int>() as libc::socklen_t,
        ) != 0
        {
          return UV_EINVAL;
        }
      }
      #[cfg(windows)]
      {
        // On Windows, nodelay will be applied when the stream is created.
        // TcpSocket doesn't expose setsockopt directly.
        let _ = on;
      }
    }
  }
  0
}

/// ### Safety
/// `tcp` must be initialized by `uv_tcp_init`. `name` must be writable and large enough
/// for a sockaddr. `namelen` must be a valid, writable pointer.
pub unsafe fn uv_tcp_getpeername(
  tcp: *const uv_tcp_t,
  name: *mut c_void,
  namelen: *mut c_int,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    if let Some(ref stream) = (*tcp).internal_stream {
      match stream.peer_addr() {
        Ok(addr) => {
          std_to_sockaddr(addr, name, namelen);
          0
        }
        Err(_) => UV_ENOTCONN,
      }
    } else {
      UV_ENOTCONN
    }
  }
}

/// ### Safety
/// `tcp` must be initialized by `uv_tcp_init`. `name` must be writable and large enough
/// for a sockaddr. `namelen` must be a valid, writable pointer.
pub unsafe fn uv_tcp_getsockname(
  tcp: *const uv_tcp_t,
  name: *mut c_void,
  namelen: *mut c_int,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    if let Some(ref stream) = (*tcp).internal_stream {
      match stream.local_addr() {
        Ok(addr) => {
          std_to_sockaddr(addr, name, namelen);
          return 0;
        }
        Err(_) => return UV_EINVAL,
      }
    }
    if let Some(addr) = (*tcp).internal_listener_addr {
      std_to_sockaddr(addr, name, namelen);
      return 0;
    }
    // Query the pre-created socket's actual bound address. This correctly
    // resolves ephemeral port 0 to the real port assigned by the OS.
    if let Some(ref socket) = (*tcp).internal_socket
      && let Ok(addr) = socket.local_addr()
    {
      std_to_sockaddr(addr, name, namelen);
      return 0;
    }
    if let Some(addr) = (*tcp).internal_bind_addr {
      std_to_sockaddr(addr, name, namelen);
      return 0;
    }
    UV_EINVAL
  }
}

/// ### Safety
/// `_tcp` must be a valid pointer to a `uv_tcp_t` initialized by `uv_tcp_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_tcp_keepalive(
  _tcp: *mut uv_tcp_t,
  _enable: c_int,
  _delay: c_uint,
) -> c_int {
  // Keepalive is a no-op: tokio's TcpStream doesn't expose SO_KEEPALIVE
  // configuration in a cross-platform way, and nghttp2 only uses this
  // as a best-effort hint.
  0
}

/// ### Safety
/// `_tcp` must be a valid pointer to a `uv_tcp_t` initialized by `uv_tcp_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_tcp_simultaneous_accepts(
  _tcp: *mut uv_tcp_t,
  _enable: c_int,
) -> c_int {
  0 // no-op
}

/// ### Safety
/// `ip` must be a valid, null-terminated C string. `addr` must be a valid, writable pointer.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_ip4_addr(
  ip: *const c_char,
  port: c_int,
  addr: *mut sockaddr_in,
) -> c_int {
  // SAFETY: Caller guarantees ip is a valid C string and addr is writable.
  unsafe {
    let c_str = std::ffi::CStr::from_ptr(ip);
    let Ok(s) = c_str.to_str() else {
      return UV_EINVAL;
    };
    let Ok(ip_addr) = s.parse::<std::net::Ipv4Addr>() else {
      return UV_EINVAL;
    };
    std::ptr::write_bytes(addr, 0, 1);
    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    {
      (*addr).sin_len = std::mem::size_of::<sockaddr_in>() as u8;
    }
    (*addr).sin_family = AF_INET as sa_family_t;
    (*addr).sin_port = (port as u16).to_be();
    (*addr).sin_addr.s_addr = u32::from(ip_addr).to_be();
    0
  }
}

/// ### Safety
/// `stream` must be a valid pointer to a `uv_tcp_t` (cast as `uv_stream_t`) initialized
/// by `uv_tcp_init`, with a bind address set via `uv_tcp_bind`.
pub unsafe fn uv_listen(
  stream: *mut uv_stream_t,
  backlog: c_int,
  cb: Option<uv_connection_cb>,
) -> c_int {
  // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
  unsafe {
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;

    // Match libuv: report deferred bind errors.
    if tcp_ref.internal_delayed_error != 0 {
      let err = tcp_ref.internal_delayed_error;
      tcp_ref.internal_delayed_error = 0;
      return err;
    }

    let effective_backlog = if backlog > 0 { backlog as u32 } else { 128 };

    // Take the pre-created socket from bind (if any). This preserves socket
    // identity so options set between bind and listen are retained on the
    // same fd, matching libuv's behavior.
    let socket = match tcp_ref.internal_socket.take() {
      Some(s) => s,
      None => {
        // No prior bind — create a socket and bind to 0.0.0.0:0,
        // matching libuv's implicit bind in uv__tcp_listen.
        let bind_addr = tcp_ref
          .internal_bind_addr
          .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());
        let s = if bind_addr.is_ipv4() {
          match tokio::net::TcpSocket::new_v4() {
            Ok(s) => s,
            Err(ref e) => return io_error_to_uv(e),
          }
        } else {
          match tokio::net::TcpSocket::new_v6() {
            Ok(s) => s,
            Err(ref e) => return io_error_to_uv(e),
          }
        };
        s.set_reuseaddr(true).ok();
        if let Err(ref e) = s.bind(bind_addr) {
          return io_error_to_uv(e);
        }
        s
      }
    };

    let tokio_listener = match socket.listen(effective_backlog) {
      Ok(l) => l,
      Err(ref e) => return io_error_to_uv(e),
    };

    let listener_addr = tokio_listener.local_addr().ok();
    tcp_ref.internal_listener = Some(tokio_listener);
    tcp_ref.internal_listener_addr = listener_addr;
    tcp_ref.internal_connection_cb = cb;
    tcp_ref.flags |= UV_HANDLE_ACTIVE;

    let inner = get_inner(tcp_ref.loop_);
    let mut handles = inner.tcp_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
      handles.push(tcp);
    }
  }
  0
}

/// ### Safety
/// `server` must be a listening `uv_tcp_t`. `client` must be initialized by `uv_tcp_init`.
pub unsafe fn uv_accept(
  server: *mut uv_stream_t,
  client: *mut uv_stream_t,
) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid, initialized uv_tcp_t handles.
  unsafe {
    let server_tcp = &mut *(server as *mut uv_tcp_t);
    let client_tcp = &mut *(client as *mut uv_tcp_t);

    match server_tcp.internal_backlog.pop_front() {
      Some(stream) => {
        if client_tcp.internal_nodelay {
          stream.set_nodelay(true).ok();
        }
        client_tcp.internal_stream = Some(stream);
        0
      }
      None => UV_EAGAIN,
    }
  }
}

pub fn new_tcp() -> uv_tcp_t {
  uv_tcp_t {
    r#type: uv_handle_type::UV_TCP,
    loop_: std::ptr::null_mut(),
    data: std::ptr::null_mut(),
    flags: 0,
    internal_fd: None,
    internal_bind_addr: None,
    internal_socket: None,
    internal_delayed_error: 0,
    internal_stream: None,
    internal_listener: None,
    internal_listener_addr: None,
    internal_nodelay: false,
    internal_alloc_cb: None,
    internal_read_cb: None,
    internal_reading: false,
    internal_connect: None,
    internal_write_queue: VecDeque::new(),
    internal_connection_cb: None,
    internal_backlog: VecDeque::new(),
    internal_shutdown: None,
  }
}

/// Poll a single TCP handle for I/O readiness and fire callbacks.
/// Returns `true` if any work was completed.
///
/// # Safety
/// `tcp_ptr` must be a valid pointer to an initialized `uv_tcp_t`.
///
/// This function carefully avoids holding `&mut uv_tcp_t` references across
/// callback invocations. Callbacks may re-enter uv_* functions that access
/// the same handle through raw pointers, so any live `&mut` would violate
/// Rust's aliasing rules.
pub(crate) unsafe fn poll_tcp_handle(
  tcp_ptr: *mut uv_tcp_t,
  cx: &mut Context<'_>,
) -> bool {
  let mut any_work = false;

  // 1. Poll pending connect.
  //    Extract result with a short-lived borrow, then call callback with
  //    no outstanding references.
  let connect_result = unsafe {
    if let Some(ref mut pending) = (*tcp_ptr).internal_connect {
      match pending.future.as_mut().poll(cx) {
        Poll::Ready(result) => Some((pending.req, pending.cb, result)),
        Poll::Pending => None,
      }
    } else {
      None
    }
  };
  if let Some((req, cb, result)) = connect_result {
    // SAFETY: tcp_ptr is valid, no outstanding borrows.
    unsafe {
      let status = match result {
        Ok(stream) => {
          if (*tcp_ptr).internal_nodelay {
            stream.set_nodelay(true).ok();
          }
          (*tcp_ptr).internal_stream = Some(stream);
          0
        }
        Err(ref e) => io_error_to_uv(e),
      };
      (*tcp_ptr).internal_connect = None;
      (*req).handle = tcp_ptr as *mut uv_stream_t;
      if let Some(cb) = cb {
        cb(req, status);
      }
      // Match libuv's uv__stream_connect: on connect failure, flush the
      // write queue with UV_ECANCELED so writers aren't left hanging.
      if status < 0 {
        while let Some(pw) = (*tcp_ptr).internal_write_queue.pop_front() {
          if let Some(wcb) = pw.cb {
            wcb(pw.req, UV_ECANCELED);
          }
        }
      }
    }
  }

  // 2. Poll listener for new connections.
  //    Drain all ready connections from poll_accept before firing callbacks.
  //    This prevents starvation under high concurrency: tokio's reactor
  //    only re-polls epoll when poll_event_loop_inner returns Pending, so
  //    connections that arrived during the same epoll batch must be accepted
  //    now or they'll be invisible until the next reactor turn.
  //    After draining, fire the connection callback once per accepted stream
  //    (libuv fires the callback once per connection). If the user doesn't
  //    call uv_accept in the callback, stop firing to avoid spinning.
  unsafe {
    if (*tcp_ptr).internal_listener.is_some()
      && (*tcp_ptr).internal_connection_cb.is_some()
      && (*tcp_ptr).internal_backlog.is_empty()
    {
      let listener = (*tcp_ptr).internal_listener.as_ref().unwrap();
      while let Poll::Ready(Ok((stream, _))) = listener.poll_accept(cx) {
        (*tcp_ptr).internal_backlog.push_back(stream);
        any_work = true;
      }
    }
    while !(*tcp_ptr).internal_backlog.is_empty() {
      let backlog_len = (*tcp_ptr).internal_backlog.len();
      if let Some(cb) = (*tcp_ptr).internal_connection_cb {
        cb(tcp_ptr as *mut uv_stream_t, 0);
      }
      // If the callback did not call uv_accept (backlog didn't shrink),
      // stop firing to avoid an infinite loop.
      if (*tcp_ptr).internal_backlog.len() >= backlog_len {
        break;
      }
    }
  }

  // 3. Poll readable stream.
  //    Copy the callback pointers out first (they're Copy). Then in the
  //    loop, each try_read creates a short-lived borrow that is dropped
  //    before the read_cb fires.
  //
  //    Match libuv: limit to 32 reads per poll to prevent starvation.
  //    On EAGAIN/WouldBlock, call read_cb(nread=0) so the user can free
  //    the buffer. On null alloc, call read_cb(UV_ENOBUFS).
  unsafe {
    if (*tcp_ptr).internal_reading && (*tcp_ptr).internal_stream.is_some() {
      let alloc_cb = (*tcp_ptr).internal_alloc_cb;
      let read_cb = (*tcp_ptr).internal_read_cb;
      if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
        // Register interest so tokio's reactor wakes us.
        if let Some(ref stream) = (*tcp_ptr).internal_stream {
          let _ = stream.poll_read_ready(cx);
        }

        // Prevent loop starvation (matches libuv's count=32 in uv__read).
        let mut count = 32;
        loop {
          // Re-check after each callback: the callback may have
          // called uv_close or uv_read_stop.
          if !(*tcp_ptr).internal_reading
            || (*tcp_ptr).internal_stream.is_none()
          {
            break;
          }
          let mut buf = uv_buf_t {
            base: std::ptr::null_mut(),
            len: 0,
          };
          alloc_cb(tcp_ptr as *mut uv_handle_t, 65536, &mut buf);
          if buf.base.is_null() || buf.len == 0 {
            // Match libuv: report UV_ENOBUFS so the user knows alloc
            // failed, rather than silently dropping.
            read_cb(tcp_ptr as *mut uv_stream_t, UV_ENOBUFS as isize, &buf);
            break;
          }
          let slice =
            std::slice::from_raw_parts_mut(buf.base.cast::<u8>(), buf.len);
          // Short-lived borrow for try_read; dropped before callback.
          let read_result =
            (*tcp_ptr).internal_stream.as_ref().unwrap().try_read(slice);
          match read_result {
            Ok(0) => {
              read_cb(tcp_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
              (*tcp_ptr).internal_reading = false;
              crate::uv_compat::stream::maybe_clear_tcp_active(tcp_ptr);
              break;
            }
            Ok(n) => {
              any_work = true;
              let buflen = buf.len;
              read_cb(tcp_ptr as *mut uv_stream_t, n as isize, &buf);
              count -= 1;
              if count == 0 {
                break;
              }
              // Match libuv: if we didn't fill the buffer, the next
              // read would likely return EAGAIN. Exit early to save
              // a syscall (libuv's uv__read does the same).
              if n < buflen {
                break;
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
              // Match libuv: call read_cb with nread=0 so the user
              // can free the buffer allocated by alloc_cb.
              read_cb(tcp_ptr as *mut uv_stream_t, 0, &buf);
              break;
            }
            Err(ref e) => {
              // Match libuv: report real error codes, not UV_EOF.
              let status = io_error_to_uv(e);
              read_cb(tcp_ptr as *mut uv_stream_t, status as isize, &buf);
              (*tcp_ptr).internal_reading = false;
              crate::uv_compat::stream::maybe_clear_tcp_active(tcp_ptr);
              break;
            }
          }
        }
      }
    }
  }

  // 4. Drain write queue in order.
  //    Match libuv's two-phase approach: uv__write() processes writes and
  //    collects completions, then uv__write_callbacks() fires all callbacks
  //    after the write loop. This ensures multiple writes can complete in
  //    one tick and callbacks see a consistent state.
  //
  //    Completed writes are collected into a local vec, then callbacks are
  //    fired after the write loop finishes (no outstanding borrows).
  let mut completed_writes: Vec<(*mut uv_write_t, Option<uv_write_cb>, c_int)> =
    Vec::new();
  unsafe {
    if !(*tcp_ptr).internal_write_queue.is_empty()
      && (*tcp_ptr).internal_stream.is_some()
    {
      if let Some(ref stream) = (*tcp_ptr).internal_stream {
        let _ = stream.poll_write_ready(cx);

        // Also poll read readiness when writes are pending. This ensures
        // we detect a broken connection (peer close / RST) promptly via
        // the readable side, rather than waiting for a TCP retransmit
        // timeout on the write side.
        if !(*tcp_ptr).internal_reading
          && let Poll::Ready(Ok(())) = stream.poll_read_ready(cx)
        {
          // Read side is ready -- peek (without consuming) to check
          // for EOF or errors. Using MSG_PEEK avoids consuming data
          // that might belong to a higher-level protocol (e.g. TLS).
          #[cfg(unix)]
          let n = {
            use std::os::unix::io::AsRawFd;
            let fd = stream.as_raw_fd();
            let mut probe = [0u8; 1];
            libc::recv(fd, probe.as_mut_ptr() as *mut c_void, 1, libc::MSG_PEEK)
              as i32
          };
          #[cfg(windows)]
          let n = {
            use std::os::windows::io::AsRawSocket;
            unsafe extern "system" {
              fn recv(
                s: usize,
                buf: *mut c_void,
                len: c_int,
                flags: c_int,
              ) -> c_int;
            }
            const MSG_PEEK: c_int = 0x2;
            let socket = stream.as_raw_socket() as usize;
            let mut probe = [0u8; 1];
            recv(socket, probe.as_mut_ptr() as *mut c_void, 1, MSG_PEEK)
          };
          if n == 0 {
            // EOF — the connection is broken.
            // Drain the entire write queue with EPIPE.
            while let Some(pw) = (*tcp_ptr).internal_write_queue.pop_front() {
              completed_writes.push((pw.req, pw.cb, UV_EPIPE));
            }
          } else if n < 0 {
            let err =
              std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            #[cfg(unix)]
            let would_block = err == libc::EAGAIN || err == libc::EWOULDBLOCK;
            #[cfg(windows)]
            let would_block = err == 10035; // WSAEWOULDBLOCK
            if !would_block {
              // Real error — connection is broken.
              while let Some(pw) = (*tcp_ptr).internal_write_queue.pop_front() {
                completed_writes.push((pw.req, pw.cb, UV_EPIPE));
              }
            }
            // EAGAIN/EWOULDBLOCK means no data yet, connection alive
          }
          // n > 0 means data available, connection alive (data not consumed)
        }
      }

      // Match libuv's count=32 limit to prevent starvation.
      let mut count = 32;
      loop {
        if (*tcp_ptr).internal_write_queue.is_empty()
          || (*tcp_ptr).internal_stream.is_none()
        {
          break;
        }

        // Try writing in a limited scope.
        let (done, error) = {
          let stream = (*tcp_ptr).internal_stream.as_ref().unwrap();
          let pw = (*tcp_ptr).internal_write_queue.front_mut().unwrap();
          let mut done = false;
          let mut error = false;
          loop {
            if pw.offset >= pw.data.len() {
              done = true;
              break;
            }
            match stream.try_write(&pw.data[pw.offset..]) {
              Ok(n) => pw.offset += n,
              Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                break;
              }
              Err(_) => {
                error = true;
                break;
              }
            }
          }
          (done, error)
        };
        // Borrows on stream and pw dropped here.

        if done {
          let pw = (*tcp_ptr).internal_write_queue.pop_front().unwrap();
          any_work = true;
          completed_writes.push((pw.req, pw.cb, 0));
          count -= 1;
          if count > 0 {
            continue; // Try the next write in the queue.
          }
          break;
        } else if error {
          let pw = (*tcp_ptr).internal_write_queue.pop_front().unwrap();
          any_work = true;
          completed_writes.push((pw.req, pw.cb, UV_EPIPE));
          // Match libuv: stop writing after an error.
          break;
        } else {
          break; // WouldBlock -- retry next tick
        }
      }
    }
  }

  // Fire write callbacks after the write loop, matching libuv's
  // uv__write_callbacks(). No outstanding borrows on the TCP handle.
  unsafe {
    for (req, cb, status) in completed_writes {
      if let Some(cb) = cb {
        cb(req, status);
      }
    }
  }

  // 5. Complete deferred shutdown once write queue is drained.
  unsafe {
    if (*tcp_ptr).internal_write_queue.is_empty()
      && (*tcp_ptr).internal_shutdown.is_some()
      && (*tcp_ptr).internal_stream.is_some()
    {
      crate::uv_compat::stream::complete_shutdown(tcp_ptr, cx);
      any_work = true;
    }
  }

  any_work
}
