// Copyright 2018-2026 the Deno authors. MIT license.

// Drop-in replacement for libuv integrated with deno_core's event loop.

mod stream;
mod tcp;

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
        if unsafe { (*tcp_ptr).flags } & UV_HANDLE_ACTIVE == 0 {
          continue;
        }

        // SAFETY: tcp_ptr is valid; checked above.
        any_work |= unsafe { tcp::poll_tcp_handle(tcp_ptr, &mut cx) };
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
      tcp.internal_shutdown = None;
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
