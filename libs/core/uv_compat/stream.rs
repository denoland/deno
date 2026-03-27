// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_void;

use super::UV_EAGAIN;
use super::UV_EALREADY;
use super::UV_EBADF;
use super::UV_EINVAL;
use super::UV_ENOTCONN;
use super::UV_EPIPE;
use super::UV_HANDLE_ACTIVE;
use super::UV_HANDLE_CLOSING;
use super::get_inner;
use super::tcp::WritePending;
use super::tcp::uv_tcp_t;
use super::tty::uv_tty_t;
use super::uv_handle_t;
use super::uv_handle_type;
use super::uv_loop_t;

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

/// Clear `UV_HANDLE_ACTIVE` when a TCP handle no longer has any pending
/// read/write/connect/listen/shutdown work keeping it alive.
///
/// # Safety
/// `tcp` must be a valid pointer to an initialized `uv_tcp_t`.
pub(crate) unsafe fn maybe_clear_tcp_active(tcp: *mut uv_tcp_t) {
  unsafe {
    if !(*tcp).internal_reading
      && (*tcp).internal_connection_cb.is_none()
      && (*tcp).internal_connect.is_none()
      && (*tcp).internal_write_queue.is_empty()
      && (*tcp).internal_shutdown.is_none()
    {
      (*tcp).flags &= !UV_HANDLE_ACTIVE;
    }
  }
}

/// ### Safety
/// `stream` must be a valid pointer to an initialized stream handle
/// (`uv_tcp_t` or `uv_tty_t`, cast as `uv_stream_t`).
pub unsafe fn uv_read_start(
  stream: *mut uv_stream_t,
  alloc_cb: Option<uv_alloc_cb>,
  read_cb: Option<uv_read_cb>,
) -> c_int {
  unsafe {
    if (*stream).r#type == uv_handle_type::UV_TTY {
      return super::tty::read_start_tty(
        stream as *mut uv_tty_t,
        alloc_cb,
        read_cb,
      );
    }
    // Match libuv: reject null callbacks.
    if alloc_cb.is_none() || read_cb.is_none() {
      return UV_EINVAL;
    }
    // Match libuv: reject closing handles.
    if (*stream).flags & UV_HANDLE_CLOSING != 0 {
      return UV_EINVAL;
    }
    // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;
    // Match libuv: always update callbacks even if already reading.
    // libuv does NOT return EALREADY here — it just overwrites the
    // callbacks.  TLSWrap relies on this to replace the plain read
    // callback with its own TLS-aware one.
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
/// `stream` must be a valid pointer to an initialized stream handle
/// (`uv_tcp_t` or `uv_tty_t`, cast as `uv_stream_t`).
pub unsafe fn uv_read_stop(stream: *mut uv_stream_t) -> c_int {
  unsafe {
    if (*stream).r#type == uv_handle_type::UV_TTY {
      return super::tty::read_stop_tty(stream as *mut uv_tty_t);
    }
    // SAFETY: Caller guarantees stream is a valid, initialized uv_tcp_t.
    let tcp = stream as *mut uv_tcp_t;
    let tcp_ref = &mut *tcp;
    tcp_ref.internal_reading = false;
    tcp_ref.internal_alloc_cb = None;
    tcp_ref.internal_read_cb = None;
    maybe_clear_tcp_active(tcp);
  }
  0
}

/// Mirrors libuv's `uv_stream_set_blocking`: toggles `O_NONBLOCK` on the
/// stream's underlying file descriptor.
///
/// **Note:** This only flips `O_NONBLOCK`; it does not implement libuv's
/// stronger "blocking writes complete synchronously" semantics. Writes
/// still queue through the poll loop. See libuv docs/src/stream.rst:229.
///
/// ### Safety
/// `stream` must be a valid pointer to an initialized stream handle.
#[cfg(unix)]
pub unsafe fn uv_stream_set_blocking(
  stream: *mut uv_stream_t,
  blocking: c_int,
) -> c_int {
  unsafe {
    let fd = if (*stream).r#type == uv_handle_type::UV_TTY {
      (*(stream as *mut uv_tty_t)).internal_fd
    } else {
      // TCP and other stream types
      match (*(stream as *mut uv_tcp_t)).internal_fd {
        Some(fd) => fd,
        None => return super::UV_EBADF,
      }
    };

    let flags = libc::fcntl(fd, libc::F_GETFL);
    if flags == -1 {
      return -(std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EINVAL));
    }
    let new_flags = if blocking != 0 {
      flags & !libc::O_NONBLOCK
    } else {
      flags | libc::O_NONBLOCK
    };
    if new_flags != flags && libc::fcntl(fd, libc::F_SETFL, new_flags) == -1 {
      return -(std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EINVAL));
    }
    0
  }
}

