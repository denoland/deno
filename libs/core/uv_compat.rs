// Copyright 2018-2026 the Deno authors. MIT license.

// Drop-in replacement for libuv integrated with deno_core's event loop.

mod stream;
mod tcp;
mod tty;

#[cfg(all(not(miri), test))]
mod tests;

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::c_int;
use std::ffi::c_void;
use std::task::Context;
use std::task::Waker;
use std::time::Instant;

pub use stream::*;
pub use tcp::*;
pub use tty::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum uv_handle_type {
  UV_UNKNOWN_HANDLE = 0,
  UV_TIMER = 1,
  UV_IDLE = 2,
  UV_PREPARE = 3,
  UV_CHECK = 4,
  UV_NAMED_PIPE = 7,
  UV_TCP = 12,
  UV_TTY = 13,
  UV_UDP = 15,
  UV_FILE = 17,
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
uv_errno!(UV_EBUSY, libc::EBUSY, -4082);
uv_errno!(UV_ENOBUFS, libc::ENOBUFS, -4060);
uv_errno!(UV_ENOTSUP, libc::ENOTSUP, -4049);
uv_errno!(UV_EALREADY, libc::EALREADY, -4084);
pub const UV_EOF: i32 = -4095;

/// Map a `std::io::Error` to the closest libuv error code.
pub(crate) fn io_error_to_uv(err: &std::io::Error) -> c_int {
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

pub type uv_timer_cb = unsafe extern "C" fn(*mut uv_timer_t);
pub type uv_idle_cb = unsafe extern "C" fn(*mut uv_idle_t);
pub type uv_prepare_cb = unsafe extern "C" fn(*mut uv_prepare_t);
pub type uv_check_cb = unsafe extern "C" fn(*mut uv_check_t);
pub type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);

pub type UvHandle = uv_handle_t;
pub type UvLoop = uv_loop_t;
pub type UvTcp = uv_tcp_t;

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
  tty_handles: RefCell<Vec<*mut uv_tty_t>>,
  waker: RefCell<Option<Waker>>,
  closing_handles: RefCell<VecDeque<(*mut uv_handle_t, Option<uv_close_cb>)>>,
  time_origin: Instant,
  /// Cached loop time in milliseconds. Updated once per tick via
  /// `update_time()`, matching libuv's `uv_update_time` semantics.
  cached_time_ms: Cell<u64>,
}

impl UvLoopInner {
  fn new() -> Self {
    let origin = Instant::now();
    Self {
      timers: RefCell::new(BTreeSet::new()),
      next_timer_id: Cell::new(1),
      timer_handles: RefCell::new(HashMap::with_capacity(16)),
      idle_handles: RefCell::new(Vec::with_capacity(8)),
      prepare_handles: RefCell::new(Vec::with_capacity(8)),
      check_handles: RefCell::new(Vec::with_capacity(8)),
      tcp_handles: RefCell::new(Vec::with_capacity(8)),
      tty_handles: RefCell::new(Vec::with_capacity(4)),
      waker: RefCell::new(None),
      closing_handles: RefCell::new(VecDeque::with_capacity(16)),
      time_origin: origin,
      cached_time_ms: Cell::new(0),
    }
  }

  pub(crate) fn set_waker(&self, waker: &Waker) {
    let mut slot = self.waker.borrow_mut();
    match slot.as_ref() {
      Some(existing) if existing.will_wake(waker) => {}
      _ => *slot = Some(waker.clone()),
    }
  }

  /// Wake the event loop so it re-polls on the next tick. Used on
  /// Windows to ensure pending TTY write callbacks are processed
  /// promptly when there is no async I/O notification mechanism.
  #[cfg(windows)]
  pub(crate) fn wake(&self) {
    if let Some(waker) = self.waker.borrow().as_ref() {
      waker.wake_by_ref();
    }
  }

  #[inline]
  fn alloc_timer_id(&self) -> u64 {
    let id = self.next_timer_id.get();
    self.next_timer_id.set(id + 1);
    id
  }

  /// Return the cached loop time. Matches libuv's `uv_now()` which
  /// returns the time cached at the start of the current tick.
  #[inline]
  fn now_ms(&self) -> u64 {
    self.cached_time_ms.get()
  }

