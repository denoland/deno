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
use libc::AF_INET;
#[cfg(unix)]
use libc::AF_INET6;
#[cfg(unix)]
use libc::sockaddr_in;
#[cfg(unix)]
use libc::sockaddr_in6;
#[cfg(unix)]
type sa_family_t = libc::sa_family_t;
#[cfg(windows)]
use win_sock::AF_INET;
#[cfg(windows)]
use win_sock::AF_INET6;
#[cfg(windows)]
use win_sock::sockaddr_in;
#[cfg(windows)]
use win_sock::sockaddr_in6;

use crate::uv_compat::UV_EADDRINUSE;
use crate::uv_compat::UV_EAGAIN;
use crate::uv_compat::UV_ECONNREFUSED;
use crate::uv_compat::UV_EINVAL;
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
use crate::uv_compat::uv_stream_t;
use crate::uv_compat::uv_shutdown_cb;
use crate::uv_compat::uv_shutdown_t;
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
  pub const SD_SEND: i32 = 1;
  unsafe extern "system" {
    pub fn shutdown(socket: usize, how: i32) -> i32;
  }
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

/// Map a `std::io::Error` to the closest libuv error code.
fn io_error_to_uv(err: &std::io::Error) -> c_int {
  use std::io::ErrorKind;
  match err.kind() {
    ErrorKind::AddrInUse => UV_EADDRINUSE,
    ErrorKind::AddrNotAvailable => UV_EINVAL,
    ErrorKind::ConnectionRefused => UV_ECONNREFUSED,
    ErrorKind::NotConnected => UV_ENOTCONN,
    ErrorKind::BrokenPipe => UV_EPIPE,
    ErrorKind::InvalidInput => UV_EINVAL,
    ErrorKind::WouldBlock => UV_EAGAIN,
    _ => {
      // On Unix, try to use the raw OS error for a more accurate mapping.
      #[cfg(unix)]
      if let Some(code) = err.raw_os_error() {
        return -code;
      }
      UV_EINVAL
    }
  }
}

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
  match sock_addr {
    Some(sa) => {
      // SAFETY: Caller guarantees tcp is valid and initialized.
      unsafe { (*tcp).internal_bind_addr = Some(sa) };
      0
    }
    None => UV_EINVAL,
  }
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
    (*tcp).flags |= UV_HANDLE_ACTIVE;
    let mut handles = inner.tcp_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
      handles.push(tcp);
    }

    (*tcp).internal_connect = Some(ConnectPending {
      future: Box::pin(tokio::net::TcpStream::connect(sock_addr)),
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
    if let Some(ref stream) = (*tcp).internal_stream
      && stream.set_nodelay(enabled).is_err()
    {
      return UV_EINVAL;
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
  _backlog: c_int,
  cb: Option<uv_connection_cb>,
) -> c_int {
  // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
  unsafe {
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;

    let bind_addr = tcp_ref
      .internal_bind_addr
      .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

    let std_listener = match std::net::TcpListener::bind(bind_addr) {
      Ok(l) => l,
      Err(ref e) => return io_error_to_uv(e),
    };
    std_listener.set_nonblocking(true).ok();
    let listener_addr = std_listener.local_addr().ok();
    let tokio_listener = match tokio::net::TcpListener::from_std(std_listener) {
      Ok(l) => l,
      Err(_) => return UV_EINVAL,
    };

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
pub(crate) unsafe fn poll_tcp_handle(
  tcp_ptr: *mut uv_tcp_t,
  cx: &mut Context<'_>,
) -> bool {
  let mut any_work = false;
  // SAFETY: Caller guarantees tcp_ptr is valid.
  let tcp = unsafe { &mut *tcp_ptr };

  // 1. Poll pending connect
  if let Some(ref mut pending) = tcp.internal_connect
    && let Poll::Ready(result) = pending.future.as_mut().poll(cx)
  {
    let req = pending.req;
    let cb = pending.cb;
    let status = match result {
      Ok(stream) => {
        if tcp.internal_nodelay {
          stream.set_nodelay(true).ok();
        }
        tcp.internal_stream = Some(stream);
        0
      }
      Err(ref e) => io_error_to_uv(e),
    };
    tcp.internal_connect = None;
    // SAFETY: req pointer was provided by the C caller and remains valid until callback.
    unsafe {
      (*req).handle = tcp_ptr as *mut uv_stream_t;
    }
    if let Some(cb) = cb {
      // SAFETY: Callback and req pointer validated above; set by C caller via uv_tcp_connect.
      unsafe { cb(req, status) };
    }
  }

  // 2. Poll listener for new connections.
  // Match libuv: accept one connection at a time. Only poll for a new
  // connection when the backlog is empty (i.e., the previous connection
  // was consumed via uv_accept). If the user doesn't call uv_accept in
  // the callback, we stop polling to avoid spinning.
  if let Some(ref listener) = tcp.internal_listener
    && tcp.internal_connection_cb.is_some()
  {
    if tcp.internal_backlog.is_empty() {
      if let Poll::Ready(Ok((stream, _))) = listener.poll_accept(cx) {
        tcp.internal_backlog.push_back(stream);
        any_work = true;
      }
    }
    if !tcp.internal_backlog.is_empty() {
      if let Some(cb) = tcp.internal_connection_cb {
        // SAFETY: tcp_ptr is valid; cb set by C caller via uv_listen.
        unsafe { cb(tcp_ptr as *mut uv_stream_t, 0) };
      }
      // If uv_accept wasn't called in the callback (backlog still
      // non-empty), don't poll again until it is consumed.
    }
  }

  // 3. Poll readable stream
  if tcp.internal_reading && tcp.internal_stream.is_some() {
    let alloc_cb = tcp.internal_alloc_cb;
    let read_cb = tcp.internal_read_cb;
    if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
      // Register interest so tokio's reactor wakes us.
      let _ = tcp
        .internal_stream
        .as_ref()
        .unwrap()
        .poll_read_ready(cx);

      loop {
        // Re-check after each callback: the callback may have
        // called uv_close or uv_read_stop.
        if !tcp.internal_reading || tcp.internal_stream.is_none() {
          break;
        }
        let mut buf = uv_buf_t {
          base: std::ptr::null_mut(),
          len: 0,
        };
        // SAFETY: alloc_cb set by C caller via uv_read_start; tcp_ptr is valid.
        unsafe {
          alloc_cb(tcp_ptr as *mut uv_handle_t, 65536, &mut buf);
        }
        if buf.base.is_null() || buf.len == 0 {
          break;
        }
        // SAFETY: alloc_cb guarantees buf.base is valid for buf.len bytes.
        let slice = unsafe {
          std::slice::from_raw_parts_mut(buf.base.cast::<u8>(), buf.len)
        };
        match tcp.internal_stream.as_ref().unwrap().try_read(slice) {
          Ok(0) => {
            // SAFETY: read_cb set by C caller via uv_read_start; tcp_ptr and buf are valid.
            unsafe {
              read_cb(tcp_ptr as *mut uv_stream_t, UV_EOF as isize, &buf)
            };
            tcp.internal_reading = false;
            break;
          }
          Ok(n) => {
            any_work = true;
            // SAFETY: read_cb set by C caller via uv_read_start; tcp_ptr and buf are valid.
            unsafe {
              read_cb(tcp_ptr as *mut uv_stream_t, n as isize, &buf)
            };
          }
          Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            break;
          }
          Err(_) => {
            // SAFETY: read_cb set by C caller via uv_read_start; tcp_ptr and buf are valid.
            unsafe {
              read_cb(tcp_ptr as *mut uv_stream_t, UV_EOF as isize, &buf)
            };
            tcp.internal_reading = false;
            break;
          }
        }
      }
    }
  }

  // 4. Drain write queue in order
  if !tcp.internal_write_queue.is_empty() && tcp.internal_stream.is_some() {
    let stream = tcp.internal_stream.as_ref().unwrap();
    let _ = stream.poll_write_ready(cx);

    while let Some(pw) = tcp.internal_write_queue.front_mut() {
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
      if done {
        let pw = tcp.internal_write_queue.pop_front().unwrap();
        if let Some(cb) = pw.cb {
          // SAFETY: Write cb and req set by C caller via uv_write; req is valid until callback.
          unsafe { cb(pw.req, 0) };
        }
      } else if error {
        let pw = tcp.internal_write_queue.pop_front().unwrap();
        if let Some(cb) = pw.cb {
          // SAFETY: Write cb and req set by C caller via uv_write; req is valid until callback.
          unsafe { cb(pw.req, UV_EPIPE) };
        }
      } else {
        break; // WouldBlock -- retry next tick
      }
    }
  }

  // 5. Complete deferred shutdown once write queue is drained
  if tcp.internal_write_queue.is_empty()
    && tcp.internal_shutdown.is_some()
    && tcp.internal_stream.is_some()
  {
    // SAFETY: tcp_ptr is valid; complete_shutdown is safe when stream and shutdown are set.
    unsafe { crate::uv_compat::stream::complete_shutdown(tcp_ptr) };
    any_work = true;
  }

  any_work
}