/// Mirrors libuv's `uv_stream_set_blocking`.
///
/// ### Safety
/// `stream` must be a valid pointer to an initialized stream handle.
#[cfg(windows)]
pub unsafe fn uv_stream_set_blocking(
  _stream: *mut uv_stream_t,
  _blocking: c_int,
) -> c_int {
  // On Windows, libuv handles blocking mode differently (via
  // SetNamedPipeHandleState / ioctlsocket). For now this is a no-op
  // stub — TTY and TCP streams on Windows use their own blocking
  // mechanisms.
  0
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized stream handle
/// (`uv_tcp_t` or `uv_tty_t`, cast as `uv_stream_t`).
pub unsafe fn uv_try_write(handle: *mut uv_stream_t, data: &[u8]) -> i32 {
  unsafe {
    if (*handle).r#type == uv_handle_type::UV_TTY {
      return super::tty::try_write_tty(handle, data);
    }
  }
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
/// `req` must be valid and remain so until the write callback fires. `handle`
/// must be an initialized stream handle (`uv_tcp_t` or `uv_tty_t`). `bufs`
/// must point to `nbufs` valid `uv_buf_t` entries.
///
/// This function avoids holding `&mut uv_tcp_t` across callback invocations
/// to prevent aliasing violations (the callback may access the handle via
/// `req.handle`).
pub unsafe fn uv_write(
  req: *mut uv_write_t,
  handle: *mut uv_stream_t,
  bufs: *const uv_buf_t,
  nbufs: u32,
  cb: Option<uv_write_cb>,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    if (*handle).r#type == uv_handle_type::UV_TTY {
      return super::tty::write_tty(req, handle, bufs, nbufs, cb);
    }
    let tcp = handle as *mut uv_tcp_t;
    (*req).handle = handle;

    if (*tcp).internal_stream.is_none() {
      // Match libuv: return error code from uv_write, don't invoke callback.
      return UV_EBADF;
    }

    let write_data = collect_bufs(bufs, nbufs);

    // Try to write synchronously when the queue is empty, matching libuv's
    // uv_write2() → uv__write() → uv__try_write() path.  This pushes data
    // into the kernel buffer immediately.  The callback is NOT fired here;
    // it is deferred to the poll loop (the entry is queued with the
    // already-written offset so the poll loop sees it as complete and fires
    // the callback then).  Deferring the callback is important because
    // callers like TLSWrap's enc_out() set re-entrancy guards (in_dowrite)
    // that would suppress the completion notification if it fired
    // synchronously.
    let mut offset = 0;
    if (*tcp).internal_write_queue.is_empty()
      && let Some(ref stream) = (*tcp).internal_stream
    {
      while offset < write_data.len() {
        match stream.try_write(&write_data[offset..]) {
          Ok(n) => offset += n,
          Err(_) => break,
        }
      }
    }

    (*tcp).internal_write_queue.push_back(WritePending {
      req,
      data: write_data,
      offset,
      cb,
      status: None,
    });
    0
  }
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
/// `req` must be a valid, writable pointer. `stream` must be an initialized
/// stream handle (`uv_tcp_t` or `uv_tty_t`). `req` must remain valid until
/// the shutdown callback fires.
pub unsafe fn uv_shutdown(
  req: *mut uv_shutdown_t,
  stream: *mut uv_stream_t,
  cb: Option<uv_shutdown_cb>,
) -> c_int {
  // SAFETY: Caller guarantees all pointers are valid.
  unsafe {
    if (*stream).r#type == uv_handle_type::UV_TTY {
      return super::tty::shutdown_tty(req, stream, cb);
    }
    // Match libuv: reject shutdown on closing streams.
    if (*stream).flags & UV_HANDLE_CLOSING != 0 {
      return UV_ENOTCONN;
    }
    let tcp = stream as *mut uv_tcp_t;
    (*req).handle = stream;

    if (*tcp).internal_stream.is_none() {
      return UV_ENOTCONN;
    }

    // Match libuv: reject if already shutting down.
    if (*tcp).internal_shutdown.is_some() {
      return UV_EALREADY;
    }

    // Defer the actual shutdown(2) until the write queue drains,
    // matching libuv's behavior where shutdown is processed in uv__drain.
    (*tcp).internal_shutdown = Some(super::tcp::ShutdownPending { req, cb });

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
pub(crate) unsafe fn complete_shutdown(
  tcp: *mut uv_tcp_t,
  cx: &mut std::task::Context<'_>,
) {
  use std::pin::Pin;

  use tokio::io::AsyncWrite;

  // SAFETY: Caller guarantees tcp is valid.
  let pending = unsafe { (*tcp).internal_shutdown.take() };
  let Some(pending) = pending else { return };

  let status =
    if let Some(ref mut stream) = unsafe { &mut *tcp }.internal_stream {
      match Pin::new(stream).poll_shutdown(cx) {
        std::task::Poll::Ready(Ok(())) => 0,
        std::task::Poll::Ready(Err(_)) => UV_ENOTCONN,
        std::task::Poll::Pending => {
          // Not ready yet — put it back and retry next tick.
          unsafe { (*tcp).internal_shutdown = Some(pending) };
          return;
        }
      }
    } else {
      UV_ENOTCONN
    };

  if let Some(cb) = pending.cb {
    // SAFETY: req and cb set by C caller via uv_shutdown.
    unsafe { cb(pending.req, status) };
  }

  // `uv_shutdown()` marks the handle active while the deferred shutdown is
  // in flight. Once it completes, drop the active bit if nothing else keeps
  // the stream alive.
  unsafe {
    maybe_clear_tcp_active(tcp);
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
