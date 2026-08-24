// Copyright 2018-2026 the Deno authors. MIT license.

// Drop-in replacement for libuv integrated with deno_core's event loop.

mod pipe;
#[cfg(unix)]
mod poll;
mod stream;
mod tcp;
mod tty;
mod waker;

#[cfg(all(not(miri), test))]
mod tests;

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::CStr;
use std::ffi::c_int;
use std::ffi::c_void;
use std::sync::Arc;
#[cfg(unix)]
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::MutexGuard;
#[cfg(unix)]
use std::sync::Weak;
#[cfg(unix)]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Waker;
use std::time::Duration;
use std::time::Instant;

pub use pipe::*;
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
  UV_POLL = 8,
  UV_TCP = 12,
  UV_TTY = 13,
  UV_UDP = 15,
  UV_FILE = 17,
}

const UV_HANDLE_ACTIVE: u32 = 1 << 0;
const UV_HANDLE_REF: u32 = 1 << 1;
const UV_HANDLE_CLOSING: u32 = 1 << 2;

pub const UV_READABLE: c_int = 1;
pub const UV_WRITABLE: c_int = 2;
pub const UV_DISCONNECT: c_int = 4;
pub const UV_PRIORITIZED: c_int = 8;

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
uv_errno!(UV_ENOENT, libc::ENOENT, -4058);
uv_errno!(UV_ENOTSOCK, libc::ENOTSOCK, -4050);
uv_errno!(UV_ECONNRESET, libc::ECONNRESET, -4077);
uv_errno!(UV_ECONNABORTED, libc::ECONNABORTED, -4079);
uv_errno!(UV_ETIMEDOUT, libc::ETIMEDOUT, -4039);
uv_errno!(UV_EACCES, libc::EACCES, -4092);
uv_errno!(UV_EEXIST, libc::EEXIST, -4075);
uv_errno!(UV_EFAULT, libc::EFAULT, -4074);
uv_errno!(UV_EIO, libc::EIO, -4070);
uv_errno!(UV_ENOMEM, libc::ENOMEM, -4057);
pub const UV_EOF: i32 = -4095;

pub fn uv_error_message(err: c_int) -> Option<&'static CStr> {
  let message = match err {
    x if x == UV_EAGAIN => c"resource temporarily unavailable",
    x if x == UV_EBADF => c"bad file descriptor",
    x if x == UV_EADDRINUSE => c"address already in use",
    x if x == UV_ECONNREFUSED => c"connection refused",
    x if x == UV_EINVAL => c"invalid argument",
    x if x == UV_ENOTCONN => c"socket is not connected",
    x if x == UV_ECANCELED => c"operation canceled",
    x if x == UV_EPIPE => c"broken pipe",
    x if x == UV_EBUSY => c"resource busy or locked",
    x if x == UV_ENOBUFS => c"no buffer space available",
    x if x == UV_ENOTSUP => c"operation not supported on socket",
    x if x == UV_EALREADY => c"connection already in progress",
    x if x == UV_ENOENT => c"no such file or directory",
    x if x == UV_ENOTSOCK => c"socket operation on non-socket",
    x if x == UV_ECONNRESET => c"connection reset by peer",
    x if x == UV_ECONNABORTED => c"software caused connection abort",
    x if x == UV_ETIMEDOUT => c"connection timed out",
    x if x == UV_EACCES => c"permission denied",
    x if x == UV_EEXIST => c"file already exists",
    x if x == UV_EFAULT => c"bad address in system call argument",
    x if x == UV_EIO => c"i/o error",
    x if x == UV_ENOMEM => c"not enough memory",
    x if x == UV_EOF => c"end of file",
    _ => return None,
  };
  Some(message)
}

