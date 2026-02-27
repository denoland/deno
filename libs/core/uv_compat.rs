// Copyright 2018-2025 the Deno authors. MIT license.

// Drop-in replacement for libuv integrated with deno_core's event loop.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::ffi::c_void;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::time::Instant;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum uv_handle_type {
  UV_UNKNOWN_HANDLE = 0,
  UV_TIMER = 1,
  UV_IDLE = 2,
  UV_PREPARE = 3,
  UV_CHECK = 4,
  UV_TCP = 12,
}

const UV_HANDLE_ACTIVE: u32 = 1 << 0;
const UV_HANDLE_REF: u32 = 1 << 1;
const UV_HANDLE_CLOSING: u32 = 1 << 2;

// libuv-compatible error codes (negative errno values on unix,
// which vary depending on platform, fixed values on windows).
macro_rules! uv_errno {
  ($name:ident, $unix:expr, $win:expr) => {
    #[cfg(unix)]
    pub const $name: i32 = -($unix);
    #[cfg(windows)]
    pub const $name: i32 = $win;
  };
}

uv_errno!(UV_EAGAIN, libc::EAGAIN, -4088);
uv_errno!(UV_EBADF, libc::EBADF, -4083);
uv_errno!(UV_EADDRINUSE, libc::EADDRINUSE, -4091);
uv_errno!(UV_ECONNREFUSED, libc::ECONNREFUSED, -4078);
uv_errno!(UV_EINVAL, libc::EINVAL, -4071);
uv_errno!(UV_ENOTCONN, libc::ENOTCONN, -4053);
uv_errno!(UV_ECANCELED, libc::ECANCELED, -4081);
uv_errno!(UV_EPIPE, libc::EPIPE, -4047);
pub const UV_EOF: i32 = -4095;

#[repr(C)]
pub struct uv_loop_t {
  internal: *mut c_void,
  pub data: *mut c_void,
  stop_flag: Cell<bool>,
}

#[repr(C)]
pub struct uv_handle_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
}

#[repr(C)]
pub struct uv_timer_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  internal_id: u64,
  internal_deadline: u64,
  cb: Option<unsafe extern "C" fn(*mut uv_timer_t)>,
  timeout: u64,
  repeat: u64,
}

#[repr(C)]
pub struct uv_idle_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  cb: Option<unsafe extern "C" fn(*mut uv_idle_t)>,
}

#[repr(C)]
pub struct uv_prepare_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  cb: Option<unsafe extern "C" fn(*mut uv_prepare_t)>,
}

#[repr(C)]
pub struct uv_check_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  cb: Option<unsafe extern "C" fn(*mut uv_check_t)>,
}

#[repr(C)]
pub struct uv_stream_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
}

#[repr(C)]
pub struct uv_tcp_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  #[cfg(unix)]
  internal_fd: Option<std::os::unix::io::RawFd>,
  #[cfg(windows)]
  internal_fd: Option<std::os::windows::io::RawSocket>,
  internal_bind_addr: Option<SocketAddr>,
  internal_stream: Option<tokio::net::TcpStream>,
  internal_listener: Option<tokio::net::TcpListener>,
  internal_listener_addr: Option<SocketAddr>,
  internal_nodelay: bool,
  internal_alloc_cb: Option<uv_alloc_cb>,
  internal_read_cb: Option<uv_read_cb>,
  internal_reading: bool,
  internal_connect: Option<ConnectPending>,
  internal_write_queue: VecDeque<WritePending>,
  internal_connection_cb: Option<uv_connection_cb>,
  internal_backlog: VecDeque<tokio::net::TcpStream>,
}

/// In-flight TCP connect operation.
///
/// # Safety
/// `req` is a raw pointer to a caller-owned `uv_connect_t`. The caller must
/// ensure it remains valid until the connect callback fires (at which point
/// `ConnectPending` is consumed). This struct is `!Send` -- it lives on the
/// event loop thread alongside `UvLoopInner`.
struct ConnectPending {
  future: Pin<Box<dyn Future<Output = std::io::Result<tokio::net::TcpStream>>>>,
  req: *mut uv_connect_t,
  cb: Option<uv_connect_cb>,
}

