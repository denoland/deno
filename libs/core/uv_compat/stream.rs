// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;

use super::get_inner;
use super::tcp::uv_tcp_t;
use super::tcp::WritePending;
use super::uv_handle_t;
use super::uv_handle_type;
use super::uv_loop_t;
use super::UV_EBADF;
use super::UV_EAGAIN;
use super::UV_ENOTCONN;
use super::UV_EPIPE;
use super::UV_HANDLE_ACTIVE;

#[repr(C)]
pub struct uv_stream_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
}

#[repr(C)]
pub struct uv_write_t {
  pub r#type: i32, // UV_REQ_TYPE fields
  pub data: *mut c_void,
  pub handle: *mut uv_stream_t,
}

#[repr(C)]
pub struct uv_connect_t {
  pub r#type: i32,
  pub data: *mut c_void,
  pub handle: *mut uv_stream_t,
}

#[repr(C)]
pub struct uv_shutdown_t {
  pub r#type: i32,
  pub data: *mut c_void,
  pub handle: *mut uv_stream_t,
}

/// I/O buffer descriptor matching libuv's `uv_buf_t`.
///
/// Field order is `{base, len}` which matches the Unix layout (all platforms,
/// including Linux -- `struct iovec` is `{iov_base, iov_len}` everywhere).
/// On Windows, real libuv uses `{len, base}` (matching `WSABUF`) with `len`
/// as `ULONG` (32-bit). If this struct ever needs to cross an FFI boundary
/// on Windows, the field order and `len` type must be made platform-conditional.
#[repr(C)]
pub struct uv_buf_t {
  pub base: *mut c_char,
  pub len: usize,
}

pub type uv_write_cb = unsafe extern "C" fn(*mut uv_write_t, i32);
pub type uv_alloc_cb =
  unsafe extern "C" fn(*mut uv_handle_t, usize, *mut uv_buf_t);
pub type uv_read_cb =
  unsafe extern "C" fn(*mut uv_stream_t, isize, *const uv_buf_t);
pub type uv_connection_cb = unsafe extern "C" fn(*mut uv_stream_t, i32);
pub type uv_connect_cb = unsafe extern "C" fn(*mut uv_connect_t, i32);
pub type uv_shutdown_cb = unsafe extern "C" fn(*mut uv_shutdown_t, i32);

pub type UvStream = uv_stream_t;
pub type UvWrite = uv_write_t;
pub type UvBuf = uv_buf_t;
pub type UvConnect = uv_connect_t;
pub type UvShutdown = uv_shutdown_t;

