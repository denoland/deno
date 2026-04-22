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
    if (*stream).r#type == uv_handle_type::UV_NAMED_PIPE {
      return super::pipe::read_start_pipe(
        stream as *mut super::pipe::uv_pipe_t,
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
    if let Some(w) = tcp_ref.internal_waker.as_ref() {
      w.mark_ready();
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
    if (*stream).r#type == uv_handle_type::UV_NAMED_PIPE {
      return super::pipe::read_stop_pipe(
        stream as *mut super::pipe::uv_pipe_t,
      );
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
/// (`uv_tcp_t`, `uv_tty_t`, or `uv_pipe_t`, cast as `uv_stream_t`).
/// Scatter-gather try_write. Mirrors libuv's
/// `uv_try_write(stream, bufs, nbufs)`: attempts a non-blocking vectored
/// write and returns the number of bytes written, `UV_EAGAIN` when the
/// socket would block, or a negative error code. Preferred over
/// `uv_try_write` for stream_wrap's `writev` op because iovecs can
/// point at V8 `ArrayBuffer` backing stores with no concat copy.
///
/// TCP uses tokio's `try_write_vectored` for true `writev(2)`; pipes
/// and TTYs fall back to iterating single-buf `try_write_{pipe,tty}`
/// since their internals already copy into owned queues.
///
/// ### Safety
/// `handle` must be a valid pointer to an initialized stream handle
/// (`uv_tcp_t`, `uv_tty_t`, or `uv_pipe_t`, cast as `uv_stream_t`).
/// Each `IoSlice` must reference memory valid for the duration of
/// this call.
pub unsafe fn uv_try_writev(
  handle: *mut uv_stream_t,
  bufs: &[std::io::IoSlice<'_>],
) -> i32 {
  // SAFETY: caller guarantees handle is valid.
  let handle_type = unsafe { (*handle).r#type };
  unsafe {
    if handle_type == uv_handle_type::UV_TTY {
      // TTY has no native writev path; iterate. Stop on partial to
      // match libuv's short-write semantics on writev.
      let mut total = 0i32;
      for buf in bufs {
        let rc = super::tty::try_write_tty(handle, buf);
        if rc < 0 {
          if total > 0 {
            return total;
          }
          return rc;
        }
        total = total.saturating_add(rc);
        if (rc as usize) != buf.len() {
          return total;
        }
      }
      return total;
    }
    if handle_type == uv_handle_type::UV_NAMED_PIPE {
      let mut total = 0i32;
      for buf in bufs {
        let rc = super::pipe::try_write_pipe(
          handle as *mut super::pipe::uv_pipe_t,
          buf,
        );
        if rc < 0 {
          if total > 0 {
            return total;
          }
          return rc;
        }
        total = total.saturating_add(rc);
        if (rc as usize) != buf.len() {
          return total;
        }
      }
      return total;
    }
  }
  // SAFETY: Caller guarantees handle is a valid, initialized uv_tcp_t.
  let tcp_ref = unsafe { &mut *(handle as *mut uv_tcp_t) };

  if tcp_ref.internal_connect.is_some()
    || !tcp_ref.internal_write_queue.is_empty()
  {
    return UV_EAGAIN;
  }

  let stream = match tcp_ref.internal_stream.as_ref() {
    Some(s) => s,
    None => return UV_EBADF,
  };

  match stream.try_write_vectored(bufs) {
    Ok(n) => n as i32,
    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => UV_EAGAIN,
    Err(_) => UV_EPIPE,
  }
}

pub unsafe fn uv_try_write(handle: *mut uv_stream_t, data: &[u8]) -> i32 {
  // Dispatch by handle type; `uv_tcp_t`, `uv_pipe_t`, and `uv_tty_t` have
  // different struct layouts, so the TCP path below must only be taken
  // for `UV_TCP` handles.
  // SAFETY: `handle` is a valid initialized uv stream per caller contract.
  let handle_type = unsafe { (*handle).r#type };
  unsafe {
    if handle_type == uv_handle_type::UV_TTY {
      return super::tty::try_write_tty(handle, data);
    }
    if handle_type == uv_handle_type::UV_NAMED_PIPE {
      return super::pipe::try_write_pipe(
        handle as *mut super::pipe::uv_pipe_t,
        data,
      );
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
    if (*handle).r#type == uv_handle_type::UV_NAMED_PIPE {
      return write_pipe(
        req,
        handle as *mut super::pipe::uv_pipe_t,
        bufs,
        nbufs,
        cb,
      );
    }
    let tcp = handle as *mut uv_tcp_t;
    (*req).handle = handle;

    if (*tcp).internal_stream.is_none() {
      // Match libuv: return error code from uv_write, don't invoke callback.
      return UV_EBADF;
    }

    let write_data = collect_bufs(bufs, nbufs);

    uv_write_owned_impl(req, tcp, write_data, cb)
  }
}

/// Take an already-owned `Vec<u8>` and queue it as a pending write on
/// the TCP handle. This avoids the extra allocation + memcpy that
/// `uv_write` does via `collect_bufs` when the caller has already
/// materialized the bytes into a single buffer (e.g. the stream_wrap
/// `writev` op concatenates JS chunks into one Vec before writing).
///
/// ### Safety
/// `req` must be valid until the write callback fires. `tcp` must be
/// initialized by `uv_tcp_init`.
pub unsafe fn uv_write_owned_tcp(
  req: *mut uv_write_t,
  tcp: *mut uv_tcp_t,
  data: Vec<u8>,
  cb: Option<uv_write_cb>,
) -> c_int {
  unsafe {
    (*req).handle = tcp as *mut uv_stream_t;
    if (*tcp).internal_stream.is_none() {
      return UV_EBADF;
    }
    uv_write_owned_impl(req, tcp, data, cb)
  }
}

/// Queue a scatter-gather async write with caller-owned retention.
/// The iovecs in `bufs` may point at memory not owned by uv_compat
/// (e.g. V8 `ArrayBuffer` backing stores retained on the JS side);
/// the caller must ensure that memory stays valid until `cb` fires.
///
/// Mirrors libuv's `uv_write(req, stream, bufs, nbufs, cb)` with the
/// key difference that our write queue preserves the iovec layout
/// across drain calls, allowing true scatter-gather writes on TCP
/// without ever concatenating into a single buffer.
///
/// ### Safety
/// `req` must be valid until the callback fires. `handle` must be an
/// initialized stream handle. Each iovec's `base` pointer must remain
/// valid and readable for its `len` until the callback fires.
pub unsafe fn uv_writev_owned(
  req: *mut uv_write_t,
  handle: *mut uv_stream_t,
  bufs: smallvec::SmallVec<[uv_buf_t; 4]>,
  cb: Option<uv_write_cb>,
) -> c_int {
  use super::tcp::IovecCursor;
  use super::tcp::WritePending;
  // SAFETY: caller contract.
  unsafe {
    (*req).handle = handle;
    match (*handle).r#type {
      uv_handle_type::UV_TCP => {
        let tcp = handle as *mut uv_tcp_t;
        if (*tcp).internal_stream.is_none() {
          return UV_EBADF;
        }
        // Try sync drain first to match uv__write's inline write-loop.
        // Callback still fires asynchronously (deferred) if everything
        // drains — we mark status=Some(0) so the poll loop fires it.
        let mut iov = IovecCursor::new(bufs);
        if (*tcp).internal_write_queue.is_empty()
          && let Some(ref stream) = (*tcp).internal_stream
        {
          loop {
            if iov.is_empty() {
              break;
            }
            let write_result = {
              let mut slices: smallvec::SmallVec<[std::io::IoSlice; 16]> =
                smallvec::SmallVec::new();
              iov.io_slices(&mut slices);
              if slices.is_empty() {
                break;
              }
              stream.try_write_vectored(&slices)
            };
            match write_result {
              Ok(0) => break,
              Ok(n) => iov.advance(n),
              Err(_) => break,
            }
          }
        }
        (*tcp).internal_write_queue.push_back(WritePending {
          req,
          data: Vec::new(),
          offset: 0,
          iovecs: Some(iov),
          cb,
          status: None,
        });
        (*tcp).flags |= UV_HANDLE_ACTIVE;
        let inner = get_inner((*tcp).loop_);
        let mut handles = inner.tcp_handles.borrow_mut();
        if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
          handles.push(tcp);
        }
        if let Some(w) = (*tcp).internal_waker.as_ref() {
          w.mark_ready();
        }
        0
      }
      _ => {
        // Pipes/TTYs internally copy; go through the regular
        // `uv_write` buf-vector dispatch.
        let raw_bufs: smallvec::SmallVec<[uv_buf_t; 4]> = bufs;
        let nbufs = raw_bufs.len() as u32;
        let rc = uv_write(req, handle, raw_bufs.as_ptr(), nbufs, cb);
        // Keep raw_bufs alive through the call; `uv_write` collects
        // into its own owned Vec, so the iovec array can drop now.
        drop(raw_bufs);
        rc
      }
    }
  }
}

/// Polymorphic counterpart to `uv_write_owned_tcp` that dispatches on
/// the runtime stream type. Callers (e.g. the stream_wrap `writev` op)
/// hold a `*mut uv_stream_t` that may back a TCP, pipe, or TTY handle;
/// blindly treating it as TCP corrupts the pipe's in-place VecDeque
/// and trips UB (the write queue is at a different offset in each
/// struct). For non-TCP types, fall back to the existing `uv_write`
/// buffer-vector path by materializing a one-entry `uv_buf_t` over
/// the owned data.
///
/// ### Safety
/// `req` must be valid until the write callback fires. `handle` must
/// be an initialized stream handle (TCP, pipe, or TTY).
pub unsafe fn uv_write_owned(
  req: *mut uv_write_t,
  handle: *mut uv_stream_t,
  data: Vec<u8>,
  cb: Option<uv_write_cb>,
) -> c_int {
  unsafe {
    match (*handle).r#type {
      uv_handle_type::UV_TCP => {
        uv_write_owned_tcp(req, handle as *mut uv_tcp_t, data, cb)
      }
      // Pipes/TTYs don't have an owned-Vec shortcut — build a single
      // uv_buf_t and go through the regular `uv_write` dispatch which
      // knows about the right per-type write queue layouts.
      _ => {
        let buf = uv_buf_t {
          base: data.as_ptr() as *mut c_char,
          len: data.len(),
        };
        // uv_write's pipe/tty paths internally copy the buffer into
        // their own queue, so releasing ownership of `data` after the
        // call is safe even though the bufs array lives on the stack.
        let rc = uv_write(req, handle, &buf, 1, cb);
        drop(data);
        rc
      }
    }
  }
}

/// Shared logic for queuing a pre-built Vec<u8> as a write.
///
/// ### Safety
/// `req` must be valid until the write callback fires. `tcp` must be
/// initialized and have `internal_stream` set.
unsafe fn uv_write_owned_impl(
  req: *mut uv_write_t,
  tcp: *mut uv_tcp_t,
  write_data: Vec<u8>,
  cb: Option<uv_write_cb>,
) -> c_int {
  unsafe {
    // Try sync write when the queue is empty, matching libuv's
    // uv_write2 → uv__write → uv__try_write path. Callback is
    // deferred to the poll loop to avoid re-entrancy with callers
    // like TLSWrap that set `in_dowrite` guards.
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
      iovecs: None,
      cb,
      status: None,
    });

    (*tcp).flags |= UV_HANDLE_ACTIVE;
    let inner = get_inner((*tcp).loop_);
    let mut handles = inner.tcp_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tcp)) {
      handles.push(tcp);
    }
    if let Some(w) = (*tcp).internal_waker.as_ref() {
      w.mark_ready();
    }
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
    (*req).handle = stream;

    if (*stream).r#type == uv_handle_type::UV_NAMED_PIPE {
      let pipe = stream as *mut super::pipe::uv_pipe_t;
      #[cfg(unix)]
      if (*pipe).internal_stream.is_none() && (*pipe).internal_fd.is_none() {
        return UV_ENOTCONN;
      }
      (*pipe).internal_shutdown = Some(super::tcp::ShutdownPending { req, cb });
      let inner = get_inner((*pipe).loop_);
      let mut handles = inner.pipe_handles.borrow_mut();
      if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
        handles.push(pipe);
      }
      (*pipe).flags |= UV_HANDLE_ACTIVE;
      drop(handles);
      if let Some(w) = (*pipe).internal_waker.as_ref() {
        w.mark_ready();
      }

      // Wake the event loop so run_io processes the deferred shutdown.
      if let Some(waker) = inner.waker.borrow().as_ref() {
        waker.wake_by_ref();
      }
      return 0;
    }

    let tcp = stream as *mut uv_tcp_t;

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
    drop(handles);
    if let Some(w) = (*tcp).internal_waker.as_ref() {
      w.mark_ready();
    }

    // Wake the event loop so run_io processes the deferred shutdown.
    // Without this, shutdowns scheduled from nextTick/microtask
    // callbacks (e.g. endWritableNT for allowHalfOpen=false sockets)
    // would stall because the Tokio reactor has no pending future to
    // wake it.
    if let Some(waker) = inner.waker.borrow().as_ref() {
      waker.wake_by_ref();
    }
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