/// Queued write operation waiting for the socket to become writable.
///
/// # Safety
/// `req` is a raw pointer to a caller-owned `uv_write_t`. The caller must
/// ensure it remains valid until the write callback fires (at which point
/// `WritePending` is consumed). This struct is `!Send`.
struct WritePending {
  req: *mut uv_write_t,
  data: Vec<u8>,
  offset: usize,
  cb: Option<uv_write_cb>,
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
/// Field order is `{base, len}` which matches the macOS/Windows layout.
/// On Linux, real libuv uses `{len, base}` (matching `struct iovec`).
/// This is fine as long as the struct is only constructed/consumed in Rust;
/// if it ever needs to cross an FFI boundary to real C code on Linux,
/// the field order must be made platform-conditional.
#[repr(C)]
pub struct uv_buf_t {
  pub base: *mut c_char,
  pub len: usize,
}

pub type uv_timer_cb = unsafe extern "C" fn(*mut uv_timer_t);
pub type uv_idle_cb = unsafe extern "C" fn(*mut uv_idle_t);
pub type uv_prepare_cb = unsafe extern "C" fn(*mut uv_prepare_t);
pub type uv_check_cb = unsafe extern "C" fn(*mut uv_check_t);
pub type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);
pub type uv_write_cb = unsafe extern "C" fn(*mut uv_write_t, i32);
pub type uv_alloc_cb =
  unsafe extern "C" fn(*mut uv_handle_t, usize, *mut uv_buf_t);
pub type uv_read_cb =
  unsafe extern "C" fn(*mut uv_stream_t, isize, *const uv_buf_t);
pub type uv_connection_cb = unsafe extern "C" fn(*mut uv_stream_t, i32);
pub type uv_connect_cb = unsafe extern "C" fn(*mut uv_connect_t, i32);
pub type uv_shutdown_cb = unsafe extern "C" fn(*mut uv_shutdown_t, i32);

pub type UvHandle = uv_handle_t;
pub type UvLoop = uv_loop_t;
pub type UvStream = uv_stream_t;
pub type UvTcp = uv_tcp_t;
pub type UvWrite = uv_write_t;
pub type UvBuf = uv_buf_t;
pub type UvConnect = uv_connect_t;
pub type UvShutdown = uv_shutdown_t;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TimerKey {
  deadline_ms: u64,
  id: u64,
}

pub(crate) struct UvLoopInner {
  timers: RefCell<BTreeSet<TimerKey>>,
  next_timer_id: Cell<u64>,
  timer_handles: RefCell<HashMap<u64, *mut uv_timer_t>>,
  idle_handles: RefCell<Vec<*mut uv_idle_t>>,
  prepare_handles: RefCell<Vec<*mut uv_prepare_t>>,
  check_handles: RefCell<Vec<*mut uv_check_t>>,
  tcp_handles: RefCell<Vec<*mut uv_tcp_t>>,
  waker: RefCell<Option<Waker>>,
  closing_handles: RefCell<VecDeque<(*mut uv_handle_t, Option<uv_close_cb>)>>,
  time_origin: Instant,
}

impl UvLoopInner {
  fn new() -> Self {
    Self {
      timers: RefCell::new(BTreeSet::new()),
      next_timer_id: Cell::new(1),
      timer_handles: RefCell::new(HashMap::with_capacity(16)),
      idle_handles: RefCell::new(Vec::with_capacity(8)),
      prepare_handles: RefCell::new(Vec::with_capacity(8)),
      check_handles: RefCell::new(Vec::with_capacity(8)),
      tcp_handles: RefCell::new(Vec::with_capacity(8)),
      waker: RefCell::new(None),
      closing_handles: RefCell::new(VecDeque::with_capacity(16)),
      time_origin: Instant::now(),
    }
  }

  pub(crate) fn set_waker(&self, waker: &Waker) {
    let mut slot = self.waker.borrow_mut();
    match slot.as_ref() {
      Some(existing) if existing.will_wake(waker) => {}
      _ => *slot = Some(waker.clone()),
    }
  }