/// ### Safety
/// `stream` must be a valid pointer to an initialized `uv_tcp_t` (cast as `uv_stream_t`).
pub unsafe fn uv_read_start(
  stream: *mut uv_stream_t,
  alloc_cb: Option<uv_alloc_cb>,
  read_cb: Option<uv_read_cb>,
) -> c_int {
  // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
  unsafe {
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;
    tcp_ref.internal_alloc_cb = alloc_cb;
    tcp_ref.internal_read_cb = read_cb;
    tcp_ref.internal_reading = true;
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
/// `stream` must be a valid pointer to an initialized `uv_tcp_t` (cast as `uv_stream_t`).
pub unsafe fn uv_read_stop(stream: *mut uv_stream_t) -> c_int {
  // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
  unsafe {
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;
    tcp_ref.internal_reading = false;
    tcp_ref.internal_alloc_cb = None;
    tcp_ref.internal_read_cb = None;
    if tcp_ref.internal_connection_cb.is_none()
      && tcp_ref.internal_connect.is_none()
      && tcp_ref.internal_write_queue.is_empty()
    {
      tcp_ref.flags &= !UV_HANDLE_ACTIVE;
    }
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized `uv_tcp_t` (cast as `uv_stream_t`).
pub unsafe fn uv_try_write(handle: *mut uv_stream_t, data: &[u8]) -> i32 {
  // SAFETY: Caller guarantees handle is a valid, initialized uv_tcp_t.
  let tcp_ref = unsafe { &mut *(handle as *mut uv_tcp_t) };

  // Match libuv: return UV_EAGAIN if a connect is in progress or writes are queued.
  if tcp_ref.internal_connect.is_some()
    || !tcp_ref.internal_write_queue.is_empty()
  {
    return UV_EAGAIN;
  }

  let stream = match tcp_ref.internal_stream.as_ref() {
    Some(s) => s,
    None => return UV_EBADF,
  };

  match stream.try_write(data) {
    Ok(n) => n as i32,
    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => UV_EAGAIN,
    Err(_) => UV_EPIPE,
  }
}

/// ### Safety
/// `req` must be valid and remain so until the write callback fires. `handle` must be an
/// initialized `uv_tcp_t`. `bufs` must point to `nbufs` valid `uv_buf_t` entries.
pub unsafe fn uv_write(
  req: *mut uv_write_t,
  handle: *mut uv_stream_t,
  bufs: *const uv_buf_t,
  nbufs: u32,
  cb: Option<uv_write_cb>,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    let tcp = handle as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;
    (*req).handle = handle;

    let stream = match tcp_ref.internal_stream.as_ref() {
      Some(s) => s,
      // Match libuv: return error code from uv_write, don't invoke callback.
      None => return UV_EBADF,
    };

    if !tcp_ref.internal_write_queue.is_empty() {
      let write_data = collect_bufs(bufs, nbufs);
      tcp_ref.internal_write_queue.push_back(WritePending {
        req,
        data: write_data,
        offset: 0,
        cb,
      });
      return 0;
    }

    if nbufs == 1 {
      let buf = &*bufs;
      if !buf.base.is_null() && buf.len > 0 {
        let data = std::slice::from_raw_parts(buf.base as *const u8, buf.len);
        let mut offset = 0;
        loop {
          match stream.try_write(&data[offset..]) {
            Ok(n) => {
              offset += n;
              if offset >= data.len() {
                if let Some(cb) = cb {
                  cb(req, 0);
                }
                return 0;
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
              tcp_ref.internal_write_queue.push_back(WritePending {
                req,
                data: data[offset..].to_vec(),
                offset: 0,
                cb,
              });
              return 0;
            }
            Err(_) => {
              if let Some(cb) = cb {
                cb(req, UV_EPIPE);
              }
              return 0;
            }
          }
        }
      }
      if let Some(cb) = cb {
        cb(req, 0);
      }
      return 0;
    }

    let iovecs: smallvec::SmallVec<[std::io::IoSlice<'_>; 8]> = (0..nbufs
      as usize)
      .filter_map(|i| {
        let buf = &*bufs.add(i);
        if buf.base.is_null() || buf.len == 0 {
          None
        } else {
          Some(std::io::IoSlice::new(std::slice::from_raw_parts(
            buf.base as *const u8,
            buf.len,
          )))
        }
      })
      .collect();

    let total_len: usize = iovecs.iter().map(|s| s.len()).sum();
    if total_len == 0 {
      if let Some(cb) = cb {
        cb(req, 0);
      }
      return 0;
    }

    match stream.try_write_vectored(&iovecs) {
      Ok(n) if n >= total_len => {
        if let Some(cb) = cb {
          cb(req, 0);
        }
        return 0;
      }
      Ok(n) => {
        let mut write_data = Vec::with_capacity(total_len - n);
        let mut skip = n;
        for iov in &iovecs {
          if skip >= iov.len() {
            skip -= iov.len();
          } else {
            write_data.extend_from_slice(&iov[skip..]);
            skip = 0;
          }
        }
        tcp_ref.internal_write_queue.push_back(WritePending {
          req,
          data: write_data,
          offset: 0,
          cb,
        });
      }
      Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        let write_data = collect_bufs(bufs, nbufs);
        tcp_ref.internal_write_queue.push_back(WritePending {
          req,
          data: write_data,
          offset: 0,
          cb,
        });
      }
      Err(_) => {
        if let Some(cb) = cb {
          cb(req, UV_EPIPE);
        }
      }
    }
  }
  0
}

/// ### Safety
/// `bufs` must point to `nbufs` valid `uv_buf_t` entries with valid `base` pointers.
unsafe fn collect_bufs(bufs: *const uv_buf_t, nbufs: u32) -> Vec<u8> {
  // SAFETY: Caller guarantees bufs points to nbufs valid entries.
  unsafe {
    let mut total = 0usize;
    for i in 0..nbufs as usize {
      let buf = &*bufs.add(i);
      if !buf.base.is_null() {
        total += buf.len;
      }
    }
    let mut data = Vec::with_capacity(total);
    for i in 0..nbufs as usize {
      let buf = &*bufs.add(i);
      if !buf.base.is_null() && buf.len > 0 {
        data.extend_from_slice(std::slice::from_raw_parts(
          buf.base as *const u8,
          buf.len,
        ));
      }
    }
    data
  }
}

/// ### Safety
/// `req` must be a valid, writable pointer. `stream` must be an initialized `uv_tcp_t`.
/// `req` must remain valid until the shutdown callback fires.
pub unsafe fn uv_shutdown(
  req: *mut uv_shutdown_t,
  stream: *mut uv_stream_t,
  cb: Option<uv_shutdown_cb>,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    let tcp = stream as *mut uv_tcp_t;
    (*req).handle = stream;

    if (*tcp).internal_stream.is_none() {
      return UV_ENOTCONN;
    }

    // Defer the actual shutdown(2) until the write queue drains,
    // matching libuv's behavior where shutdown is processed in uv__drain.
    (*tcp).internal_shutdown =
      Some(super::tcp::ShutdownPending { req, cb });

    let inner = get_inner((*tcp).loop_);
    let mut handles = inner.tcp_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
      handles.push(tcp);
    }
    (*tcp).flags |= UV_HANDLE_ACTIVE;
  }
  0
}

/// Perform the deferred shutdown(2) syscall and fire the callback.
///
/// # Safety
/// `tcp` must be a valid pointer to an initialized `uv_tcp_t` with
/// `internal_stream` set and `internal_shutdown` set.
pub(crate) unsafe fn complete_shutdown(tcp: *mut uv_tcp_t) {
  // SAFETY: Caller guarantees tcp is valid.
  let pending = unsafe { (*tcp).internal_shutdown.take() };
  let Some(pending) = pending else { return };

  let status = if let Some(ref stream) = unsafe { &*tcp }.internal_stream {
    #[cfg(unix)]
    {
      use std::os::unix::io::AsRawFd;
      let fd = stream.as_raw_fd();
      // SAFETY: fd is a valid file descriptor from the TcpStream.
      if unsafe { libc::shutdown(fd, libc::SHUT_WR) } == 0 {
        0
      } else {
        UV_ENOTCONN
      }
    }
    #[cfg(windows)]
    {
      use std::os::windows::io::AsRawSocket;
      let sock = stream.as_raw_socket();
      if win_sock::shutdown(sock as usize, win_sock::SD_SEND) == 0 {
        0
      } else {
        UV_ENOTCONN
      }
    }
  } else {
    UV_ENOTCONN
  };

  if let Some(cb) = pending.cb {
    // SAFETY: req and cb set by C caller via uv_shutdown.
    unsafe { cb(pending.req, status) };
  }
}

#[cfg(windows)]
mod win_sock {
  pub const SD_SEND: i32 = 1;
  unsafe extern "system" {
    pub fn shutdown(socket: usize, how: i32) -> i32;
  }
}

pub fn new_write() -> UvWrite {
  uv_write_t {
    r#type: 0,
    data: std::ptr::null_mut(),
    handle: std::ptr::null_mut(),
  }
}

pub fn new_connect() -> UvConnect {
  uv_connect_t {
    r#type: 0,
    data: std::ptr::null_mut(),
    handle: std::ptr::null_mut(),
  }
}

pub fn new_shutdown() -> UvShutdown {
  uv_shutdown_t {
    r#type: 0,
    data: std::ptr::null_mut(),
    handle: std::ptr::null_mut(),
  }
}