/// Write to a pipe handle. Tries synchronous write first, queues remainder.
unsafe fn write_pipe(
  req: *mut uv_write_t,
  pipe: *mut super::pipe::uv_pipe_t,
  bufs: *const uv_buf_t,
  nbufs: u32,
  cb: Option<uv_write_cb>,
) -> c_int {
  use super::tcp::WritePending;
  unsafe {
    (*req).handle = pipe as *mut uv_stream_t;

    let write_data = collect_bufs(bufs, nbufs);

    // Try synchronous write when queue is empty.
    let mut offset = 0;
    #[cfg(unix)]
    if (*pipe).internal_write_queue.is_empty() {
      // Use try_write on UnixStream if available, fall back to libc::write.
      if let Some(ref stream) = (*pipe).internal_stream {
        while offset < write_data.len() {
          match stream.try_write(&write_data[offset..]) {
            Ok(n) => {
              offset += n;
            }
            Err(ref _e) => {
              break;
            }
          }
        }
      } else if let Some(fd) = (*pipe).internal_fd {
        while offset < write_data.len() {
          let n = libc::write(
            fd,
            write_data[offset..].as_ptr() as *const std::ffi::c_void,
            write_data.len() - offset,
          );
          if n >= 0 {
            offset += n as usize;
          } else {
            break;
          }
        }
      }
    }
    #[cfg(windows)]
    if (*pipe).internal_write_queue.is_empty()
      && let Some(handle) = (*pipe).internal_handle
    {
      use std::io::Write;
      use std::os::windows::io::FromRawHandle;
      let mut file = std::fs::File::from_raw_handle(handle);
      while offset < write_data.len() {
        match file.write(&write_data[offset..]) {
          Ok(n) => offset += n,
          Err(_) => break,
        }
      }
      let _ = std::os::windows::io::IntoRawHandle::into_raw_handle(file);
    }

    let status = if offset >= write_data.len() {
      Some(0) // fully written
    } else {
      None // needs async completion
    };

    (*pipe).internal_write_queue.push_back(WritePending {
      req,
      data: write_data,
      offset,
      iovecs: None,
      cb,
      status,
    });

    // Ensure AsyncFd exists for write readiness tracking. This is
    // normally created eagerly in uv_pipe_open, but serves as a
    // safety net for pipes that skipped that path.
    #[cfg(unix)]
    if (*pipe).internal_async_fd.is_none()
      && (*pipe).internal_stream.is_none()
      && (*pipe).internal_connect.is_none()
      && (*pipe).internal_fd.is_some()
    {
      let fd = (*pipe).internal_fd.unwrap();
      if let Ok(afd) =
        tokio::io::unix::AsyncFd::new(super::pipe::RawFdWrapper(fd))
      {
        (*pipe).internal_async_fd = Some(afd);
      }
    }

    // Ensure the pipe is registered for polling so async writes complete.
    let inner = get_inner((*pipe).loop_);
    if let Ok(mut handles) = inner.pipe_handles.try_borrow_mut()
      && !handles.iter().any(|&h| std::ptr::eq(h, pipe))
    {
      handles.push(pipe);
    }
    (*pipe).flags |= UV_HANDLE_ACTIVE;
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
    }
  }
  0
}