  #[inline]
  fn alloc_timer_id(&self) -> u64 {
    let id = self.next_timer_id.get();
    self.next_timer_id.set(id + 1);
    id
  }

  #[inline]
  fn now_ms(&self) -> u64 {
    Instant::now().duration_since(self.time_origin).as_millis() as u64
  }

  pub(crate) fn has_alive_handles(&self) -> bool {
    for (_, handle_ptr) in self.timer_handles.borrow().iter() {
      // SAFETY: Handle pointers in timer_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    for handle_ptr in self.idle_handles.borrow().iter() {
      // SAFETY: Handle pointers in idle_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    for handle_ptr in self.prepare_handles.borrow().iter() {
      // SAFETY: Handle pointers in prepare_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    for handle_ptr in self.check_handles.borrow().iter() {
      // SAFETY: Handle pointers in check_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    for handle_ptr in self.tcp_handles.borrow().iter() {
      // SAFETY: Handle pointers in tcp_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    if !self.closing_handles.borrow().is_empty() {
      return true;
    }
    false
  }

  /// ### Safety
  /// All timer handle pointers stored in `timer_handles` must be valid.
  pub(crate) unsafe fn run_timers(&self) {
    let now = self.now_ms();
    let mut expired = Vec::new();
    {
      let timers = self.timers.borrow();
      for key in timers.iter() {
        if key.deadline_ms > now {
          break;
        }
        expired.push(*key);
      }
    }

    for key in expired {
      self.timers.borrow_mut().remove(&key);
      let handle_ptr = match self.timer_handles.borrow().get(&key.id).copied() {
        Some(h) => h,
        None => continue,
      };
      // SAFETY: handle_ptr comes from timer_handles; caller guarantees validity.
      let handle = unsafe { &mut *handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE == 0 {
        self.timer_handles.borrow_mut().remove(&key.id);
        continue;
      }
      let cb = handle.cb;
      let repeat = handle.repeat;

      if repeat > 0 {
        let new_deadline = now + repeat;
        let new_key = TimerKey {
          deadline_ms: new_deadline,
          id: key.id,
        };
        handle.internal_deadline = new_deadline;
        self.timers.borrow_mut().insert(new_key);
      } else {
        handle.flags &= !UV_HANDLE_ACTIVE;
        self.timer_handles.borrow_mut().remove(&key.id);
      }

      if let Some(cb) = cb {
        // SAFETY: handle_ptr is valid; cb was set by the C caller via uv_timer_start.
        unsafe { cb(handle_ptr) };
      }
    }
  }

  /// ### Safety
  /// All idle handle pointers stored in `idle_handles` must be valid.
  pub(crate) unsafe fn run_idle(&self) {
    let mut i = 0;
    loop {
      let handle_ptr = {
        let handles = self.idle_handles.borrow();
        if i >= handles.len() {
          break;
        }
        handles[i]
      };
      i += 1;
      // SAFETY: handle_ptr comes from idle_handles; caller guarantees validity.
      let handle = unsafe { &*handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && let Some(cb) = handle.cb
      {
        // SAFETY: Callback set by C caller via uv_idle_start; handle_ptr is valid.
        unsafe { cb(handle_ptr) };
      }
    }
  }

  /// ### Safety
  /// All prepare handle pointers stored in `prepare_handles` must be valid.
  pub(crate) unsafe fn run_prepare(&self) {
    let mut i = 0;
    loop {
      let handle_ptr = {
        let handles = self.prepare_handles.borrow();
        if i >= handles.len() {
          break;
        }
        handles[i]
      };
      i += 1;
      // SAFETY: handle_ptr comes from prepare_handles; caller guarantees validity.
      let handle = unsafe { &*handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && let Some(cb) = handle.cb
      {
        // SAFETY: Callback set by C caller via uv_prepare_start; handle_ptr is valid.
        unsafe { cb(handle_ptr) };
      }
    }
  }

  /// ### Safety
  /// All check handle pointers stored in `check_handles` must be valid.
  pub(crate) unsafe fn run_check(&self) {
    let mut i = 0;
    loop {
      let handle_ptr = {
        let handles = self.check_handles.borrow();
        if i >= handles.len() {
          break;
        }
        handles[i]
      };
      i += 1;
      // SAFETY: handle_ptr comes from check_handles; caller guarantees validity.
      let handle = unsafe { &*handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && let Some(cb) = handle.cb
      {
        // SAFETY: Callback set by C caller via uv_check_start; handle_ptr is valid.
        unsafe { cb(handle_ptr) };
      }
    }
  }

  /// ### Safety
  /// All handle pointers in `closing_handles` must be valid.
  pub(crate) unsafe fn run_close(&self) {
    let mut closing = self.closing_handles.borrow_mut();
    let snapshot: Vec<_> = closing.drain(..).collect();
    drop(closing);
    for (handle_ptr, cb) in snapshot {
      if let Some(cb) = cb {
        // SAFETY: handle_ptr is valid; cb was registered by C caller via uv_close.
        unsafe { cb(handle_ptr) };
      }
    }
  }

  /// Poll all TCP handles for I/O readiness and fire callbacks.
  ///
  /// Uses direct polling via tokio's `poll_accept`/`try_read`/`try_write`.
  /// No spawned tasks, no channels -- zero allocation in the hot path.
  ///
  /// Multiple passes: after callbacks fire they may produce new data
  /// (e.g. HTTP2 frame processing triggers writes which complete
  /// immediately). Re-poll up to 16 times to batch I/O within a
  /// single event loop tick.
  ///
  /// # Safety
  /// All TCP handle pointers in `tcp_handles` must be valid.
  pub(crate) unsafe fn run_io(&self) -> bool {
    let noop = Waker::noop();
    let waker_ref = self.waker.borrow();
    let waker = waker_ref.as_ref().unwrap_or(noop);
    let mut cx = Context::from_waker(waker);

    let mut did_any_work = false;

    for _pass in 0..16 {
      let mut any_work = false;

      let mut i = 0;
      loop {
        let tcp_ptr = {
          let handles = self.tcp_handles.borrow();
          if i >= handles.len() {
            break;
          }
          handles[i]
        };
        i += 1;
        // SAFETY: tcp_ptr comes from tcp_handles; caller guarantees validity.
        let tcp = unsafe { &mut *tcp_ptr };
        if tcp.flags & UV_HANDLE_ACTIVE == 0 {
          continue;
        }

        // 1. Poll pending connect
        if let Some(ref mut pending) = tcp.internal_connect
          && let Poll::Ready(result) = pending.future.as_mut().poll(&mut cx)
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
            Err(_) => UV_ECONNREFUSED,
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

        // 2. Poll listener for new connections
        if let Some(ref listener) = tcp.internal_listener
          && tcp.internal_connection_cb.is_some()
        {
          while let Poll::Ready(Ok((stream, _))) = listener.poll_accept(&mut cx)
          {
            tcp.internal_backlog.push_back(stream);
            any_work = true;
          }
          while !tcp.internal_backlog.is_empty() {
            if let Some(cb) = tcp.internal_connection_cb {
              // SAFETY: tcp_ptr is valid; cb set by C caller via uv_listen.
              unsafe { cb(tcp_ptr as *mut uv_stream_t, 0) };
            }
            // If uv_accept wasn't called in the callback, stop
            // to avoid an infinite loop.
            if !tcp.internal_backlog.is_empty() {
              break;
            }
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
              .poll_read_ready(&mut cx);

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
        if !tcp.internal_write_queue.is_empty() && tcp.internal_stream.is_some()
        {
          let stream = tcp.internal_stream.as_ref().unwrap();
          let _ = stream.poll_write_ready(&mut cx);

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
      } // end per-handle loop

      if !any_work {
        break;
      }
      did_any_work = true;
    } // end multi-pass loop

    did_any_work
  }

  /// ### Safety
  /// `handle` must be a valid pointer to an initialized `uv_timer_t`.
  unsafe fn stop_timer(&self, handle: *mut uv_timer_t) {
    // SAFETY: Caller guarantees handle is valid and initialized.
    let handle_ref = unsafe { &mut *handle };
    let id = handle_ref.internal_id;
    if id != 0 {
      let key = TimerKey {
        deadline_ms: handle_ref.internal_deadline,
        id,
      };
      self.timers.borrow_mut().remove(&key);
      self.timer_handles.borrow_mut().remove(&id);
    }
    handle_ref.flags &= !UV_HANDLE_ACTIVE;
  }

  fn stop_idle(&self, handle: *mut uv_idle_t) {
    self
      .idle_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      (*handle).flags &= !UV_HANDLE_ACTIVE;
    }
  }

  fn stop_prepare(&self, handle: *mut uv_prepare_t) {
    self
      .prepare_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      (*handle).flags &= !UV_HANDLE_ACTIVE;
    }
  }

  fn stop_check(&self, handle: *mut uv_check_t) {
    self
      .check_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      (*handle).flags &= !UV_HANDLE_ACTIVE;
    }
  }

  fn stop_tcp(&self, handle: *mut uv_tcp_t) {
    self
      .tcp_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      let tcp = &mut *handle;
      tcp.internal_reading = false;
      tcp.internal_alloc_cb = None;
      tcp.internal_read_cb = None;
      tcp.internal_connection_cb = None;
      tcp.internal_connect = None;
      tcp.internal_write_queue.clear();
      tcp.internal_stream = None;
      tcp.internal_listener = None;
      tcp.internal_backlog.clear();
      tcp.flags &= !UV_HANDLE_ACTIVE;
    }
  }
}

/// ### Safety
/// `loop_` must be a valid pointer to a `uv_loop_t` previously initialized by `uv_loop_init`.
#[inline]
unsafe fn get_inner(loop_: *mut uv_loop_t) -> &'static UvLoopInner {
  // SAFETY: Caller guarantees loop_ is valid and was initialized by uv_loop_init.
  unsafe { &*((*loop_).internal as *const UvLoopInner) }
}

/// ### Safety
/// `loop_` must be a valid pointer to a `uv_loop_t` previously initialized by `uv_loop_init`.
pub unsafe fn uv_loop_get_inner_ptr(
  loop_: *const uv_loop_t,
) -> *const std::ffi::c_void {
  // SAFETY: Caller guarantees loop_ is valid and was initialized by uv_loop_init.
  unsafe { (*loop_).internal as *const std::ffi::c_void }
}

/// ### Safety
/// `loop_` must be a valid, non-null pointer to an uninitialized `uv_loop_t`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_loop_init(loop_: *mut uv_loop_t) -> c_int {
  let inner = Box::new(UvLoopInner::new());
  // SAFETY: Caller guarantees loop_ is a valid, writable pointer.
  unsafe {
    (*loop_).internal = Box::into_raw(inner) as *mut c_void;
    (*loop_).data = std::ptr::null_mut();
    (*loop_).stop_flag = Cell::new(false);
  }
  0
}

/// ### Safety
/// `loop_` must be a valid pointer to a `uv_loop_t` initialized by `uv_loop_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_loop_close(loop_: *mut uv_loop_t) -> c_int {
  // SAFETY: Caller guarantees loop_ was initialized by uv_loop_init.
  unsafe {
    let internal = (*loop_).internal;
    if !internal.is_null() {
      drop(Box::from_raw(internal as *mut UvLoopInner));
      (*loop_).internal = std::ptr::null_mut();
    }
  }
  0
}