  /// Re-read the wall clock and update the cached time.
  /// Matches libuv's `uv_update_time()`.
  #[inline]
  pub(crate) fn update_time(&self) {
    let ms = Instant::now().duration_since(self.time_origin).as_millis() as u64;
    self.cached_time_ms.set(ms);
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
    for handle_ptr in self.tty_handles.borrow().iter() {
      // SAFETY: Handle pointers in tty_handles are kept valid by the C caller until uv_close.
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
        if unsafe { (*tcp_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }

        // SAFETY: tcp_ptr is valid; checked above.
        let work = unsafe { tcp::poll_tcp_handle(tcp_ptr, &mut cx) };
        any_work |= work;
      } // end per-tcp-handle loop

      let mut j = 0;
      loop {
        let tty_ptr = {
          let handles = self.tty_handles.borrow();
          if j >= handles.len() {
            break;
          }
          handles[j]
        };
        j += 1;
        // SAFETY: tty_ptr comes from tty_handles; caller guarantees validity.
        if unsafe { (*tty_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }

        // SAFETY: tty_ptr is valid; checked above.
        any_work |= unsafe { tty::poll_tty_handle(tty_ptr, &mut cx) };
      } // end per-tty-handle loop

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

  fn stop_tty(&self, handle: *mut uv_tty_t) {
    self
      .tty_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      let tty = &mut *handle;

      // Always check if this fd is the globally tracked one, matching
      // libuv's unconditional check in uv__tty_close.
      #[cfg(unix)]
      {
        tty::restore_termios_on_close(tty.internal_fd);
      }

      tty.internal_reading = false;
      tty.internal_alloc_cb = None;
      tty.internal_read_cb = None;

      // Cancel in-flight write requests with UV_ECANCELED, matching libuv.
      while let Some(pw) = tty.internal_write_queue.pop_front() {
        if let Some(cb) = pw.cb {
          cb(pw.req, UV_ECANCELED);
        }
      }

      // Cancel pending shutdown with UV_ECANCELED.
      if let Some(pending) = tty.internal_shutdown.take()
        && let Some(cb) = pending.cb
      {
        cb(pending.req, UV_ECANCELED);
      }

      // Drop the reactor (AsyncFd or select fallback) to deregister
      // from the reactor, then close the fd.
      // Match libuv: do NOT close stdio fds (0, 1, 2).
      #[cfg(unix)]
      {
        // If using the select fallback, shut down the background thread.
        #[cfg(target_os = "macos")]
        if let Some(tty::TtyReactor::SelectFallback(ref mut s)) =
          tty.internal_reactor
        {
          tty::shutdown_select_fallback(s);
        }
        tty.internal_reactor = None;
        if tty.internal_fd > 2 {
          libc::close(tty.internal_fd);
          tty.internal_fd = -1;
        }
      }

      // Tear down Windows async read machinery, then close the handle.
      #[cfg(windows)]
      {
        tty::close_tty_read(handle);
        if !tty.internal_handle.is_null() {
          if tty.internal_handle_owned {
            // We duplicated this handle in init -- close it directly.
            tty::win_console::CloseHandle(tty.internal_handle);
          } else if tty.internal_fd >= 0 {
            // Non-duplicated: close through the CRT to free the fd slot.
            tty::win_console::_close(tty.internal_fd);
          }
          tty.internal_handle = std::ptr::null_mut();
          tty.internal_fd = -1;
        }
      }

      tty.flags &= !UV_HANDLE_ACTIVE;
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

      // Cancel in-flight connect request with UV_ECANCELED, matching libuv.
      if let Some(pending) = tcp.internal_connect.take()
        && let Some(cb) = pending.cb
      {
        cb(pending.req, UV_ECANCELED);
      }

      // Cancel in-flight write requests with UV_ECANCELED, matching libuv's
      // uv__stream_flush_write_queue() called from uv__stream_destroy().
      while let Some(pw) = tcp.internal_write_queue.pop_front() {
        if let Some(cb) = pw.cb {
          cb(pw.req, UV_ECANCELED);
        }
      }
      if let Some(stream) = tcp.internal_stream.take() {
        // Match libuv: just close the fd. The OS delivers FIN/RST to the
        // peer naturally; the peer's read loop detects EOF via recv()
        // returning 0.  libuv does NOT manually signal EOF to peer handles.
        if let Ok(std_stream) = stream.into_std() {
          let _ = std_stream.shutdown(std::net::Shutdown::Both);
        }
      }
      tcp.internal_socket = None;
      tcp.internal_delayed_error = 0;
      tcp.internal_listener = None;
      tcp.internal_backlog.clear();

      // Cancel pending shutdown with UV_ECANCELED.
      if let Some(pending) = tcp.internal_shutdown.take()
        && let Some(cb) = pending.cb
      {
        cb(pending.req, UV_ECANCELED);
      }

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

/// Matches libuv's `uv_guess_handle`: detects TTYs, regular files,
/// character devices, pipes (FIFOs), TCP/UDP sockets, and Unix domain
/// sockets (named pipes).
pub fn uv_guess_handle(fd: c_int) -> uv_handle_type {
  if fd < 0 {
    return uv_handle_type::UV_UNKNOWN_HANDLE;
  }

  #[cfg(unix)]
  {
    if unsafe { libc::isatty(fd) } != 0 {
      return uv_handle_type::UV_TTY;
    }

    let mut s: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut s) } != 0 {
      return uv_handle_type::UV_UNKNOWN_HANDLE;
    }

    let ft = s.st_mode & libc::S_IFMT;
    if ft == libc::S_IFREG || ft == libc::S_IFCHR {
      return uv_handle_type::UV_FILE;
    }

    if ft == libc::S_IFIFO {
      return uv_handle_type::UV_NAMED_PIPE;
    }

    if ft != libc::S_IFSOCK {
      return uv_handle_type::UV_UNKNOWN_HANDLE;
    }

    // It's a socket — determine type.
    let mut ss: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len: libc::socklen_t =
      std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    if unsafe {
      libc::getsockname(fd, &mut ss as *mut _ as *mut libc::sockaddr, &mut len)
    } != 0
    {
      return uv_handle_type::UV_UNKNOWN_HANDLE;
    }

    let mut sock_type: c_int = 0;
    let mut type_len: libc::socklen_t =
      std::mem::size_of::<c_int>() as libc::socklen_t;
    if unsafe {
      libc::getsockopt(
        fd,
        libc::SOL_SOCKET,
        libc::SO_TYPE,
        &mut sock_type as *mut _ as *mut c_void,
        &mut type_len,
      )
    } != 0
    {
      return uv_handle_type::UV_UNKNOWN_HANDLE;
    }

    if sock_type == libc::SOCK_DGRAM
      && (ss.ss_family == libc::AF_INET as libc::sa_family_t
        || ss.ss_family == libc::AF_INET6 as libc::sa_family_t)
    {
      return uv_handle_type::UV_UDP;
    }

    if sock_type == libc::SOCK_STREAM {
      if ss.ss_family == libc::AF_INET as libc::sa_family_t
        || ss.ss_family == libc::AF_INET6 as libc::sa_family_t
      {
        return uv_handle_type::UV_TCP;
      }
      if ss.ss_family == libc::AF_UNIX as libc::sa_family_t {
        return uv_handle_type::UV_NAMED_PIPE;
      }
    }

    uv_handle_type::UV_UNKNOWN_HANDLE
  }

  #[cfg(windows)]
  {
    let handle = unsafe { tty::win_console::safe_get_osfhandle(fd) };
    if handle == -1 {
      return uv_handle_type::UV_UNKNOWN_HANDLE;
    }
    let h = handle as *mut c_void;
    match unsafe { tty::win_console::GetFileType(h) } {
      tty::win_console::FILE_TYPE_CHAR => {
        let mut mode: u32 = 0;
        if unsafe { tty::win_console::GetConsoleMode(h, &mut mode) } != 0 {
          uv_handle_type::UV_TTY
        } else {
          uv_handle_type::UV_FILE
        }
      }
      tty::win_console::FILE_TYPE_PIPE => uv_handle_type::UV_NAMED_PIPE,
      tty::win_console::FILE_TYPE_DISK => uv_handle_type::UV_FILE,
      _ => uv_handle_type::UV_UNKNOWN_HANDLE,
    }
  }
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
      let inner = &*(internal as *const UvLoopInner);
      // Match libuv: return UV_EBUSY if handles or requests are still alive.
      if inner.has_alive_handles() {
        return UV_EBUSY;
      }
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
pub unsafe extern "C" fn uv_update_time(loop_: *mut uv_loop_t) {
  // SAFETY: Caller guarantees loop_ was initialized by uv_loop_init.
  let inner = unsafe { get_inner(loop_) };
  inner.update_time();
}

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
    if (*handle).flags & UV_HANDLE_CLOSING != 0 {
      return UV_EINVAL;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);

    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
      inner.stop_timer(handle);
    }

    let id = inner.alloc_timer_id();
    let now = inner.now_ms();
    let deadline = now.saturating_add(timeout);

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
    // Real libuv returns UV_EINVAL if the timer was never started (cb is NULL).
    if (*handle).cb.is_none() {
      return UV_EINVAL;
    }
    let repeat = (*handle).repeat;
    // When repeat is 0, uv_timer_again is a no-op (returns 0).
    if repeat == 0 {
      return 0;
    }
    let loop_ = (*handle).loop_;
    let inner = get_inner(loop_);

    inner.stop_timer(handle);

    let id = inner.alloc_timer_id();
    let now = inner.now_ms();
    let deadline = now.saturating_add(repeat);

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
    // Match libuv: no-op if already active.
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
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
    // Match libuv: no-op if already active.
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
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
    // Match libuv: no-op if already active.
    if (*handle).flags & UV_HANDLE_ACTIVE != 0 {
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

/// Manages the two libuv handles that implement setImmediate, matching
/// Node.js's architecture:
///
/// - **check handle** (`uv_check_t`): always started, always unref'd.
///   Participates in `run_check()` every iteration but never keeps the
///   event loop alive. The actual JS callback draining happens in Rust
///   after `run_check()`, gated on the immediate count.
///
/// - **idle handle** (`uv_idle_t`): started/stopped to control event loop
///   liveness. Started when refed immediates exist (keeps the loop alive),
///   stopped when none remain (allows exit). This matches Node.js's
///   `immediate_idle_handle_` + `ToggleImmediateRef()`.
pub(crate) struct ImmediateCheckHandle {
  check_handle: *mut uv_check_t,
  idle_handle: *mut uv_idle_t,
}

/// No-op callback for the check handle — the actual draining is done by
/// checking immediate_info counts after `run_check()` in the event loop.
unsafe extern "C" fn immediate_check_noop_cb(_: *mut uv_check_t) {}

/// No-op callback for the idle handle — its only purpose is to keep
/// the event loop alive when refed immediates exist.
unsafe extern "C" fn immediate_idle_noop_cb(_: *mut uv_idle_t) {}

impl ImmediateCheckHandle {
  /// Create and initialize both handles on the given loop.
  ///
  /// The check handle is immediately started and unref'd (always runs,
  /// never keeps the loop alive). The idle handle starts stopped.
  ///
  /// # Safety
  /// `loop_ptr` must be a valid, initialized `uv_loop_t`.
  /// The returned handles borrow from the loop and must not outlive it.
  pub unsafe fn new(loop_ptr: *mut uv_loop_t) -> Self {
    // Check handle: always started, always unref'd
    let check_handle = Box::into_raw(Box::new(unsafe {
      std::mem::MaybeUninit::<uv_check_t>::zeroed().assume_init()
    }));
    unsafe {
      uv_check_init(loop_ptr, check_handle);
      uv_unref(check_handle as *mut uv_handle_t);
      uv_check_start(check_handle, immediate_check_noop_cb);
    }

    // Idle handle: controls event loop liveness for refed immediates
    let idle_handle = Box::into_raw(Box::new(unsafe {
      std::mem::MaybeUninit::<uv_idle_t>::zeroed().assume_init()
    }));
    unsafe {
      uv_idle_init(loop_ptr, idle_handle);
      // Starts stopped — only started when refed immediates exist
    }

    Self {
      check_handle,
      idle_handle,
    }
  }

  /// Start the idle handle (keeps event loop alive for refed immediates).
  pub fn make_ref(&self) {
    // SAFETY: idle_handle is valid — set in new().
    unsafe {
      uv_idle_start(self.idle_handle, immediate_idle_noop_cb);
    }
  }

  /// Stop the idle handle (allows event loop to exit).
  pub fn make_unref(&self) {
    // SAFETY: idle_handle is valid — set in new().
    unsafe {
      uv_idle_stop(self.idle_handle);
    }
  }

  /// Stop both handles and free their heap allocations.
  ///
  /// # Safety
  /// Must be called before the owning uv loop is closed/dropped.
  /// Must not be called more than once.
  pub unsafe fn close(self) {
    unsafe {
      uv_check_stop(self.check_handle);
      drop(Box::from_raw(self.check_handle));
      uv_idle_stop(self.idle_handle);
      drop(Box::from_raw(self.idle_handle));
    }
  }
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
      uv_handle_type::UV_TTY => {
        inner.stop_tty(handle as *mut uv_tty_t);
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
pub unsafe extern "C" fn uv_has_ref(handle: *const uv_handle_t) -> c_int {
  // SAFETY: Caller guarantees handle is valid and initialized.
  unsafe {
    if (*handle).flags & UV_HANDLE_REF != 0 {
      1
    } else {
      0
    }
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

/// Counter for libuv-style async IDs (used by Node.js async_hooks).
/// Starts at 1 because that's the ID of the bootstrap execution context.
pub struct AsyncId(i64);

impl Default for AsyncId {
  fn default() -> Self {
    Self(1)
  }
}

impl AsyncId {
  /// Increment the internal id counter and return the value.
  #[allow(clippy::should_implement_trait, reason = "this is more clear")]
  pub fn next(&mut self) -> i64 {
    self.0 += 1;
    self.0
  }
}