/// Map a `std::io::Error` to the closest libuv error code.
pub(crate) fn io_error_to_uv(err: &std::io::Error) -> c_int {
  use std::io::ErrorKind;
  // On Windows, several Win32 error codes don't get a stable ErrorKind
  // mapping from std (they all end up as `Uncategorized`). Handle the
  // pipe-related ones explicitly first so they don't fall through to
  // the catch-all UV_EINVAL.
  #[cfg(windows)]
  if let Some(code) = err.raw_os_error() {
    match code {
      231 => return UV_EBUSY,  // ERROR_PIPE_BUSY
      536 => return UV_EAGAIN, // ERROR_PIPE_LISTENING
      230 => return UV_EPIPE,  // ERROR_BAD_PIPE
      // `ERROR_PIPE_NOT_CONNECTED` maps to `UV_EPIPE` to match libuv's
      // `uv_translate_sys_error` — node code pattern-matches against
      // libuv's actual values, so semantic accuracy (`UV_ENOTCONN`)
      // would break those callers.
      233 => return UV_EPIPE, // ERROR_PIPE_NOT_CONNECTED
      _ => {}
    }
  }
  match err.kind() {
    ErrorKind::AddrInUse => UV_EADDRINUSE,
    ErrorKind::AddrNotAvailable => UV_EINVAL,
    ErrorKind::ConnectionRefused => UV_ECONNREFUSED,
    ErrorKind::ConnectionReset => UV_ECONNRESET,
    ErrorKind::ConnectionAborted => UV_ECONNABORTED,
    ErrorKind::NotConnected => UV_ENOTCONN,
    ErrorKind::NotFound => UV_ENOENT,
    ErrorKind::BrokenPipe => UV_EPIPE,
    ErrorKind::InvalidInput => UV_EINVAL,
    ErrorKind::WouldBlock => UV_EAGAIN,
    ErrorKind::TimedOut => UV_ETIMEDOUT,
    ErrorKind::PermissionDenied => UV_EACCES,
    _ => {
      // On Unix, try to use the raw OS error for a more accurate mapping.
      #[cfg(unix)]
      if let Some(code) = err.raw_os_error() {
        return -code;
      }
      // On Windows, map common Winsock errors to libuv codes.
      #[cfg(windows)]
      if let Some(code) = err.raw_os_error() {
        return match code {
          10054 => UV_ECONNRESET,   // WSAECONNRESET
          10053 => UV_ECONNABORTED, // WSAECONNABORTED
          10061 => UV_ECONNREFUSED, // WSAECONNREFUSED
          10048 => UV_EADDRINUSE,   // WSAEADDRINUSE
          10060 => UV_ETIMEDOUT,    // WSAETIMEDOUT
          10057 => UV_ENOTCONN,     // WSAENOTCONN
          10038 => UV_ENOTSOCK,     // WSAENOTSOCK
          10035 => UV_EAGAIN,       // WSAEWOULDBLOCK
          _ => UV_EINVAL,
        };
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

impl Drop for uv_loop_t {
  fn drop(&mut self) {
    if !self.internal.is_null() {
      // SAFETY: `internal` was allocated by `uv_loop_init` as
      // `Box::into_raw(Box::new(UvLoopInner::new()))`. We must free it
      // unconditionally during drop — unlike `uv_loop_close` which
      // returns UV_EBUSY when handles are still alive, there is no way
      // to signal failure from Drop.
      unsafe {
        drop(Box::from_raw(self.internal as *mut UvLoopInner));
      }
      self.internal = std::ptr::null_mut();
    }
  }
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

#[cfg(unix)]
pub type uv_poll_cb = unsafe extern "C" fn(*mut uv_poll_t, c_int, c_int);

/// An owner can suppress readiness for all poll handles created for one
/// embedding scope.
///
/// Invalidation removes matching worker watches and rejects queued readiness
/// and new starts. Loop-side handle state and fd ownership remain until each
/// handle is stopped or closed. The worker sees only this numeric owner id; it
/// never retains an addon or ABI-handle pointer.
#[cfg(unix)]
#[derive(Clone)]
pub struct UvPollOwner {
  id: u64,
  live: Arc<AtomicBool>,
  control: Weak<poll::PollControl>,
}

#[cfg(unix)]
pub struct UvLoopLiveness {
  live: AtomicBool,
  lock: Mutex<()>,
}

#[cfg(unix)]
impl UvLoopLiveness {
  fn new() -> Self {
    Self {
      live: AtomicBool::new(true),
      lock: Mutex::new(()),
    }
  }
  fn invalidate(&self) {
    let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
    self.live.store(false, Ordering::Release);
  }
}

#[cfg(unix)]
/// Prevents loop teardown while the caller accesses loop-associated external
/// state. Returns `None` once teardown has begun; callers must hold the guard
/// for their entire operation.
pub fn uv_loop_operation_guard(
  liveness: &UvLoopLiveness,
) -> Option<MutexGuard<'_, ()>> {
  let guard = liveness.lock.lock().unwrap_or_else(|p| p.into_inner());
  if liveness.live.load(Ordering::Acquire) {
    Some(guard)
  } else {
    None
  }
}

#[cfg(unix)]
impl UvPollOwner {
  pub fn invalidate(&self) {
    self.live.store(false, Ordering::Release);
    if let Some(control) = self.control.upgrade() {
      control.stop_owner(self.id);
    }
  }
}

/// Caller storage containing this handle must remain valid until
/// `uv_poll_close` removes it from the loop registry or loop teardown
/// invalidates `UvLoopLiveness`.
#[cfg(unix)]
#[repr(C)]
pub struct uv_poll_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,
  // The token identifies this handle without sharing its ABI pointer with the
  // worker. The generation identifies its current start/update, so readiness
  // queued for an older watch can be discarded after stop or restart.
  internal_token: u64,
  internal_generation: u64,
  internal_fd: c_int,
  internal_events: c_int,
  internal_cb: Option<uv_poll_cb>,
  internal_stop_cb: Option<unsafe extern "C" fn(*mut uv_poll_t)>,
  internal_owner: Option<UvPollOwner>,
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
  #[cfg(unix)]
  liveness: Arc<UvLoopLiveness>,
  timers: RefCell<BTreeSet<TimerKey>>,
  next_timer_id: Cell<u64>,
  timer_handles: RefCell<HashMap<u64, *mut uv_timer_t>>,
  idle_handles: RefCell<Vec<*mut uv_idle_t>>,
  prepare_handles: RefCell<Vec<*mut uv_prepare_t>>,
  check_handles: RefCell<Vec<*mut uv_check_t>>,
  tcp_handles: RefCell<Vec<*mut uv_tcp_t>>,
  pipe_handles: RefCell<Vec<*mut uv_pipe_t>>,
  tty_handles: RefCell<Vec<*mut uv_tty_t>>,
  #[cfg(unix)]
  poll_driver: RefCell<poll::PollDriver>,
  #[cfg(unix)]
  poll_handles: RefCell<HashMap<u64, *mut uv_poll_t>>,
  // Mirrors libuv's loop-wide uv__fd_exists duplicate-fd registry rather than
  // poll-local ownership. Only poll handles participate today. An entry
  // reserves its raw fd until stop or close; closing the fd first can let OS
  // reuse make a later setup fail with UV_EEXIST.
  #[cfg(unix)]
  fd_watchers: RefCell<HashMap<c_int, u64>>,
  #[cfg(unix)]
  next_poll_token: Cell<u64>,
  #[cfg(unix)]
  next_poll_owner: Cell<u64>,
  waker: RefCell<Option<Waker>>,
  closing_handles: RefCell<VecDeque<(*mut uv_handle_t, Option<uv_close_cb>)>>,
  time_origin: Instant,
  /// Cached loop time in milliseconds. Updated once per tick via
  /// `update_time()`, matching libuv's `uv_update_time` semantics.
  cached_time_ms: Cell<u64>,
  /// Shared state between the loop and per-handle wakers. Waker
  /// callbacks may fire from tokio's reactor thread, so access goes
  /// through this Send+Sync struct instead of `UvLoopInner` directly.
  pub(crate) shared: std::sync::Arc<waker::LoopShared>,
}

impl UvLoopInner {
  fn new() -> Self {
    let origin = Instant::now();
    let shared = waker::LoopShared::new();
    #[cfg(unix)]
    let poll_driver = poll::PollDriver::new(shared.clone());
    Self {
      #[cfg(unix)]
      liveness: Arc::new(UvLoopLiveness::new()),
      timers: RefCell::new(BTreeSet::new()),
      next_timer_id: Cell::new(1),
      timer_handles: RefCell::new(HashMap::with_capacity(16)),
      idle_handles: RefCell::new(Vec::with_capacity(8)),
      prepare_handles: RefCell::new(Vec::with_capacity(8)),
      check_handles: RefCell::new(Vec::with_capacity(8)),
      tcp_handles: RefCell::new(Vec::with_capacity(8)),
      pipe_handles: RefCell::new(Vec::with_capacity(4)),
      tty_handles: RefCell::new(Vec::with_capacity(4)),
      #[cfg(unix)]
      poll_driver: RefCell::new(poll_driver),
      #[cfg(unix)]
      poll_handles: RefCell::new(HashMap::with_capacity(4)),
      #[cfg(unix)]
      fd_watchers: RefCell::new(HashMap::with_capacity(4)),
      #[cfg(unix)]
      next_poll_token: Cell::new(1),
      #[cfg(unix)]
      next_poll_owner: Cell::new(1),
      waker: RefCell::new(None),
      closing_handles: RefCell::new(VecDeque::with_capacity(16)),
      time_origin: origin,
      cached_time_ms: Cell::new(0),
      shared,
    }
  }

  pub(crate) fn set_waker(&self, waker: &Waker) {
    let mut slot = self.waker.borrow_mut();
    match slot.as_ref() {
      Some(existing) if existing.will_wake(waker) => {}
      _ => *slot = Some(waker.clone()),
    }
    // Register with the shared AtomicWaker too, so handle wakers
    // (which run on tokio's reactor thread) can wake the loop.
    self.shared.loop_waker.register(waker);
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

  /// The earliest pending timer, as its absolute deadline (loop-time ms) and
  /// the delay from now until it fires (`0` if already due). Returns `None`
  /// when no timer is scheduled. Mirrors libuv's `uv__next_timeout`, which the
  /// event loop uses to bound how long it may block before the next timer.
  ///
  /// The deadline is returned alongside the delay so the caller can detect
  /// when the earliest deadline changes and avoid re-arming its wakeup every
  /// tick. `timers` is ordered by `(deadline_ms, id)`, so the front entry is
  /// always the soonest.
  pub(crate) fn next_timeout(&self) -> Option<(u64, Duration)> {
    let deadline = self.timers.borrow().iter().next()?.deadline_ms;
    Some((
      deadline,
      Duration::from_millis(deadline.saturating_sub(self.now_ms())),
    ))
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
    for handle_ptr in self.pipe_handles.borrow().iter() {
      // SAFETY: Handle pointers in pipe_handles are kept valid by the C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
      {
        return true;
      }
    }
    #[cfg(unix)]
    for handle_ptr in self.poll_handles.borrow().values() {
      // SAFETY: Poll pointers are retained by their C caller until uv_close.
      let handle = unsafe { &**handle_ptr };
      if handle.flags & UV_HANDLE_ACTIVE != 0
        && handle.flags & UV_HANDLE_REF != 0
        // Owner invalidation is an Env-scoped close signal. The handle can
        // remain allocated until its bridge processes close, but it must not
        // keep this runtime alive after that owner is gone.
        && handle
          .internal_owner
          .as_ref()
          .is_some_and(|owner| owner.live.load(Ordering::Acquire))
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
    // Drain ready queues once. Each popped handle is polled with a
    // `Context` built from its own per-handle waker; tokio re-registers
    // interest under that waker so the next readiness signal re-queues
    // the handle for the next tick.
    //
    // Validate each popped pointer against the authoritative `*_handles`
    // list before dereferencing — the handle may have been closed
    // between a waker fire and the drain.
    //
    // One pass only: looping in place (e.g. `for _ in 0..16`) batches
    // more I/O per event-loop tick but under sustained HTTP load the
    // ready queues refill as tokio delivers new readiness while we're
    // still polling, so the loop never exits. Handles whose data lands
    // mid-tick then wait for the entire batch to drain before their
    // response fires, pushing p99 from ~1 ms (Node-matching) to 30–200
    // ms at 50 concurrent connections. Libuv's uv_run does one
    // uv__io_poll per iteration for the same reason.
    {
      let mut any_work = false;

      // --- TCP ---
      // IMPORTANT: always call `reset_queued()` on every popped
      // handle, even ones we skip (detached or inactive). If we
      // skip without resetting, the per-handle `in_queue` flag
      // stays `true` and a later `mark_ready()` from the same
      // handle (e.g. after `uv_read_stop` cleared ACTIVE and then
      // `uv_read_start` re-armed it) becomes a no-op — the handle
      // never gets enqueued again and the event loop stops polling
      // it entirely.
      //
      // Liveness is established by `handle_waker.live_ptr()`: the
      // waker's ptr field is zeroed by `detach()` when the handle is
      // torn down, so queued-then-closed entries self-invalidate.
      // The Arc keeps the waker allocation alive across the handle's
      // destruction, so the atomic load is always safe.
      let mut tcp_ready: VecDeque<Arc<waker::TcpHandleWaker>> =
        std::mem::take(&mut *self.shared.ready_tcp.lock().unwrap());
      while let Some(handle_waker) = tcp_ready.pop_front() {
        handle_waker.reset_queued();
        let ptr = handle_waker.live_ptr();
        if ptr == 0 {
          continue;
        }
        let tcp_ptr = ptr as *mut uv_tcp_t;
        // SAFETY: ptr is non-zero, so the handle has not been detached.
        if unsafe { (*tcp_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }
        let waker = std::task::Waker::from(handle_waker);
        let mut cx = Context::from_waker(&waker);
        // SAFETY: tcp_ptr is live per the ptr check above.
        any_work |= unsafe { tcp::poll_tcp_handle(tcp_ptr, &mut cx) };
      }

      // --- pipe ---
      let mut pipe_ready: VecDeque<Arc<waker::PipeHandleWaker>> =
        std::mem::take(&mut *self.shared.ready_pipe.lock().unwrap());
      while let Some(handle_waker) = pipe_ready.pop_front() {
        handle_waker.reset_queued();
        let ptr = handle_waker.live_ptr();
        if ptr == 0 {
          continue;
        }
        let pipe_ptr = ptr as *mut uv_pipe_t;
        // SAFETY: ptr is non-zero.
        if unsafe { (*pipe_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }
        let waker = std::task::Waker::from(handle_waker);
        let mut cx = Context::from_waker(&waker);
        // SAFETY: pipe_ptr is live.
        any_work |= unsafe { pipe::poll_pipe_handle(pipe_ptr, &mut cx) };
      }

      // --- TTY ---
      let mut tty_ready: VecDeque<Arc<waker::TtyHandleWaker>> =
        std::mem::take(&mut *self.shared.ready_tty.lock().unwrap());
      while let Some(handle_waker) = tty_ready.pop_front() {
        handle_waker.reset_queued();
        let ptr = handle_waker.live_ptr();
        if ptr == 0 {
          continue;
        }
        let tty_ptr = ptr as *mut uv_tty_t;
        // SAFETY: ptr is non-zero.
        if unsafe { (*tty_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }
        let waker = std::task::Waker::from(handle_waker);
        let mut cx = Context::from_waker(&waker);
        // SAFETY: tty_ptr is live.
        any_work |= unsafe { tty::poll_tty_handle(tty_ptr, &mut cx) };
      }

      #[cfg(unix)]
      {
        // Preserve one-pass I/O ordering: tokio-backed TCP/pipe/TTY callbacks
        // run before worker-driven poll callbacks.
        any_work |= unsafe { self.run_poll_io() };
      }

      any_work
    }
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
      if let Some(w) = tty.internal_waker.take() {
        w.detach();
      }

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
      // Detach the per-handle waker so any late wake from tokio's
      // reactor becomes a no-op. The Arc is dropped via take() so
      // the waker memory is released.
      if let Some(w) = tcp.internal_waker.take() {
        w.detach();
      }
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
      // Match libuv's uv__stream_close: plain close(2) on the fd, no
      // shutdown. The kernel emits FIN when the kernel-level open file
      // description (OFD) refcount reaches zero, which handles both regular
      // closes and IPC-transferred fds (where the receiver still holds a dup,
      // so FIN is correctly suppressed).
      drop(tcp.internal_stream.take());
      tcp.internal_fd = None;
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

  fn stop_pipe(&self, handle: *mut uv_pipe_t) {
    self
      .pipe_handles
      .borrow_mut()
      .retain(|&h| !std::ptr::eq(h, handle));
    // SAFETY: Caller guarantees handle is valid and initialized.
    unsafe {
      if let Some(w) = (*handle).internal_waker.take() {
        w.detach();
      }
      // Cancel in-flight write requests with UV_ECANCELED.
      while let Some(pw) = (*handle).internal_write_queue.pop_front() {
        if let Some(cb) = pw.cb {
          cb(pw.req, UV_ECANCELED);
        }
      }
      // Close the pipe fd and deregister from epoll/kqueue.
      pipe::close_pipe(handle);
      (*handle).flags &= !UV_HANDLE_ACTIVE;
    }
  }

  #[cfg(unix)]
  unsafe fn stop_poll(&self, handle: *mut uv_poll_t) {
    // SAFETY: Caller guarantees handle is valid and initialized.
    let handle_ref = unsafe { &mut *handle };
    let generation = handle_ref.internal_generation;
    handle_ref.internal_generation = generation.wrapping_add(1);
    handle_ref.flags &= !UV_HANDLE_ACTIVE;
    handle_ref.internal_events = 0;
    handle_ref.internal_cb = None;

    // Release fd ownership before notifying the embedding: its stop hook or a
    // subsequent error callback may synchronously install a replacement.
    let mut watchers = self.fd_watchers.borrow_mut();
    if watchers
      .get(&handle_ref.internal_fd)
      .is_some_and(|token| *token == handle_ref.internal_token)
    {
      watchers.remove(&handle_ref.internal_fd);
    }
    drop(watchers);
    self
      .poll_driver
      .borrow()
      .stop(handle_ref.internal_token, generation);
    if let Some(callback) = handle_ref.internal_stop_cb {
      unsafe { callback(handle) };
    }
  }

  /// Drain exactly one worker-ready snapshot and invoke callbacks on the
  /// loop thread. No registry borrow survives a callback: it may close or
  /// restart the same handle, including through a higher-level ABI bridge.
  #[cfg(unix)]
  unsafe fn run_poll_io(&self) -> bool {
    let ready = self.poll_driver.borrow().drain_ready();
    let mut any_work = false;

    for ready in ready {
      let snapshot = unsafe {
        let handles = self.poll_handles.borrow();
        if let Some(handle_ptr) = handles.get(&ready.token) {
          let handle = &mut **handle_ptr;
          if handle.internal_token != ready.token
            || handle.internal_generation != ready.generation
            || handle.flags & UV_HANDLE_ACTIVE == 0
            || !handle
              .internal_owner
              .as_ref()
              .is_some_and(|owner| owner.live.load(Ordering::Acquire))
          {
            None
          } else {
            let (status, events) = if ready.status != 0 {
              (ready.status, 0)
            } else {
              poll::poll_revents_to_uv_callback_args(
                ready.revents,
                handle.internal_events,
              )
            };
            handle
              .internal_cb
              .map(|callback| (*handle_ptr, callback, status, events))
          }
        } else {
          None
        }
      };
      let Some((handle_ptr, callback, status, events)) = snapshot else {
        continue;
      };

      if status != 0 {
        // Stop first so the poll callback can install a replacement watch.
        unsafe { self.stop_poll(handle_ptr) };
      }
      // SAFETY: Callback and pointer were copied while the registry entry was
      // live. The C caller keeps the handle valid until its close callback.
      unsafe { callback(handle_ptr, status, events) };
      any_work = true;

      let should_rearm = unsafe {
        let handles = self.poll_handles.borrow();
        handles.get(&ready.token).is_some_and(|handle_ptr| {
          let handle = &**handle_ptr;
          handle.internal_token == ready.token
            && handle.internal_generation == ready.generation
            && handle.flags & UV_HANDLE_ACTIVE != 0
            && handle
              .internal_owner
              .as_ref()
              .is_some_and(|owner| owner.live.load(Ordering::Acquire))
        })
      };
      if should_rearm
        && self
          .poll_driver
          .borrow()
          .rearm(ready.token, ready.generation)
          .is_err()
      {
        // The worker has entered a terminal state after this callback was
        // delivered. It cannot accept the rearm, so release loop ownership
        // instead of leaving a referenced but unserviceable handle alive.
        unsafe { self.stop_poll(handle_ptr) };
      }
    }

    any_work
  }
}

#[cfg(unix)]
impl Drop for UvLoopInner {
  fn drop(&mut self) {
    self.liveness.invalidate();
    // Join before the token registry and LoopShared are dropped. The worker
    // only owns tokens, but it can still wake LoopShared while shutting down.
    self.poll_driver.get_mut().shutdown();
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

/// Create an invalidation owner for core poll handles on `loop_`.
///
/// # Safety
/// `loop_` must be a valid loop initialized by `uv_loop_init`.
#[cfg(unix)]
pub unsafe fn new_poll_owner(loop_: *mut uv_loop_t) -> UvPollOwner {
  // SAFETY: Caller guarantees loop_ is initialized.
  let inner = unsafe { get_inner(loop_) };
  let id = inner.next_poll_owner.get();
  inner.next_poll_owner.set(id.wrapping_add(1));
  UvPollOwner {
    id,
    live: Arc::new(AtomicBool::new(true)),
    control: inner.poll_driver.borrow().control(),
  }
}

/// Returns the shared operation/lifetime token for an initialized uv loop.
///
/// # Safety
///
/// `loop_` must point to a live initialized loop.
#[cfg(unix)]
pub unsafe fn uv_loop_liveness(loop_: *mut uv_loop_t) -> Arc<UvLoopLiveness> {
  unsafe { get_inner(loop_) }.liveness.clone()
}

/// Initialize a Unix poll handle managed by the core loop.
///
/// # Safety
/// `loop_` must be initialized and `handle` must be valid writable storage.
#[cfg(unix)]
pub unsafe fn uv_poll_init(
  loop_: *mut uv_loop_t,
  handle: *mut uv_poll_t,
  fd: c_int,
  owner: UvPollOwner,
) -> c_int {
  // SAFETY: Caller guarantees loop_ and handle are valid.
  let inner = unsafe { get_inner(loop_) };
  // Match libuv's uv__fd_exists check before changing descriptor flags or
  // registering this handle. An active watch owns its fd for this loop.
  if inner.fd_watchers.borrow().contains_key(&fd) {
    return UV_EEXIST;
  }
  if let Err(errno) = poll::set_fd_nonblocking(fd) {
    return -errno;
  }
  let token = inner.next_poll_token.get();
  inner.next_poll_token.set(token.wrapping_add(1));
  // SAFETY: handle is valid writable storage supplied by the caller.
  unsafe {
    std::ptr::write(
      handle,
      uv_poll_t {
        r#type: uv_handle_type::UV_POLL,
        loop_,
        data: std::ptr::null_mut(),
        flags: UV_HANDLE_REF,
        internal_token: token,
        internal_generation: 0,
        internal_fd: fd,
        internal_events: 0,
        internal_cb: None,
        internal_stop_cb: None,
        internal_owner: Some(owner),
      },
    );
  }
  inner.poll_handles.borrow_mut().insert(token, handle);
  0
}

/// Install a loop-thread notification invoked whenever a poll handle stops.
///
/// # Safety
///
/// `handle` must be a live initialized poll handle.
#[cfg(unix)]
pub unsafe fn uv_poll_set_stop_callback(
  handle: *mut uv_poll_t,
  callback: Option<unsafe extern "C" fn(*mut uv_poll_t)>,
) {
  unsafe { (*handle).internal_stop_cb = callback };
}

/// Start or update a core poll handle.
///
/// # Safety
/// `handle` must have been initialized by `uv_poll_init` and remain valid.
#[cfg(unix)]
pub unsafe fn uv_poll_start(
  handle: *mut uv_poll_t,
  events: c_int,
  cb: Option<uv_poll_cb>,
) -> c_int {
  const VALID_EVENTS: c_int =
    UV_READABLE | UV_WRITABLE | UV_DISCONNECT | UV_PRIORITIZED;
  // SAFETY: Caller guarantees handle is valid and initialized.
  let handle_ref = unsafe { &mut *handle };
  if handle_ref.flags & UV_HANDLE_CLOSING != 0 || events & !VALID_EVENTS != 0 {
    return UV_EINVAL;
  }
  // SAFETY: An initialized poll handle always records its initialized loop.
  let inner = unsafe { get_inner(handle_ref.loop_) };
  let Some(owner) = handle_ref.internal_owner.as_ref() else {
    return UV_EINVAL;
  };
  if !owner.live.load(Ordering::Acquire) {
    return UV_ECANCELED;
  }

  // libuv validates fd ownership before treating a zero mask as stop.
  if inner
    .fd_watchers
    .borrow()
    .get(&handle_ref.internal_fd)
    .is_some_and(|token| *token != handle_ref.internal_token)
  {
    return UV_EEXIST;
  }
  // libuv stores a null callback without validation and would later invoke it
  // during event dispatch. Reject it synchronously instead.
  if events != 0 && cb.is_none() {
    return UV_EINVAL;
  }
  if events == 0 {
    return unsafe { uv_poll_stop(handle) };
  }

  let generation = handle_ref.internal_generation.wrapping_add(1);
  let watch = poll::PollWatch {
    token: handle_ref.internal_token,
    generation,
    owner: owner.id,
    fd: handle_ref.internal_fd,
    events: poll::uv_events_to_poll_events(events),
  };
  // Commit loop state only after lazy worker startup accepts the watch, so a
  // failure leaves an existing registration, generation, and fd owner intact.
  if let Err(errno) = inner.poll_driver.borrow_mut().upsert(watch, &owner.live)
  {
    return -errno;
  }
  handle_ref.internal_generation = generation;
  handle_ref.internal_events = events;
  handle_ref.internal_cb = cb;
  handle_ref.flags |= UV_HANDLE_ACTIVE;
  inner
    .fd_watchers
    .borrow_mut()
    .insert(handle_ref.internal_fd, handle_ref.internal_token);
  0
}

/// Stop a core poll handle and invalidate its currently queued generation.
///
/// # Safety
/// `handle` must have been initialized by `uv_poll_init` and remain valid.
#[cfg(unix)]
pub unsafe fn uv_poll_stop(handle: *mut uv_poll_t) -> c_int {
  // SAFETY: Caller guarantees handle is valid and initialized.
  let loop_ = unsafe { (*handle).loop_ };
  if loop_.is_null() || unsafe { (*loop_).internal.is_null() } {
    // SAFETY: handle is valid per this function's contract.
    unsafe { (*handle).flags &= !UV_HANDLE_ACTIVE };
    return 0;
  }
  // SAFETY: loop_ is initialized while its internal pointer is non-null.
  let inner = unsafe { get_inner(loop_) };
  unsafe { inner.stop_poll(handle) };
  0
}

/// Remove a core poll handle from its loop registry.
///
/// # Safety
/// `handle` must have been initialized by `uv_poll_init` and remain valid.
#[cfg(unix)]
pub unsafe fn uv_poll_close(handle: *mut uv_poll_t) {
  // SAFETY: Caller guarantees handle is valid and initialized.
  let loop_ = unsafe { (*handle).loop_ };
  if !loop_.is_null() && unsafe { !(*loop_).internal.is_null() } {
    // SAFETY: loop_ is initialized while its internal pointer is non-null.
    let inner = unsafe { get_inner(loop_) };
    unsafe { inner.stop_poll(handle) };
    // SAFETY: handle remains valid until its caller completes close handling.
    inner
      .poll_handles
      .borrow_mut()
      .remove(&unsafe { (*handle).internal_token });
  }
  // SAFETY: handle is valid per this function's contract.
  unsafe {
    (*handle).flags |= UV_HANDLE_CLOSING;
    (*handle).flags &= !(UV_HANDLE_ACTIVE | UV_HANDLE_REF);
    (*handle).internal_owner = None;
  }
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
    (*handle).flags &= !(UV_HANDLE_ACTIVE | UV_HANDLE_REF);

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
      uv_handle_type::UV_NAMED_PIPE => {
        inner.stop_pipe(handle as *mut uv_pipe_t);
      }
      #[cfg(unix)]
      uv_handle_type::UV_POLL => {
        uv_poll_close(handle as *mut uv_poll_t);
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