/// ### Safety
/// `loop_` must be a valid pointer to a `uv_loop_t` initialized by `uv_loop_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_now(loop_: *mut uv_loop_t) -> u64 {
  // SAFETY: Caller guarantees loop_ was initialized by uv_loop_init.
  let inner = unsafe { get_inner(loop_) };
  inner.now_ms()
}

/// ### Safety
/// `_loop_` must be a valid pointer to a `uv_loop_t` initialized by `uv_loop_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_update_time(_loop_: *mut uv_loop_t) {}

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `handle` must be a valid, writable pointer.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_init(
  loop_: *mut uv_loop_t,
  handle: *mut uv_timer_t,
) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid.
  unsafe {
    (*handle).r#type = uv_handle_type::UV_TIMER;
    (*handle).loop_ = loop_;
    (*handle).data = std::ptr::null_mut();
    (*handle).flags = UV_HANDLE_REF;
    (*handle).internal_id = 0;
    (*handle).internal_deadline = 0;
    (*handle).cb = None;
    (*handle).timeout = 0;
    (*handle).repeat = 0;
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_timer_t` initialized by `uv_timer_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_start(
  handle: *mut uv_timer_t,
  cb: uv_timer_cb,
  timeout: u64,
  repeat: u64,
) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_timer_init.
  unsafe {
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);

    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      inner.stop_timer(handle);
    }

    let id = inner.alloc_timer_id();
    let deadline = inner.now_ms() + timeout;

    (*handle).cb = Some(cb);
    (*handle).timeout = timeout;
    (*handle).repeat = repeat;
    (*handle).internal_id = id;
    (*handle).internal_deadline = deadline;
    (*handle).flags |= UV_HANDLE_ACTIVE;

    let key = TimerKey {
      deadline_ms: deadline,
      id,
    };
    inner.timers.borrow_mut().insert(key);
    inner.timer_handles.borrow_mut().insert(id, handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_timer_t` initialized by `uv_timer_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_stop(handle: *mut uv_timer_t) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_timer_init.
  unsafe {
    let loop_ = (*handle).loop_;
    if loop_.is_null() || (*loop_).internal.is_null() {
      (*handle).flags &= !UV_HANDLE_ACTIVE;
      return 0;
    }
    let inner = get_inner(loop_);
    inner.stop_timer(handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_timer_t` initialized by `uv_timer_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_again(handle: *mut uv_timer_t) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_timer_init.
  unsafe {
    let repeat = (*handle).repeat;
    if repeat == 0 {
      return UV_EINVAL;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);

    inner.stop_timer(handle);

    let id = inner.alloc_timer_id();
    let deadline = inner.now_ms() + repeat;

    (*handle).internal_id = id;
    (*handle).internal_deadline = deadline;
    (*handle).flags |= UV_HANDLE_ACTIVE;

    let key = TimerKey {
      deadline_ms: deadline,
      id,
    };
    inner.timers.borrow_mut().insert(key);
    inner.timer_handles.borrow_mut().insert(id, handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_timer_t` initialized by `uv_timer_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_get_repeat(handle: *const uv_timer_t) -> u64 {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe { (*handle).repeat }
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_timer_t` initialized by `uv_timer_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_timer_set_repeat(
  handle: *mut uv_timer_t,
  repeat: u64,
) {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    (*handle).repeat = repeat;
  }
}

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `handle` must be a valid, writable pointer.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_idle_init(
  loop_: *mut uv_loop_t,
  handle: *mut uv_idle_t,
) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid.
  unsafe {
    (*handle).r#type = uv_handle_type::UV_IDLE;
    (*handle).loop_ = loop_;
    (*handle).data = std::ptr::null_mut();
    (*handle).flags = UV_HANDLE_REF;
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_idle_t` initialized by `uv_idle_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_idle_start(
  handle: *mut uv_idle_t,
  cb: uv_idle_cb,
) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_idle_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      (*handle).cb = Some(cb);
      return 0;
    }
    (*handle).cb = Some(cb);
    (*handle).flags |= UV_HANDLE_ACTIVE;

    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.idle_handles.borrow_mut().push(handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_idle_t` initialized by `uv_idle_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_idle_stop(handle: *mut uv_idle_t) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_idle_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE == 0 {
      return 0;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.stop_idle(handle);
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `handle` must be a valid, writable pointer.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_prepare_init(
  loop_: *mut uv_loop_t,
  handle: *mut uv_prepare_t,
) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid.
  unsafe {
    (*handle).r#type = uv_handle_type::UV_PREPARE;
    (*handle).loop_ = loop_;
    (*handle).data = std::ptr::null_mut();
    (*handle).flags = UV_HANDLE_REF;
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_prepare_t` initialized by `uv_prepare_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_prepare_start(
  handle: *mut uv_prepare_t,
  cb: uv_prepare_cb,
) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_prepare_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      (*handle).cb = Some(cb);
      return 0;
    }
    (*handle).cb = Some(cb);
    (*handle).flags |= UV_HANDLE_ACTIVE;

    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.prepare_handles.borrow_mut().push(handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_prepare_t` initialized by `uv_prepare_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_prepare_stop(handle: *mut uv_prepare_t) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_prepare_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE == 0 {
      return 0;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.stop_prepare(handle);
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `handle` must be a valid, writable pointer.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_check_init(
  loop_: *mut uv_loop_t,
  handle: *mut uv_check_t,
) -> c_int {
  // SAFETY: Caller guarantees both pointers are valid.
  unsafe {
    (*handle).r#type = uv_handle_type::UV_CHECK;
    (*handle).loop_ = loop_;
    (*handle).data = std::ptr::null_mut();
    (*handle).flags = UV_HANDLE_REF;
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_check_t` initialized by `uv_check_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_check_start(
  handle: *mut uv_check_t,
  cb: uv_check_cb,
) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_check_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      (*handle).cb = Some(cb);
      return 0;
    }
    (*handle).cb = Some(cb);
    (*handle).flags |= UV_HANDLE_ACTIVE;

    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.check_handles.borrow_mut().push(handle);
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to a `uv_check_t` initialized by `uv_check_init`.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_check_stop(handle: *mut uv_check_t) -> c_int {
  // SAFETY: Caller guarantees handle was initialized by uv_check_init.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE == 0 {
      return 0;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);
    inner.stop_check(handle);
    (*handle).cb = None;
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to any uv handle type (timer, idle, tcp, etc.) initialized
/// by the corresponding `uv_*_init` function. Must not be called twice on the same handle.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_close(
  handle: *mut uv_handle_t,
  close_cb: Option<uv_close_cb>,
) {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    (*handle).flags |= UV_HANDLE_CLOSING;
    (*handle).flags &= !UV_HANDLE_ACTIVE;

    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);

    match (*handle).r#type {
      uv_handle_type::UV_TIMER => {
        inner.stop_timer(handle as *mut uv_timer_t);
      }
      uv_handle_type::UV_IDLE => {
        inner.stop_idle(handle as *mut uv_idle_t);
      }
      uv_handle_type::UV_PREPARE => {
        inner.stop_prepare(handle as *mut uv_prepare_t);
      }
      uv_handle_type::UV_CHECK => {
        inner.stop_check(handle as *mut uv_check_t);
      }
      uv_handle_type::UV_TCP => {
        inner.stop_tcp(handle as *mut uv_tcp_t);
      }
      _ => {}
    }

    inner
      .closing_handles
      .borrow_mut()
      .push_back((handle, close_cb));
  }
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized uv handle.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_ref(handle: *mut uv_handle_t) {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    (*handle).flags |= UV_HANDLE_REF;
  }
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized uv handle.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_unref(handle: *mut uv_handle_t) {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    (*handle).flags &= !UV_HANDLE_REF;
  }
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized uv handle.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_is_active(handle: *const uv_handle_t) -> c_int {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      1
    } else {
      0
    }
  }
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized uv handle.
#[cfg_attr(feature = "uv_compat_export", unsafe(no_mangle))]
pub unsafe extern "C" fn uv_is_closing(handle: *const uv_handle_t) -> c_int {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    if (*handle).flags & UV_HANDLE_CLOSING != 0 {
      1
    } else {
      0
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
    use std::ptr::{addr_of_mut, write};
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
      Err(_) => return UV_EADDRINUSE,
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

  if !tcp_ref.internal_write_queue.is_empty() {
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
      None => {
        if let Some(cb) = cb {
          cb(req, UV_ENOTCONN);
        }
        return 0;
      }
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

    let status = if let Some(ref stream) = (*tcp).internal_stream {
      #[cfg(unix)]
      {
        use std::os::unix::io::AsRawFd;
        let fd = stream.as_raw_fd();
        if libc::shutdown(fd, libc::SHUT_WR) == 0 {
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

    if let Some(cb) = cb {
      cb(req, status);
    }
  }
  0
}

pub fn new_tcp() -> UvTcp {
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
