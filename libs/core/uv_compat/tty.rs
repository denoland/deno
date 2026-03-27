// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::VecDeque;
use std::ffi::c_int;
use std::ffi::c_void;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::io::RawFd;
use std::task::Context;
#[cfg(windows)]
use std::task::Waker;

#[cfg(unix)]
use tokio::io::unix::AsyncFd;

use super::UV_EAGAIN;
#[cfg(windows)]
use super::UV_EBADF;
use super::UV_EINVAL;
use super::UV_ENOBUFS;
use super::UV_EOF;
use super::UV_EPIPE;
use super::UV_HANDLE_ACTIVE;
use super::UV_HANDLE_REF;
use super::get_inner;
use super::io_error_to_uv;
use super::tcp::ShutdownPending;
use super::tcp::WritePending;
use super::uv_alloc_cb;
use super::uv_buf_t;
use super::uv_handle_t;
use super::uv_handle_type;
use super::uv_loop_t;
use super::uv_read_cb;
use super::uv_shutdown_cb;
use super::uv_shutdown_t;
use super::uv_stream_t;
use super::uv_write_cb;
use super::uv_write_t;

// ---- Enums ----

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum uv_tty_mode_t {
  UV_TTY_MODE_NORMAL = 0,
  UV_TTY_MODE_RAW = 1,
  UV_TTY_MODE_IO = 2,
  UV_TTY_MODE_RAW_VT = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum uv_tty_vtermstate_t {
  UV_TTY_SUPPORTED = 0,
  UV_TTY_UNSUPPORTED = 1,
}

// ---- AsyncFd wrapper (Unix) ----

#[cfg(unix)]
pub(crate) struct TtyFd(RawFd);

#[cfg(unix)]
impl AsRawFd for TtyFd {
  fn as_raw_fd(&self) -> RawFd {
    self.0
  }
}

// ---- Reactor integration (Unix) ----
//
// On most platforms, we register the TTY fd directly with tokio's
// AsyncFd (epoll/kqueue). On macOS, kqueue doesn't work with some
// /dev devices (e.g. /dev/tty). libuv handles this with
// `uv__stream_try_select`: a background thread polls with select(2)
// and signals readiness through a socketpair whose read end IS
// kqueue-compatible. We do the same.
//
// TODO: When we add pipe support, refactor the reactor types and
// select fallback infrastructure (StreamReactor, SelectFallbackState,
// setup_select_fallback, select_thread_main, drain_fd, etc.) into
// stream.rs as a shared `stream_open()` function — mirroring libuv's
// `uv__stream_try_select` which is called from both tty.c and pipe.c.

#[cfg(unix)]
pub(crate) enum TtyReactor {
  /// Normal path: the TTY fd is registered directly with tokio.
  Normal(AsyncFd<TtyFd>),
  /// macOS fallback: a background thread polls with select(2) and
  /// signals readiness through a kqueue-compatible socketpair.
  #[cfg(target_os = "macos")]
  SelectFallback(SelectFallbackState),
}

#[cfg(unix)]
impl TtyReactor {
  /// Get a reference to the AsyncFd used for polling readiness.
  /// In the normal case this wraps the TTY fd directly; in the
  /// select fallback case it wraps the socketpair's read end.
  fn async_fd(&self) -> &AsyncFd<TtyFd> {
    match self {
      TtyReactor::Normal(afd) => afd,
      #[cfg(target_os = "macos")]
      TtyReactor::SelectFallback(s) => {
        s.async_fd.as_ref().expect("async_fd taken during shutdown")
      }
    }
  }
}

#[cfg(target_os = "macos")]
pub(crate) const SELECT_INTEREST_READ: u8 = 1;
#[cfg(target_os = "macos")]
pub(crate) const SELECT_INTEREST_WRITE: u8 = 2;

/// Socketpair communication:
///
/// ```text
///   fake_fd (fds[0]) <──────────> int_fd (fds[1])
///
///   Select thread writes to int_fd  → main thread reads fake_fd (readiness signal)
///   Main thread writes to fake_fd   → select thread reads int_fd (interrupt/shutdown)
/// ```
#[cfg(target_os = "macos")]
pub(crate) struct SelectFallbackState {
  /// AsyncFd wrapping the "fake" end of the socketpair. Readiness on
  /// this fd means the real TTY fd is ready. Wrapped in Option so
  /// `shutdown_select_fallback` can take it to deregister from kqueue
  /// before closing the raw fd (avoiding fd-reuse races).
  pub(crate) async_fd: Option<AsyncFd<TtyFd>>,
  /// The "fake" end of the socketpair (fds[0]). The main thread reads
  /// this (via AsyncFd) to receive readiness signals, and writes to it
  /// to interrupt the select thread.
  pub(crate) fake_fd: RawFd,
  /// The "interrupt" end of the socketpair (fds[1]). The select thread
  /// reads this for interrupt/shutdown signals, and writes to it to
  /// signal readiness to the main thread.
  pub(crate) int_fd: RawFd,
  /// Join handle for the background select thread.
  pub(crate) thread: Option<std::thread::JoinHandle<()>>,
  /// Signalled to tell the select thread to shut down.
  /// Matches libuv's `close_sem`.
  pub(crate) close: std::sync::Arc<std::sync::atomic::AtomicBool>,
  /// Current IO interest flags (SELECT_INTEREST_READ/WRITE).
  /// Updated by the main thread, read by the select thread.
  /// Matches libuv's `uv__io_active` checks in the select thread.
  pub(crate) interest: std::sync::Arc<std::sync::atomic::AtomicU8>,
  /// Semaphore for back-pressure: the select thread waits on this
  /// after signaling readiness, and the main thread posts it after
  /// processing events. Matches libuv's `async_sem`.
  pub(crate) async_sem: std::sync::Arc<Semaphore>,
}

/// A simple counting semaphore matching libuv's `uv_sem_t` usage.
/// The select thread calls `wait()` after signaling events; the main
/// thread calls `post()` after processing them.
#[cfg(target_os = "macos")]
pub(crate) struct Semaphore {
  mutex: std::sync::Mutex<u32>,
  cond: std::sync::Condvar,
}

#[cfg(target_os = "macos")]
impl Semaphore {
  pub(crate) fn new(initial: u32) -> Self {
    Self {
      mutex: std::sync::Mutex::new(initial),
      cond: std::sync::Condvar::new(),
    }
  }

  pub(crate) fn post(&self) {
    let mut count = self.mutex.lock().unwrap();
    *count += 1;
    self.cond.notify_one();
  }

  pub(crate) fn wait(&self) {
    let mut count = self.mutex.lock().unwrap();
    while *count == 0 {
      count = self.cond.wait(count).unwrap();
    }
    *count -= 1;
  }
}

// ---- The handle struct ----

#[repr(C)]
pub struct uv_tty_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,

  // TTY mode
  pub(crate) mode: uv_tty_mode_t,

  // Stream-like fields for read/write
  pub(crate) internal_alloc_cb: Option<uv_alloc_cb>,
  pub(crate) internal_read_cb: Option<uv_read_cb>,
  pub(crate) internal_reading: bool,
  pub(crate) internal_write_queue: VecDeque<WritePending>,
  pub(crate) internal_shutdown: Option<ShutdownPending>,

  // Unix-specific
  #[cfg(unix)]
  pub(crate) internal_fd: RawFd,
  #[cfg(unix)]
  pub(crate) internal_reactor: Option<TtyReactor>,
  #[cfg(unix)]
  pub(crate) internal_orig_termios: Option<libc::termios>,

  // Windows-specific
  #[cfg(windows)]
  pub(crate) internal_handle: *mut c_void, // HANDLE
  #[cfg(windows)]
  pub(crate) internal_readable: bool,
  #[cfg(windows)]
  pub(crate) internal_saved_mode: u32,
  #[cfg(windows)]
  pub(crate) internal_handle_owned: bool, // true if we duplicated (fd <= 2)
  #[cfg(windows)]
  pub(crate) internal_fd: c_int, // original CRT fd

  // Raw mode read state (mirrors libuv's tty.rd fields)
  #[cfg(windows)]
  pub(crate) internal_last_key: [u8; 32],
  #[cfg(windows)]
  pub(crate) internal_last_key_len: u8,
  #[cfg(windows)]
  pub(crate) internal_last_key_offset: u8,
  #[cfg(windows)]
  pub(crate) internal_last_repeat_count: u16,
  #[cfg(windows)]
  pub(crate) internal_utf16_high_surrogate: u16,

  // RegisterWaitForSingleObject state for async console input
  // notification (matching libuv's approach). The wait callback runs
  // on a thread pool thread and wakes the tokio event loop.
  #[cfg(windows)]
  pub(crate) internal_wait_handle: *mut c_void, // HANDLE from RegisterWait
  #[cfg(windows)]
  pub(crate) internal_wait_waker: Option<Box<std::sync::Mutex<Option<Waker>>>>,

  // Line-mode threaded read state. ReadConsoleW blocks until Enter,
  // so it runs on a worker thread (matching libuv's
  // uv_tty_line_read_thread). The result is collected on the next
  // poll_tty_handle call.
  #[cfg(windows)]
  pub(crate) internal_line_read_result: Option<LineReadResult>,
  #[cfg(windows)]
  pub(crate) internal_line_read_pending: bool,
}

#[cfg(windows)]
type LineReadResult =
  std::sync::Arc<std::sync::Mutex<Option<Result<Vec<u8>, i32>>>>;

pub type UvTty = uv_tty_t;

// ---- Global termios state for uv_tty_reset_mode (Unix) ----
//
// Uses an atomic spinlock (not a mutex) so that uv_tty_reset_mode can be
// called from a signal handler, matching libuv's async-signal-safe design.

#[cfg(unix)]
mod global_termios {
  use std::cell::UnsafeCell;
  use std::ffi::c_int;
  use std::mem::MaybeUninit;
  use std::os::unix::io::RawFd;
  use std::sync::atomic::AtomicI32;
  use std::sync::atomic::Ordering;

  // Wrapper to allow UnsafeCell in a static.
  struct SyncTermios(UnsafeCell<MaybeUninit<libc::termios>>);
  // SAFETY: Access is guarded by SPINLOCK.
  unsafe impl Sync for SyncTermios {}

  static ORIG_TERMIOS_FD: AtomicI32 = AtomicI32::new(-1);
  static SPINLOCK: AtomicI32 = AtomicI32::new(0);
  static ORIG_TERMIOS: SyncTermios =
    SyncTermios(UnsafeCell::new(MaybeUninit::uninit()));

  pub(super) fn acquire() {
    loop {
      if SPINLOCK
        .compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
      {
        return;
      }
      std::hint::spin_loop();
    }
  }

  pub(super) fn try_acquire() -> bool {
    SPINLOCK
      .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
      .is_ok()
  }

  pub(super) fn release() {
    SPINLOCK.store(0, Ordering::Release);
  }

  /// Save the original termios if this is the first TTY to enter
  /// non-normal mode. Must be called while spinlock is held.
  pub(super) fn save_if_first(fd: RawFd, termios: &libc::termios) {
    if ORIG_TERMIOS_FD.load(Ordering::Relaxed) == -1 {
      // SAFETY: Spinlock is held, so we have exclusive access.
      unsafe {
        (*ORIG_TERMIOS.0.get()).write(*termios);
      }
      ORIG_TERMIOS_FD.store(fd, Ordering::Relaxed);
    }
  }

  /// Restore original termios and clear global state if `fd` matches.
  /// Must be called while spinlock is held.
  pub(super) fn restore_and_clear(fd: RawFd) {
    if ORIG_TERMIOS_FD.load(Ordering::Relaxed) == fd {
      // SAFETY: Spinlock is held and fd matches, so ORIG_TERMIOS is init.
      let termios = unsafe { (*ORIG_TERMIOS.0.get()).assume_init_ref() };
      // Retry on EINTR, matching libuv's uv__tcsetattr.
      unsafe {
        loop {
          if libc::tcsetattr(fd, libc::TCSANOW, termios) == 0 {
            break;
          }
          if *errno_location() != libc::EINTR {
            break;
          }
        }
      }
      ORIG_TERMIOS_FD.store(-1, Ordering::Relaxed);
    }
  }

  /// Reset the terminal to its original mode. Async-signal-safe.
  /// Preserves `errno` to match libuv's signal-handler-safe contract.
  pub(super) fn reset() -> c_int {
    // Save errno — this function may be called from a signal handler.
    // We avoid std::io::Error here to stay async-signal-safe.
    let errno_ptr = errno_location();
    let saved_errno = unsafe { *errno_ptr };

    if !try_acquire() {
      unsafe { *errno_ptr = saved_errno };
      return super::super::UV_EBUSY;
    }

    let fd = ORIG_TERMIOS_FD.load(Ordering::Relaxed);
    let err = if fd != -1 {
      // SAFETY: Spinlock is held and fd != -1, so ORIG_TERMIOS is init.
      let termios = unsafe { (*ORIG_TERMIOS.0.get()).assume_init_ref() };
      // Retry on EINTR, matching libuv's uv__tcsetattr.
      loop {
        let rc = unsafe { libc::tcsetattr(fd, libc::TCSANOW, termios) };
        if rc == 0 {
          break 0;
        }
        let e = unsafe { *errno_ptr };
        if e != libc::EINTR {
          break -e;
        }
      }
    } else {
      0
    };

    release();
    unsafe { *errno_ptr = saved_errno };
    err
  }

  /// Get a pointer to the thread-local errno value.
  #[cfg(target_os = "macos")]
  fn errno_location() -> *mut c_int {
    unsafe extern "C" {
      fn __error() -> *mut c_int;
    }
    unsafe { __error() }
  }

  #[cfg(target_os = "linux")]
  fn errno_location() -> *mut c_int {
    unsafe extern "C" {
      fn __errno_location() -> *mut c_int;
    }
    unsafe { __errno_location() }
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  fn errno_location() -> *mut c_int {
    compile_error!(
      "errno_location not implemented for this platform — \
       add the appropriate extern function (e.g. __error on BSDs)"
    );
  }
}

// ---- Windows console FFI ----

#[cfg(windows)]
pub(crate) mod win_console {
  #![allow(
    non_snake_case,
    non_camel_case_types,
    clippy::upper_case_acronyms,
    dead_code,
    reason = "ffi"
  )]

  use std::ffi::c_int;
  use std::ffi::c_void;

  pub type HANDLE = *mut c_void;
  pub type DWORD = u32;
  pub type BOOL = i32;

  pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
  pub const DUPLICATE_SAME_ACCESS: DWORD = 0x00000002;

  pub const ENABLE_ECHO_INPUT: DWORD = 0x0004;
  pub const ENABLE_LINE_INPUT: DWORD = 0x0002;
  pub const ENABLE_PROCESSED_INPUT: DWORD = 0x0001;
  pub const ENABLE_WINDOW_INPUT: DWORD = 0x0008;
  pub const ENABLE_VIRTUAL_TERMINAL_INPUT: DWORD = 0x0200;
  pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;
  pub const ENABLE_PROCESSED_OUTPUT: DWORD = 0x0001;

  pub const FILE_TYPE_CHAR: DWORD = 0x0002;
  pub const FILE_TYPE_DISK: DWORD = 0x0001;
  pub const FILE_TYPE_PIPE: DWORD = 0x0003;

  #[repr(C)]
  pub struct COORD {
    pub X: i16,
    pub Y: i16,
  }

  #[repr(C)]
  pub struct SMALL_RECT {
    pub Left: i16,
    pub Top: i16,
    pub Right: i16,
    pub Bottom: i16,
  }

  #[repr(C)]
  pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dwSize: COORD,
    pub dwCursorPosition: COORD,
    pub wAttributes: u16,
    pub srWindow: SMALL_RECT,
    pub dwMaximumWindowSize: COORD,
  }

  // Console input event types
  pub const KEY_EVENT: u16 = 0x0001;
  pub const WINDOW_BUFFER_SIZE_EVENT: u16 = 0x0004;

  // Virtual key codes
  pub const VK_BACK: u16 = 0x08;
  pub const VK_TAB: u16 = 0x09;
  pub const VK_RETURN: u16 = 0x0D;
  pub const VK_ESCAPE: u16 = 0x1B;
  pub const VK_PRIOR: u16 = 0x21; // Page Up
  pub const VK_NEXT: u16 = 0x22; // Page Down
  pub const VK_END: u16 = 0x23;
  pub const VK_HOME: u16 = 0x24;
  pub const VK_LEFT: u16 = 0x25;
  pub const VK_UP: u16 = 0x26;
  pub const VK_RIGHT: u16 = 0x27;
  pub const VK_DOWN: u16 = 0x28;
  pub const VK_INSERT: u16 = 0x2D;
  pub const VK_DELETE: u16 = 0x2E;
  pub const VK_NUMPAD0: u16 = 0x60;
  pub const VK_NUMPAD1: u16 = 0x61;
  pub const VK_NUMPAD2: u16 = 0x62;
  pub const VK_NUMPAD3: u16 = 0x63;
  pub const VK_NUMPAD4: u16 = 0x64;
  pub const VK_NUMPAD5: u16 = 0x65;
  pub const VK_NUMPAD6: u16 = 0x66;
  pub const VK_NUMPAD7: u16 = 0x67;
  pub const VK_NUMPAD8: u16 = 0x68;
  pub const VK_NUMPAD9: u16 = 0x69;
  pub const VK_DECIMAL: u16 = 0x6E;
  pub const VK_F1: u16 = 0x70;
  pub const VK_F2: u16 = 0x71;
  pub const VK_F3: u16 = 0x72;
  pub const VK_F4: u16 = 0x73;
  pub const VK_F5: u16 = 0x74;
  pub const VK_F6: u16 = 0x75;
  pub const VK_F7: u16 = 0x76;
  pub const VK_F8: u16 = 0x77;
  pub const VK_F9: u16 = 0x78;
  pub const VK_F10: u16 = 0x79;
  pub const VK_F11: u16 = 0x7A;
  pub const VK_F12: u16 = 0x7B;
  pub const VK_MENU: u16 = 0x12; // Alt key
  pub const VK_CLEAR: u16 = 0x0C;

  // Control key state flags
  pub const SHIFT_PRESSED: u32 = 0x0010;
  pub const LEFT_ALT_PRESSED: u32 = 0x0002;
  pub const RIGHT_ALT_PRESSED: u32 = 0x0001;
  pub const LEFT_CTRL_PRESSED: u32 = 0x0008;
  pub const RIGHT_CTRL_PRESSED: u32 = 0x0004;
  pub const ENHANCED_KEY: u32 = 0x0100;

  #[repr(C)]
  pub struct KEY_EVENT_RECORD {
    pub bKeyDown: BOOL,
    pub wRepeatCount: u16,
    pub wVirtualKeyCode: u16,
    pub wVirtualScanCode: u16,
    pub uChar: u16, // UChar union - we only need the UnicodeChar member
    pub dwControlKeyState: DWORD,
  }

  // INPUT_RECORD is a tagged union. We represent it with the largest
  // variant (KEY_EVENT_RECORD) and interpret based on EventType.
  #[repr(C)]
  pub struct INPUT_RECORD {
    pub EventType: u16,
    pub Event: KEY_EVENT_RECORD,
  }

  unsafe extern "system" {
    pub fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> BOOL;
    pub fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    pub fn GetConsoleScreenBufferInfo(
      hConsoleHandle: HANDLE,
      lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> BOOL;
    pub fn GetNumberOfConsoleInputEvents(
      hConsoleHandle: HANDLE,
      lpcNumberOfEvents: *mut DWORD,
    ) -> BOOL;
    pub fn PeekConsoleInputW(
      hConsoleInput: HANDLE,
      lpBuffer: *mut INPUT_RECORD,
      nLength: DWORD,
      lpNumberOfEventsRead: *mut DWORD,
    ) -> BOOL;
    pub fn ReadConsoleInputW(
      hConsoleInput: HANDLE,
      lpBuffer: *mut INPUT_RECORD,
      nLength: DWORD,
      lpNumberOfEventsRead: *mut DWORD,
    ) -> BOOL;
    pub fn ReadConsoleW(
      hConsoleInput: HANDLE,
      lpBuffer: *mut u16,
      nNumberOfCharsToRead: DWORD,
      lpNumberOfCharsRead: *mut DWORD,
      pInputControl: *mut c_void,
    ) -> BOOL;
    pub fn GetLastError() -> DWORD;
    pub fn WriteFile(
      hFile: HANDLE,
      lpBuffer: *const u8,
      nNumberOfBytesToWrite: DWORD,
      lpNumberOfBytesWritten: *mut DWORD,
      lpOverlapped: *mut c_void,
    ) -> BOOL;
    pub fn ReadFile(
      hFile: HANDLE,
      lpBuffer: *mut u8,
      nNumberOfBytesToRead: DWORD,
      lpNumberOfBytesRead: *mut DWORD,
      lpOverlapped: *mut c_void,
    ) -> BOOL;
    pub fn WriteConsoleW(
      hConsoleOutput: HANDLE,
      lpBuffer: *const u16,
      nNumberOfCharsToWrite: DWORD,
      lpNumberOfCharsWritten: *mut DWORD,
      lpReserved: *mut c_void,
    ) -> BOOL;
    pub fn GetFileType(hFile: HANDLE) -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn DuplicateHandle(
      hSourceProcessHandle: HANDLE,
      hSourceHandle: HANDLE,
      hTargetProcessHandle: HANDLE,
      lpTargetHandle: *mut HANDLE,
      dwDesiredAccess: DWORD,
      bInheritHandle: BOOL,
      dwOptions: DWORD,
    ) -> BOOL;
    pub fn RegisterWaitForSingleObject(
      phNewWaitObject: *mut HANDLE,
      hObject: HANDLE,
      Callback: unsafe extern "system" fn(*mut c_void, u8),
      Context: *mut c_void,
      dwMilliseconds: DWORD,
      dwFlags: DWORD,
    ) -> BOOL;
    pub fn UnregisterWaitEx(
      WaitHandle: HANDLE,
      CompletionEvent: HANDLE,
    ) -> BOOL;
  }

  pub const INFINITE: DWORD = 0xFFFFFFFF;
  pub const WT_EXECUTEINWAITTHREAD: DWORD = 0x00000004;
  pub const WT_EXECUTEONLYONCE: DWORD = 0x00000008;
  pub const INVALID_HANDLE_VALUE_ISIZE: isize = -1;

  pub const GENERIC_READ: DWORD = 0x80000000;
  pub const GENERIC_WRITE: DWORD = 0x40000000;
  pub const FILE_SHARE_READ: DWORD = 0x00000001;
  pub const FILE_SHARE_WRITE: DWORD = 0x00000002;
  pub const OPEN_EXISTING: DWORD = 3;

  unsafe extern "system" {
    pub fn CreateFileW(
      lpFileName: *const u16,
      dwDesiredAccess: DWORD,
      dwShareMode: DWORD,
      lpSecurityAttributes: *mut c_void,
      dwCreationDisposition: DWORD,
      dwFlagsAndAttributes: DWORD,
      hTemplateFile: HANDLE,
    ) -> HANDLE;
  }

  unsafe extern "C" {
    pub fn _get_osfhandle(fd: c_int) -> isize;
    pub fn _close(fd: c_int) -> c_int;
  }

  // Invalid parameter handler type matching MSVC CRT.
  type InvalidParameterHandler = Option<
    unsafe extern "C" fn(*const u16, *const u16, *const u16, u32, usize),
  >;

  unsafe extern "C" {
    fn _set_thread_local_invalid_parameter_handler(
      handler: InvalidParameterHandler,
    ) -> InvalidParameterHandler;
  }

  // No-op handler that prevents CRT from aborting on invalid parameters.
  unsafe extern "C" fn noop_invalid_parameter_handler(
    _expression: *const u16,
    _function: *const u16,
    _file: *const u16,
    _line: u32,
    _reserved: usize,
  ) {
  }

  /// Call `_get_osfhandle` without crashing on invalid fds.
  /// Returns -1 (INVALID_HANDLE_VALUE) for invalid fds.
  pub unsafe fn safe_get_osfhandle(fd: c_int) -> isize {
    unsafe {
      let prev = _set_thread_local_invalid_parameter_handler(Some(
        noop_invalid_parameter_handler,
      ));
      let handle = _get_osfhandle(fd);
      _set_thread_local_invalid_parameter_handler(prev);
      handle
    }
  }
}

// ---- Global console state for uv_tty_reset_mode (Windows) ----

#[cfg(windows)]
mod global_console {
  use std::ffi::c_void;
  use std::sync::Once;
  use std::sync::atomic::AtomicU32;
  use std::sync::atomic::Ordering;

  use super::win_console;

  static INIT: Once = Once::new();
  static ORIG_MODE: AtomicU32 = AtomicU32::new(0);
  // The CONIN$ handle, stored as atomic usize. This is opened once and
  // never closed, matching libuv's `uv__tty_console_handle_in`.
  static CONIN_HANDLE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
  // Whether mode was ever changed and needs reset.
  static NEEDS_RESET: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

  /// Open CONIN$ and save the original console input mode. Called once
  /// during the first `uv_tty_set_mode` on a readable handle.
  pub(super) fn save_if_first() {
    INIT.call_once(|| {
      // "CONIN$\0" as UTF-16
      let conin: [u16; 7] = [
        b'C' as u16,
        b'O' as u16,
        b'N' as u16,
        b'I' as u16,
        b'N' as u16,
        b'$' as u16,
        0,
      ];
      let handle = unsafe {
        win_console::CreateFileW(
          conin.as_ptr(),
          win_console::GENERIC_READ | win_console::GENERIC_WRITE,
          win_console::FILE_SHARE_READ,
          std::ptr::null_mut(),
          win_console::OPEN_EXISTING,
          0,
          std::ptr::null_mut(),
        )
      };
      if handle == win_console::INVALID_HANDLE_VALUE {
        return;
      }
      let mut mode: u32 = 0;
      if unsafe { win_console::GetConsoleMode(handle, &mut mode) } != 0 {
        ORIG_MODE.store(mode, Ordering::Relaxed);
        CONIN_HANDLE.store(handle as usize, Ordering::Relaxed);
      }
    });
  }

  /// Mark the console as needing a reset. Called only for RAW/RAW_VT modes,
  /// matching libuv's behavior where NORMAL mode does not set the flag.
  pub(super) fn mark_needs_reset() {
    NEEDS_RESET.store(true, Ordering::Relaxed);
  }

  /// Restore the original console input mode.
  pub(super) fn reset() -> i32 {
    if !NEEDS_RESET.load(Ordering::Relaxed) {
      return 0;
    }
    let handle = CONIN_HANDLE.load(Ordering::Relaxed);
    if handle == 0 {
      return 0;
    }
    let mode = ORIG_MODE.load(Ordering::Relaxed);
    if unsafe { win_console::SetConsoleMode(handle as *mut c_void, mode) } == 0
    {
      return super::super::UV_EINVAL;
    }
    0
  }
}

// ---- Platform-specific I/O helpers ----

#[cfg(unix)]
unsafe fn tty_try_write(
  tty: *const uv_tty_t,
  data: &[u8],
) -> std::io::Result<usize> {
  let fd = unsafe { (*tty).internal_fd };
  let ret =
    unsafe { libc::write(fd, data.as_ptr() as *const c_void, data.len()) };
  if ret < 0 {
    Err(std::io::Error::last_os_error())
  } else {
    Ok(ret as usize)
  }
}

#[cfg(unix)]
unsafe fn tty_try_read(
  tty: *const uv_tty_t,
  buf: &mut [u8],
) -> std::io::Result<usize> {
  let fd = unsafe { (*tty).internal_fd };
  let ret =
    unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, buf.len()) };
  if ret < 0 {
    Err(std::io::Error::last_os_error())
  } else {
    Ok(ret as usize)
  }
}

#[cfg(windows)]
unsafe fn tty_try_write(
  tty: *const uv_tty_t,
  data: &[u8],
) -> std::io::Result<usize> {
  let handle = unsafe { (*tty).internal_handle };

  // Use WriteConsoleW for TTY output to correctly handle UTF-8
  // regardless of the console's active code page. This matches libuv
  // which converts to UTF-16 and calls WriteConsoleW.
  let utf16: Vec<u16> = match std::str::from_utf8(data) {
    Ok(s) => s.encode_utf16().collect(),
    Err(_) => {
      // Not valid UTF-8 — fall back to WriteFile (raw bytes).
      let mut written: u32 = 0;
      let ret = unsafe {
        win_console::WriteFile(
          handle,
          data.as_ptr(),
          data.len() as u32,
          &mut written,
          std::ptr::null_mut(),
        )
      };
      return if ret == 0 {
        Err(std::io::Error::last_os_error())
      } else {
        Ok(written as usize)
      };
    }
  };

  if utf16.is_empty() {
    return Ok(0);
  }

  let mut chars_written: u32 = 0;
  let ret = unsafe {
    win_console::WriteConsoleW(
      handle,
      utf16.as_ptr(),
      utf16.len() as u32,
      &mut chars_written,
      std::ptr::null_mut(),
    )
  };
  if ret == 0 {
    Err(std::io::Error::last_os_error())
  } else {
    // WriteConsoleW reports UTF-16 code units written. We need to
    // return the number of input bytes consumed. If everything was
    // written, that's all of `data`. For partial writes, count the
    // bytes of the UTF-8 that produced the written UTF-16 units.
    if chars_written as usize >= utf16.len() {
      Ok(data.len())
    } else {
      // Find byte offset corresponding to chars_written UTF-16 units.
      let s = std::str::from_utf8(data).unwrap();
      let mut utf16_count = 0usize;
      let mut byte_offset = 0usize;
      for ch in s.chars() {
        let ch_utf16_len = ch.len_utf16();
        if utf16_count + ch_utf16_len > chars_written as usize {
          break;
        }
        utf16_count += ch_utf16_len;
        byte_offset += ch.len_utf8();
      }
      Ok(byte_offset)
    }
  }
}

/// Map a virtual key code + modifiers to a VT100/xterm escape sequence.
/// Returns None for keys we don't handle (caller should skip them).
/// Matches libuv's get_vt100_fn_key (Cygwin-compatible mappings).
#[cfg(windows)]
fn get_vt100_fn_key(
  code: u16,
  shift: bool,
  ctrl: bool,
) -> Option<&'static [u8]> {
  use win_console::*;
  match (code, shift, ctrl) {
    // Arrow keys
    (VK_UP, false, false) => Some(b"\x1b[A"),
    (VK_UP, true, false) => Some(b"\x1b[1;2A"),
    (VK_UP, false, true) => Some(b"\x1b[1;5A"),
    (VK_UP, true, true) => Some(b"\x1b[1;6A"),
    (VK_DOWN, false, false) => Some(b"\x1b[B"),
    (VK_DOWN, true, false) => Some(b"\x1b[1;2B"),
    (VK_DOWN, false, true) => Some(b"\x1b[1;5B"),
    (VK_DOWN, true, true) => Some(b"\x1b[1;6B"),
    (VK_RIGHT, false, false) => Some(b"\x1b[C"),
    (VK_RIGHT, true, false) => Some(b"\x1b[1;2C"),
    (VK_RIGHT, false, true) => Some(b"\x1b[1;5C"),
    (VK_RIGHT, true, true) => Some(b"\x1b[1;6C"),
    (VK_LEFT, false, false) => Some(b"\x1b[D"),
    (VK_LEFT, true, false) => Some(b"\x1b[1;2D"),
    (VK_LEFT, false, true) => Some(b"\x1b[1;5D"),
    (VK_LEFT, true, true) => Some(b"\x1b[1;6D"),
    // Clear (numpad 5)
    (VK_CLEAR, false, false) => Some(b"\x1b[G"),
    (VK_CLEAR, true, false) => Some(b"\x1b[1;2G"),
    (VK_CLEAR, false, true) => Some(b"\x1b[1;5G"),
    (VK_CLEAR, true, true) => Some(b"\x1b[1;6G"),
    // Home / End
    (VK_HOME, false, false) => Some(b"\x1b[1~"),
    (VK_HOME, true, false) => Some(b"\x1b[1;2~"),
    (VK_HOME, false, true) => Some(b"\x1b[1;5~"),
    (VK_HOME, true, true) => Some(b"\x1b[1;6~"),
    (VK_END, false, false) => Some(b"\x1b[4~"),
    (VK_END, true, false) => Some(b"\x1b[4;2~"),
    (VK_END, false, true) => Some(b"\x1b[4;5~"),
    (VK_END, true, true) => Some(b"\x1b[4;6~"),
    // Insert / Delete
    (VK_INSERT, false, false) => Some(b"\x1b[2~"),
    (VK_INSERT, true, false) => Some(b"\x1b[2;2~"),
    (VK_INSERT, false, true) => Some(b"\x1b[2;5~"),
    (VK_INSERT, true, true) => Some(b"\x1b[2;6~"),
    (VK_DELETE, false, false) => Some(b"\x1b[3~"),
    (VK_DELETE, true, false) => Some(b"\x1b[3;2~"),
    (VK_DELETE, false, true) => Some(b"\x1b[3;5~"),
    (VK_DELETE, true, true) => Some(b"\x1b[3;6~"),
    // Page Up / Page Down
    (VK_PRIOR, false, false) => Some(b"\x1b[5~"),
    (VK_PRIOR, true, false) => Some(b"\x1b[5;2~"),
    (VK_PRIOR, false, true) => Some(b"\x1b[5;5~"),
    (VK_PRIOR, true, true) => Some(b"\x1b[5;6~"),
    (VK_NEXT, false, false) => Some(b"\x1b[6~"),
    (VK_NEXT, true, false) => Some(b"\x1b[6;2~"),
    (VK_NEXT, false, true) => Some(b"\x1b[6;5~"),
    (VK_NEXT, true, true) => Some(b"\x1b[6;6~"),
    // Numpad (same sequences as the corresponding navigation keys)
    (VK_NUMPAD0, false, false) => Some(b"\x1b[2~"),
    (VK_NUMPAD0, true, false) => Some(b"\x1b[2;2~"),
    (VK_NUMPAD0, false, true) => Some(b"\x1b[2;5~"),
    (VK_NUMPAD0, true, true) => Some(b"\x1b[2;6~"),
    (VK_NUMPAD1, false, false) => Some(b"\x1b[4~"),
    (VK_NUMPAD1, true, false) => Some(b"\x1b[4;2~"),
    (VK_NUMPAD1, false, true) => Some(b"\x1b[4;5~"),
    (VK_NUMPAD1, true, true) => Some(b"\x1b[4;6~"),
    (VK_NUMPAD2, false, false) => Some(b"\x1b[B"),
    (VK_NUMPAD2, true, false) => Some(b"\x1b[1;2B"),
    (VK_NUMPAD2, false, true) => Some(b"\x1b[1;5B"),
    (VK_NUMPAD2, true, true) => Some(b"\x1b[1;6B"),
    (VK_NUMPAD3, false, false) => Some(b"\x1b[6~"),
    (VK_NUMPAD3, true, false) => Some(b"\x1b[6;2~"),
    (VK_NUMPAD3, false, true) => Some(b"\x1b[6;5~"),
    (VK_NUMPAD3, true, true) => Some(b"\x1b[6;6~"),
    (VK_NUMPAD4, false, false) => Some(b"\x1b[D"),
    (VK_NUMPAD4, true, false) => Some(b"\x1b[1;2D"),
    (VK_NUMPAD4, false, true) => Some(b"\x1b[1;5D"),
    (VK_NUMPAD4, true, true) => Some(b"\x1b[1;6D"),
    (VK_NUMPAD5, false, false) => Some(b"\x1b[G"),
    (VK_NUMPAD5, true, false) => Some(b"\x1b[1;2G"),
    (VK_NUMPAD5, false, true) => Some(b"\x1b[1;5G"),
    (VK_NUMPAD5, true, true) => Some(b"\x1b[1;6G"),
    (VK_NUMPAD6, false, false) => Some(b"\x1b[C"),
    (VK_NUMPAD6, true, false) => Some(b"\x1b[1;2C"),
    (VK_NUMPAD6, false, true) => Some(b"\x1b[1;5C"),
    (VK_NUMPAD6, true, true) => Some(b"\x1b[1;6C"),
    (VK_NUMPAD7, false, false) => Some(b"\x1b[A"),
    (VK_NUMPAD7, true, false) => Some(b"\x1b[1;2A"),
    (VK_NUMPAD7, false, true) => Some(b"\x1b[1;5A"),
    (VK_NUMPAD7, true, true) => Some(b"\x1b[1;6A"),
    (VK_NUMPAD8, false, false) => Some(b"\x1b[1~"),
    (VK_NUMPAD8, true, false) => Some(b"\x1b[1;2~"),
    (VK_NUMPAD8, false, true) => Some(b"\x1b[1;5~"),
    (VK_NUMPAD8, true, true) => Some(b"\x1b[1;6~"),
    (VK_NUMPAD9, false, false) => Some(b"\x1b[5~"),
    (VK_NUMPAD9, true, false) => Some(b"\x1b[5;2~"),
    (VK_NUMPAD9, false, true) => Some(b"\x1b[5;5~"),
    (VK_NUMPAD9, true, true) => Some(b"\x1b[5;6~"),
    (VK_DECIMAL, false, false) => Some(b"\x1b[3~"),
    (VK_DECIMAL, true, false) => Some(b"\x1b[3;2~"),
    (VK_DECIMAL, false, true) => Some(b"\x1b[3;5~"),
    (VK_DECIMAL, true, true) => Some(b"\x1b[3;6~"),
    // Function keys
    (VK_F1, false, false) => Some(b"\x1b[[A"),
    (VK_F1, true, false) => Some(b"\x1b[23~"),
    (VK_F1, false, true) => Some(b"\x1b[11^"),
    (VK_F1, true, true) => Some(b"\x1b[23^"),
    (VK_F2, false, false) => Some(b"\x1b[[B"),
    (VK_F2, true, false) => Some(b"\x1b[24~"),
    (VK_F2, false, true) => Some(b"\x1b[12^"),
    (VK_F2, true, true) => Some(b"\x1b[24^"),
    (VK_F3, false, false) => Some(b"\x1b[[C"),
    (VK_F3, true, false) => Some(b"\x1b[25~"),
    (VK_F3, false, true) => Some(b"\x1b[13^"),
    (VK_F3, true, true) => Some(b"\x1b[25^"),
    (VK_F4, false, false) => Some(b"\x1b[[D"),
    (VK_F4, true, false) => Some(b"\x1b[26~"),
    (VK_F4, false, true) => Some(b"\x1b[14^"),
    (VK_F4, true, true) => Some(b"\x1b[26^"),
    (VK_F5, false, false) => Some(b"\x1b[[E"),
    (VK_F5, true, false) => Some(b"\x1b[28~"),
    (VK_F5, false, true) => Some(b"\x1b[15^"),
    (VK_F5, true, true) => Some(b"\x1b[28^"),
    (VK_F6, false, false) => Some(b"\x1b[17~"),
    (VK_F6, true, false) => Some(b"\x1b[29~"),
    (VK_F6, false, true) => Some(b"\x1b[17^"),
    (VK_F6, true, true) => Some(b"\x1b[29^"),
    (VK_F7, false, false) => Some(b"\x1b[18~"),
    (VK_F7, true, false) => Some(b"\x1b[31~"),
    (VK_F7, false, true) => Some(b"\x1b[18^"),
    (VK_F7, true, true) => Some(b"\x1b[31^"),
    (VK_F8, false, false) => Some(b"\x1b[19~"),
    (VK_F8, true, false) => Some(b"\x1b[32~"),
    (VK_F8, false, true) => Some(b"\x1b[19^"),
    (VK_F8, true, true) => Some(b"\x1b[32^"),
    (VK_F9, false, false) => Some(b"\x1b[20~"),
    (VK_F9, true, false) => Some(b"\x1b[33~"),
    (VK_F9, false, true) => Some(b"\x1b[20^"),
    (VK_F9, true, true) => Some(b"\x1b[33^"),
    (VK_F10, false, false) => Some(b"\x1b[21~"),
    (VK_F10, true, false) => Some(b"\x1b[34~"),
    (VK_F10, false, true) => Some(b"\x1b[21^"),
    (VK_F10, true, true) => Some(b"\x1b[34^"),
    (VK_F11, false, false) => Some(b"\x1b[23~"),
    (VK_F11, true, false) => Some(b"\x1b[23$"),
    (VK_F11, false, true) => Some(b"\x1b[23^"),
    (VK_F11, true, true) => Some(b"\x1b[23@"),
    (VK_F12, false, false) => Some(b"\x1b[24~"),
    (VK_F12, true, false) => Some(b"\x1b[24$"),
    (VK_F12, false, true) => Some(b"\x1b[24^"),
    (VK_F12, true, true) => Some(b"\x1b[24@"),
    _ => None,
  }
}

/// Returns true if the mode is a raw mode (RAW or RAW_VT).
#[cfg(windows)]
fn is_raw_tty_mode(mode: uv_tty_mode_t) -> bool {
  matches!(
    mode,
    uv_tty_mode_t::UV_TTY_MODE_RAW | uv_tty_mode_t::UV_TTY_MODE_RAW_VT
  )
}

/// Raw-mode read for Windows: uses ReadConsoleInputW to process individual
/// INPUT_RECORD structs. This avoids ReadFile which blocks on non-character
/// events (KEY_UP, FOCUS, MOUSE, etc.). Matches libuv's
/// uv_process_tty_read_raw_req approach.
///
/// Fills `buf` with decoded UTF-8 bytes and returns the number of bytes
/// written. Returns WouldBlock if no character-producing events are
/// available.
#[cfg(windows)]
unsafe fn tty_try_read_raw(
  tty: *mut uv_tty_t,
  buf: &mut [u8],
) -> std::io::Result<usize> {
  let handle = unsafe { (*tty).internal_handle };
  let mut buf_used: usize = 0;

  loop {
    // Phase 1: Drain any pending bytes from the last decoded key.
    unsafe {
      while (*tty).internal_last_key_offset < (*tty).internal_last_key_len {
        if buf_used >= buf.len() {
          return Ok(buf_used);
        }
        buf[buf_used] =
          (*tty).internal_last_key[(*tty).internal_last_key_offset as usize];
        (*tty).internal_last_key_offset += 1;
        buf_used += 1;
      }

      // Phase 2: Handle repeat count from previous key.
      if (*tty).internal_last_key_len > 0 {
        if (*tty).internal_last_repeat_count > 0 {
          (*tty).internal_last_repeat_count -= 1;
          (*tty).internal_last_key_offset = 0;
          continue;
        }
        (*tty).internal_last_key_len = 0;
      }
    }

    // Phase 3: Check if there are more input records to process.
    let mut num_events: u32 = 0;
    if unsafe {
      win_console::GetNumberOfConsoleInputEvents(handle, &mut num_events)
    } == 0
      || num_events == 0
    {
      break;
    }

    // Phase 4: Read the next input record.
    let mut record: win_console::INPUT_RECORD = unsafe { std::mem::zeroed() };
    let mut records_read: u32 = 0;
    if unsafe {
      win_console::ReadConsoleInputW(handle, &mut record, 1, &mut records_read)
    } == 0
    {
      return Err(std::io::Error::last_os_error());
    }
    if records_read == 0 {
      break;
    }

    // Skip non-key events.
    if record.EventType != win_console::KEY_EVENT {
      // TODO: handle WINDOW_BUFFER_SIZE_EVENT for resize signaling
      continue;
    }

    let kev = &record.Event;

    // Skip KEY_UP events, unless the Alt key was released with a valid
    // Unicode character (Alt-code composition).
    if kev.bKeyDown == 0
      && (kev.wVirtualKeyCode != win_console::VK_MENU || kev.uChar == 0)
    {
      continue;
    }

    // Skip numpad keys during Alt-code composition (LEFT_ALT held,
    // no ENHANCED_KEY flag).
    if (kev.dwControlKeyState & win_console::LEFT_ALT_PRESSED) != 0
      && (kev.dwControlKeyState & win_console::ENHANCED_KEY) == 0
      && matches!(
        kev.wVirtualKeyCode,
        win_console::VK_INSERT
          | win_console::VK_END
          | win_console::VK_DOWN
          | win_console::VK_NEXT
          | win_console::VK_LEFT
          | win_console::VK_CLEAR
          | win_console::VK_RIGHT
          | win_console::VK_HOME
          | win_console::VK_UP
          | win_console::VK_PRIOR
          | win_console::VK_NUMPAD0
          | win_console::VK_NUMPAD1
          | win_console::VK_NUMPAD2
          | win_console::VK_NUMPAD3
          | win_console::VK_NUMPAD4
          | win_console::VK_NUMPAD5
          | win_console::VK_NUMPAD6
          | win_console::VK_NUMPAD7
          | win_console::VK_NUMPAD8
          | win_console::VK_NUMPAD9
      )
    {
      continue;
    }

    if kev.uChar != 0 {
      // Character key pressed.
      let unicode_char = kev.uChar;

      // Handle UTF-16 high surrogates.
      if (0xD800..0xDC00).contains(&unicode_char) {
        unsafe {
          (*tty).internal_utf16_high_surrogate = unicode_char;
        }
        continue;
      }

      // Determine Alt prefix: ESC before character if Alt held
      // without Ctrl (Ctrl+Alt = AltGr on some layouts).
      let alt_held = (kev.dwControlKeyState
        & (win_console::LEFT_ALT_PRESSED | win_console::RIGHT_ALT_PRESSED))
        != 0;
      let ctrl_held = (kev.dwControlKeyState
        & (win_console::LEFT_CTRL_PRESSED | win_console::RIGHT_CTRL_PRESSED))
        != 0;
      let prefix_len = if alt_held && !ctrl_held && kev.bKeyDown != 0 {
        1usize
      } else {
        0usize
      };

      // Encode UTF-16 to UTF-8.
      let high_surrogate = unsafe { (*tty).internal_utf16_high_surrogate };
      let decoded = if high_surrogate != 0 {
        unsafe {
          (*tty).internal_utf16_high_surrogate = 0;
        }
        char::decode_utf16([high_surrogate, unicode_char].iter().copied())
          .next()
      } else {
        char::decode_utf16(std::iter::once(unicode_char)).next()
      };

      let ch = match decoded {
        Some(Ok(c)) => c,
        _ => continue, // Invalid surrogate pair, skip
      };

      let mut key_buf = [0u8; 32];
      let mut offset = 0;
      if prefix_len > 0 {
        key_buf[0] = 0x1b; // ESC
        offset = 1;
      }
      let encoded = ch.encode_utf8(&mut key_buf[offset..]);
      let total_len = offset + encoded.len();

      unsafe {
        (&mut (*tty).internal_last_key)[..total_len]
          .copy_from_slice(&key_buf[..total_len]);
        (*tty).internal_last_key_len = total_len as u8;
        (*tty).internal_last_key_offset = 0;
        (*tty).internal_last_repeat_count = kev.wRepeatCount.saturating_sub(1);
      }
    } else {
      // Function key pressed (no Unicode character).
      let shift = (kev.dwControlKeyState & win_console::SHIFT_PRESSED) != 0;
      let ctrl = (kev.dwControlKeyState
        & (win_console::LEFT_CTRL_PRESSED | win_console::RIGHT_CTRL_PRESSED))
        != 0;

      let vt100 = match get_vt100_fn_key(kev.wVirtualKeyCode, shift, ctrl) {
        Some(seq) => seq,
        None => continue, // Unknown function key, skip
      };

      // Alt prefix for function keys.
      let alt_held = (kev.dwControlKeyState
        & (win_console::LEFT_ALT_PRESSED | win_console::RIGHT_ALT_PRESSED))
        != 0;
      let prefix_len = if alt_held { 1usize } else { 0usize };

      let total_len = prefix_len + vt100.len();
      if total_len > 32 {
        continue; // Sequence too long, skip
      }

      unsafe {
        if prefix_len > 0 {
          (*tty).internal_last_key[0] = 0x1b; // ESC
        }
        (&mut (*tty).internal_last_key)[prefix_len..total_len]
          .copy_from_slice(vt100);
        (*tty).internal_last_key_len = total_len as u8;
        (*tty).internal_last_key_offset = 0;
        (*tty).internal_last_repeat_count = kev.wRepeatCount.saturating_sub(1);
      }
    }
  }

  if buf_used > 0 {
    Ok(buf_used)
  } else {
    Err(std::io::Error::new(
      std::io::ErrorKind::WouldBlock,
      "no console input available",
    ))
  }
}

// ---- Public API ----

/// ### Safety
/// `loop_` must be initialized by `uv_loop_init`. `tty` must be a valid,
/// writable pointer. `fd` must be a valid file descriptor referring to a TTY.
pub unsafe fn uv_tty_init(
  loop_: *mut uv_loop_t,
  tty: *mut uv_tty_t,
  fd: c_int,
  _unused: c_int,
) -> c_int {
  unsafe {
    use std::ptr::addr_of_mut;
    use std::ptr::write;

    // Platform validation runs first — we don't write any fields until
    // we know init will succeed, matching libuv's pattern where
    // uv__stream_init only runs after validation passes.

    #[cfg(unix)]
    let mut actual_fd;
    #[cfg(unix)]
    let reactor: Option<TtyReactor>;

    #[cfg(unix)]
    {
      let handle_type = super::uv_guess_handle(fd);
      if handle_type == uv_handle_type::UV_FILE
        || handle_type == uv_handle_type::UV_UNKNOWN_HANDLE
      {
        return UV_EINVAL;
      }

      // Save the fd flags in case we need to restore on error.
      let saved_flags = loop {
        let f = libc::fcntl(fd, libc::F_GETFL);
        if f != -1 {
          break f;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
          return -err.raw_os_error().unwrap_or(libc::EINVAL);
        }
      };
      let mode = saved_flags & libc::O_ACCMODE;

      // Reopen the file descriptor when it refers to a tty slave.
      // This gives us our own struct file in the kernel so that
      // setting O_NONBLOCK doesn't affect other processes sharing
      // the fd (e.g. `node | cat`).
      //
      // Reopening a pty master won't work: on *BSD it opens in
      // slave mode, on Linux it allocates a new master/slave pair.
      // So we only reopen slave devices.
      // Unlike libuv which dup2's the reopened fd over the original,
      // we keep the original fd untouched and use the new fd directly.
      // This prevents setting O_NONBLOCK on stdin/stdout/stderr from
      // affecting other users of those fds (e.g. rustyline in the REPL).
      actual_fd = fd;
      let mut reopened = false;
      if handle_type == uv_handle_type::UV_TTY && tty_is_slave(fd) {
        let mut path = [0u8; 256];
        if libc::ttyname_r(fd, path.as_mut_ptr().cast(), path.len()) == 0 {
          let new_fd =
            open_cloexec(path.as_ptr().cast(), mode | libc::O_NOCTTY);
          if new_fd >= 0 {
            actual_fd = new_fd;
            reopened = true;
          }
        }
      }

      // Set non-blocking.
      let cur_flags = libc::fcntl(actual_fd, libc::F_GETFL);
      if cur_flags == -1 {
        return -std::io::Error::last_os_error()
          .raw_os_error()
          .unwrap_or(libc::EINVAL);
      }
      if cur_flags & libc::O_NONBLOCK == 0
        && libc::fcntl(actual_fd, libc::F_SETFL, cur_flags | libc::O_NONBLOCK)
          == -1
      {
        if reopened {
          libc::fcntl(fd, libc::F_SETFL, saved_flags);
        }
        return -std::io::Error::last_os_error()
          .raw_os_error()
          .unwrap_or(libc::EINVAL);
      }

      // Wrap in AsyncFd for reactor integration.
      //
      // On macOS, kqueue doesn't work with some /dev devices (e.g.
      // /dev/tty). When AsyncFd::new fails, we probe kqueue to
      // confirm, then set up a select(2) fallback thread — matching
      // libuv's uv__stream_try_select.
      match AsyncFd::new(TtyFd(actual_fd)) {
        Ok(afd) => {
          reactor = Some(TtyReactor::Normal(afd));
        }
        Err(e) => {
          #[cfg(target_os = "macos")]
          if kqueue_rejects_fd(actual_fd) {
            // kqueue confirmed incompatible — set up select fallback.
            match setup_select_fallback(actual_fd) {
              Ok(fallback) => {
                reactor = Some(TtyReactor::SelectFallback(fallback));
              }
              Err(err_code) => {
                libc::fcntl(actual_fd, libc::F_SETFL, cur_flags);
                if reopened {
                  libc::close(actual_fd);
                }
                return err_code;
              }
            }
          } else {
            libc::fcntl(actual_fd, libc::F_SETFL, cur_flags);
            if reopened {
              libc::close(actual_fd);
            }
            return io_error_to_uv(&e);
          }

          #[cfg(not(target_os = "macos"))]
          {
            libc::fcntl(actual_fd, libc::F_SETFL, cur_flags);
            if reopened {
              libc::close(actual_fd);
            }
            return io_error_to_uv(&e);
          }
        }
      }
    };

    #[cfg(windows)]
    let (win_handle, win_readable, win_saved_mode, win_handle_owned) = {
      let raw_handle = win_console::safe_get_osfhandle(fd);
      if raw_handle == -1 {
        return UV_EBADF;
      }
      let mut handle = raw_handle as *mut c_void;

      // For stdio fds 0-2, duplicate the handle so closing the uv_tty
      // doesn't close the original stdio handle.
      if fd <= 2 {
        let mut dup: *mut c_void = std::ptr::null_mut();
        let current_process = win_console::GetCurrentProcess();
        if win_console::DuplicateHandle(
          current_process,
          handle,
          current_process,
          &mut dup,
          0,
          0,
          win_console::DUPLICATE_SAME_ACCESS,
        ) == 0
        {
          return UV_EBADF;
        }
        handle = dup;
      }

      let mut dummy: u32 = 0;
      let readable =
        win_console::GetNumberOfConsoleInputEvents(handle, &mut dummy) != 0;

      let mut saved_mode: u32 = 0;
      win_console::GetConsoleMode(handle, &mut saved_mode);

      // For writable (output) handles, validate via
      // GetConsoleScreenBufferInfo (matching libuv) and try to enable
      // virtual terminal processing for ANSI escape sequence support.
      if !readable {
        let mut info: win_console::CONSOLE_SCREEN_BUFFER_INFO =
          std::mem::zeroed();
        if win_console::GetConsoleScreenBufferInfo(handle, &mut info) == 0 {
          // Translate the Windows error to a uv error code, matching libuv's
          // uv_translate_sys_error(GetLastError()) behavior.
          let win_err = win_console::GetLastError();
          if fd <= 2 {
            win_console::CloseHandle(handle);
          }
          const ERROR_INVALID_HANDLE: u32 = 6;
          const ERROR_ACCESS_DENIED: u32 = 5;
          return match win_err {
            ERROR_INVALID_HANDLE => UV_EBADF,
            ERROR_ACCESS_DENIED => UV_EBADF,
            _ => UV_EINVAL,
          };
        }
        let mut current_mode: u32 = 0;
        if win_console::GetConsoleMode(handle, &mut current_mode) != 0 {
          let _ = win_console::SetConsoleMode(
            handle,
            current_mode | win_console::ENABLE_VIRTUAL_TERMINAL_PROCESSING,
          );
        }
      }

      (handle, readable, saved_mode, fd <= 2)
    };

    // -- Validation passed. Initialize all fields. --

    write(addr_of_mut!((*tty).r#type), uv_handle_type::UV_TTY);
    write(addr_of_mut!((*tty).loop_), loop_);
    write(addr_of_mut!((*tty).data), std::ptr::null_mut());
    write(addr_of_mut!((*tty).flags), UV_HANDLE_REF);
    write(addr_of_mut!((*tty).mode), uv_tty_mode_t::UV_TTY_MODE_NORMAL);
    write(addr_of_mut!((*tty).internal_alloc_cb), None);
    write(addr_of_mut!((*tty).internal_read_cb), None);
    write(addr_of_mut!((*tty).internal_reading), false);
    write(addr_of_mut!((*tty).internal_write_queue), VecDeque::new());
    write(addr_of_mut!((*tty).internal_shutdown), None);

    #[cfg(unix)]
    {
      write(addr_of_mut!((*tty).internal_fd), actual_fd);
      write(addr_of_mut!((*tty).internal_reactor), reactor);
      write(addr_of_mut!((*tty).internal_orig_termios), None);
    }

    #[cfg(windows)]
    {
      write(addr_of_mut!((*tty).internal_handle), win_handle);
      write(addr_of_mut!((*tty).internal_readable), win_readable);
      write(addr_of_mut!((*tty).internal_saved_mode), win_saved_mode);
      write(addr_of_mut!((*tty).internal_handle_owned), win_handle_owned);
      write(addr_of_mut!((*tty).internal_fd), fd);
      write(addr_of_mut!((*tty).internal_last_key), [0u8; 32]);
      write(addr_of_mut!((*tty).internal_last_key_len), 0);
      write(addr_of_mut!((*tty).internal_last_key_offset), 0);
      write(addr_of_mut!((*tty).internal_last_repeat_count), 0);
      write(addr_of_mut!((*tty).internal_utf16_high_surrogate), 0);
      write(
        addr_of_mut!((*tty).internal_wait_handle),
        std::ptr::null_mut(),
      );
      write(addr_of_mut!((*tty).internal_wait_waker), None);
      write(addr_of_mut!((*tty).internal_line_read_result), None);
      write(addr_of_mut!((*tty).internal_line_read_pending), false);
    }
  }
  0
}

/// ### Safety
/// `tty` must be a valid pointer to a `uv_tty_t` initialized by `uv_tty_init`.
pub unsafe fn uv_tty_set_mode(
  tty: *mut uv_tty_t,
  mode: uv_tty_mode_t,
) -> c_int {
  unsafe {
    #[cfg(unix)]
    {
      use uv_tty_mode_t::*;

      // On Unix, RAW_VT is treated as RAW (there is only one raw mode).
      let effective_mode = if mode == UV_TTY_MODE_RAW_VT {
        UV_TTY_MODE_RAW
      } else {
        mode
      };

      if (*tty).mode == effective_mode {
        return 0;
      }

      let fd = (*tty).internal_fd;

      // When transitioning from normal to non-normal, save orig termios.
      if (*tty).mode == UV_TTY_MODE_NORMAL
        && effective_mode != UV_TTY_MODE_NORMAL
      {
        let mut orig: libc::termios = std::mem::zeroed();
        loop {
          let rc = libc::tcgetattr(fd, &mut orig);
          if rc == 0 {
            break;
          }
          let err = std::io::Error::last_os_error();
          if err.raw_os_error() != Some(libc::EINTR) {
            return -err.raw_os_error().unwrap_or(libc::EINVAL);
          }
        }
        (*tty).internal_orig_termios = Some(orig);

        // Save globally for uv_tty_reset_mode.
        global_termios::acquire();
        global_termios::save_if_first(fd, &orig);
        global_termios::release();
      }

      let orig = match (*tty).internal_orig_termios {
        Some(ref t) => *t,
        None => {
          let mut t: libc::termios = std::mem::zeroed();
          loop {
            let rc = libc::tcgetattr(fd, &mut t);
            if rc == 0 {
              break;
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EINTR) {
              return -err.raw_os_error().unwrap_or(libc::EINVAL);
            }
          }
          t
        }
      };

      let mut tmp = orig;
      match effective_mode {
        UV_TTY_MODE_NORMAL => { /* restore original settings */ }
        UV_TTY_MODE_RAW => {
          tmp.c_iflag &= !(libc::BRKINT
            | libc::ICRNL
            | libc::INPCK
            | libc::ISTRIP
            | libc::IXON);
          tmp.c_oflag |= libc::ONLCR;
          tmp.c_cflag |= libc::CS8;
          tmp.c_lflag &=
            !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
          tmp.c_cc[libc::VMIN] = 1;
          tmp.c_cc[libc::VTIME] = 0;
        }
        UV_TTY_MODE_IO => {
          libc::cfmakeraw(&mut tmp);
        }
        _ => return UV_EINVAL,
      }

      // Apply with TCSADRAIN to allow queued output to drain.
      let rc = tcsetattr_eintr(fd, libc::TCSADRAIN, &tmp);
      if rc == 0 {
        (*tty).mode = effective_mode;
      }
      rc
    }

    #[cfg(windows)]
    {
      use uv_tty_mode_t::*;

      if !(*tty).internal_readable {
        return UV_EINVAL;
      }

      if (*tty).mode == mode {
        return 0;
      }

      let handle = (*tty).internal_handle;

      // Save the original console mode globally for uv_tty_reset_mode.
      global_console::save_if_first();

      // TODO: libuv stops in-progress reads before changing mode and
      // restarts them after, because Windows uses different read
      // mechanisms for different console modes. Once we have threaded
      // reads, add stop/restart logic here.
      let (flags, try_flags) = match mode {
        // Restore the original console mode saved at init time,
        // matching libuv which restores `uv__tty_console_orig_mode`.
        // This preserves flags like ENABLE_QUICK_EDIT_MODE,
        // ENABLE_INSERT_MODE, and ENABLE_EXTENDED_FLAGS that are
        // present by default on Windows.
        UV_TTY_MODE_NORMAL => ((*tty).internal_saved_mode, 0),
        UV_TTY_MODE_RAW => (win_console::ENABLE_WINDOW_INPUT, 0),
        UV_TTY_MODE_RAW_VT => (
          win_console::ENABLE_WINDOW_INPUT,
          win_console::ENABLE_VIRTUAL_TERMINAL_INPUT,
        ),
        UV_TTY_MODE_IO => return super::UV_ENOTSUP,
      };

      // Try with optional flags first, fall back without.
      if win_console::SetConsoleMode(handle, flags | try_flags) == 0
        && (try_flags == 0 || win_console::SetConsoleMode(handle, flags) == 0)
      {
        return UV_EINVAL;
      }

      // Only mark needs_reset for RAW/RAW_VT modes (matching libuv),
      // and only after the mode change succeeds.
      if mode != UV_TTY_MODE_NORMAL {
        global_console::mark_needs_reset();
      }

      (*tty).mode = mode;
      0
    }
  }
}

/// ### Safety
/// `tty` must be a valid pointer to a `uv_tty_t` initialized by `uv_tty_init`.
/// `width` and `height` must be valid, writable pointers.
pub unsafe fn uv_tty_get_winsize(
  tty: *mut uv_tty_t,
  width: *mut c_int,
  height: *mut c_int,
) -> c_int {
  unsafe {
    #[cfg(unix)]
    {
      let fd = (*tty).internal_fd;
      let mut ws: libc::winsize = std::mem::zeroed();
      loop {
        let rc = libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws);
        if rc == 0 {
          break;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
          return -err.raw_os_error().unwrap_or(libc::EINVAL);
        }
      }
      *width = ws.ws_col as c_int;
      *height = ws.ws_row as c_int;
      0
    }

    #[cfg(windows)]
    {
      let handle = (*tty).internal_handle;
      let mut info: win_console::CONSOLE_SCREEN_BUFFER_INFO =
        std::mem::zeroed();
      if win_console::GetConsoleScreenBufferInfo(handle, &mut info) == 0 {
        return UV_EINVAL;
      }
      // libuv uses buffer width (dwSize.X) and visible window height,
      // matching its virtual window abstraction.
      *width = info.dwSize.X as c_int;
      *height = (info.srWindow.Bottom - info.srWindow.Top + 1) as c_int;
      0
    }
  }
}

/// Reset the console to its original mode. This function is
/// async-signal-safe on Unix.
pub fn uv_tty_reset_mode() -> c_int {
  #[cfg(unix)]
  {
    global_termios::reset()
  }

  #[cfg(windows)]
  {
    global_console::reset()
  }
}

pub fn uv_tty_set_vterm_state(_state: uv_tty_vtermstate_t) {
  // No-op on Unix. On Windows this would control ANSI processing.
}

pub fn uv_tty_get_vterm_state(_state: *mut uv_tty_vtermstate_t) -> c_int {
  #[cfg(unix)]
  {
    super::UV_ENOTSUP
  }
  #[cfg(windows)]
  {
    // TODO: Track vterm state on Windows.
    super::UV_ENOTSUP
  }
}

// ---- Stream operation entry points (called from stream.rs dispatch) ----

/// ### Safety
/// `tty` must be a valid pointer to an initialized `uv_tty_t`.
pub(crate) unsafe fn read_start_tty(
  tty: *mut uv_tty_t,
  alloc_cb: Option<uv_alloc_cb>,
  read_cb: Option<uv_read_cb>,
) -> c_int {
  if alloc_cb.is_none() || read_cb.is_none() {
    return UV_EINVAL;
  }
  unsafe {
    // Match libuv: reject closing handles.
    if (*tty).flags & super::UV_HANDLE_CLOSING != 0 {
      return UV_EINVAL;
    }
    // Match libuv: return UV_EALREADY if already reading.
    if (*tty).internal_reading {
      return super::UV_EALREADY;
    }
    (*tty).internal_alloc_cb = alloc_cb;
    (*tty).internal_read_cb = read_cb;
    (*tty).internal_reading = true;
    (*tty).flags |= UV_HANDLE_ACTIVE;

    // Notify the select fallback thread about interest change.
    #[cfg(target_os = "macos")]
    update_select_interest(tty);

    let inner = get_inner((*tty).loop_);
    let mut handles = inner.tty_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tty)) {
      handles.push(tty);
    }
    drop(handles);

    // Wake the event loop so poll_tty_handle runs on the next tick.
    // This registers the RegisterWaitForSingleObject (for future
    // input notifications) and drains any pending stdout write
    // callbacks. Without this, the event loop may park before the
    // first poll_tty_handle call, causing the prompt to not render
    // until the user presses a key.
    #[cfg(windows)]
    inner.wake();
  }
  0
}

/// ### Safety
/// `tty` must be a valid pointer to an initialized `uv_tty_t`.
pub(crate) unsafe fn read_stop_tty(tty: *mut uv_tty_t) -> c_int {
  unsafe {
    (*tty).internal_reading = false;
    (*tty).internal_alloc_cb = None;
    (*tty).internal_read_cb = None;

    // Unregister the console input wait.
    #[cfg(windows)]
    tty_unregister_wait(tty);

    // Notify the select fallback thread about interest change.
    #[cfg(target_os = "macos")]
    update_select_interest(tty);
    if (*tty).internal_write_queue.is_empty()
      && (*tty).internal_shutdown.is_none()
    {
      (*tty).flags &= !UV_HANDLE_ACTIVE;
      // Remove from poll list when fully inactive.
      let inner = get_inner((*tty).loop_);
      inner
        .tty_handles
        .borrow_mut()
        .retain(|&h| !std::ptr::eq(h, tty));
    }
  }
  0
}

/// ### Safety
/// `handle` must be a valid pointer to an initialized `uv_tty_t` (cast
/// as `uv_stream_t`).
pub(crate) unsafe fn try_write_tty(
  handle: *mut uv_stream_t,
  data: &[u8],
) -> i32 {
  let tty = handle as *mut uv_tty_t;
  unsafe {
    if !(*tty).internal_write_queue.is_empty() {
      return UV_EAGAIN;
    }
    match tty_try_write(tty, data) {
      Ok(n) => n as i32,
      Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => UV_EAGAIN,
      Err(_) => UV_EPIPE,
    }
  }
}

/// ### Safety
/// `req` must be valid and remain so until the write callback fires.
/// `handle` must be an initialized `uv_tty_t` (cast as `uv_stream_t`).
/// `bufs` must point to `nbufs` valid `uv_buf_t` entries.
pub(crate) unsafe fn write_tty(
  req: *mut uv_write_t,
  handle: *mut uv_stream_t,
  bufs: *const uv_buf_t,
  nbufs: u32,
  cb: Option<uv_write_cb>,
) -> c_int {
  unsafe {
    let tty = handle as *mut uv_tty_t;
    (*req).handle = handle;

    // If writes are already queued, just append.
    if !(*tty).internal_write_queue.is_empty() {
      let data = collect_bufs(bufs, nbufs);
      (*tty).internal_write_queue.push_back(WritePending {
        req,
        data,
        offset: 0,
        cb,
        status: None,
      });
      return 0;
    }

    // Never fire callbacks synchronously — always queue.
    // This matches real libuv behavior and prevents re-entrancy panics.
    if nbufs == 1 {
      let buf = &*bufs;
      if !buf.base.is_null() && buf.len > 0 {
        let data = std::slice::from_raw_parts(buf.base as *const u8, buf.len);
        let mut offset = 0;
        loop {
          match tty_try_write(tty, &data[offset..]) {
            Ok(n) => {
              offset += n;
              if offset >= data.len() {
                (*tty).internal_write_queue.push_back(WritePending {
                  req,
                  data: Vec::new(),
                  offset: 0,
                  cb,
                  status: Some(0),
                });
                ensure_tty_registered(tty);
                return 0;
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
              (*tty).internal_write_queue.push_back(WritePending {
                req,
                data: data[offset..].to_vec(),
                offset: 0,
                cb,
                status: None,
              });
              ensure_tty_registered(tty);
              return 0;
            }
            Err(_) => {
              (*tty).internal_write_queue.push_back(WritePending {
                req,
                data: Vec::new(),
                offset: 0,
                cb,
                status: Some(UV_EPIPE),
              });
              ensure_tty_registered(tty);
              return 0;
            }
          }
        }
      }
      (*tty).internal_write_queue.push_back(WritePending {
        req,
        data: Vec::new(),
        offset: 0,
        cb,
        status: Some(0),
      });
      ensure_tty_registered(tty);
      return 0;
    }

    // Multi-buffer: collect and write.
    let data = collect_bufs(bufs, nbufs);
    if data.is_empty() {
      (*tty).internal_write_queue.push_back(WritePending {
        req,
        data: Vec::new(),
        offset: 0,
        cb,
        status: Some(0),
      });
      ensure_tty_registered(tty);
      return 0;
    }

    let mut offset = 0;
    loop {
      match tty_try_write(tty, &data[offset..]) {
        Ok(n) => {
          offset += n;
          if offset >= data.len() {
            (*tty).internal_write_queue.push_back(WritePending {
              req,
              data: Vec::new(),
              offset: 0,
              cb,
              status: Some(0),
            });
            ensure_tty_registered(tty);
            return 0;
          }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          (*tty).internal_write_queue.push_back(WritePending {
            req,
            data: data[offset..].to_vec(),
            offset: 0,
            cb,
            status: None,
          });
          ensure_tty_registered(tty);
          return 0;
        }
        Err(_) => {
          (*tty).internal_write_queue.push_back(WritePending {
            req,
            data: Vec::new(),
            offset: 0,
            cb,
            status: Some(UV_EPIPE),
          });
          ensure_tty_registered(tty);
          return 0;
        }
      }
    }
  }
}

/// Ensure the TTY handle is registered for polling so queued writes
/// (and other deferred work) get drained by the event loop.
///
/// # Safety
/// `tty` must be a valid pointer to an initialized `uv_tty_t`.
unsafe fn ensure_tty_registered(tty: *mut uv_tty_t) {
  unsafe {
    (*tty).flags |= UV_HANDLE_ACTIVE;

    // Notify the select fallback thread about write interest.
    #[cfg(target_os = "macos")]
    update_select_interest(tty);

    let inner = get_inner((*tty).loop_);
    let mut handles = inner.tty_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tty)) {
      handles.push(tty);
    }
    drop(handles);

    // On Windows, wake the event loop so pending write callbacks are
    // processed promptly. Without this, deferred callbacks can stall
    // until something else wakes the loop (e.g. console input).
    #[cfg(windows)]
    inner.wake();
  }
}

/// ### Safety
/// `req` must be a valid, writable pointer. `stream` must be an initialized
/// `uv_tty_t` (cast as `uv_stream_t`). `req` must remain valid until the
/// shutdown callback fires.
pub(crate) unsafe fn shutdown_tty(
  req: *mut uv_shutdown_t,
  stream: *mut uv_stream_t,
  cb: Option<uv_shutdown_cb>,
) -> c_int {
  unsafe {
    // Match libuv: reject shutdown on closing streams.
    if (*stream).flags & super::UV_HANDLE_CLOSING != 0 {
      return super::UV_ENOTCONN;
    }
    let tty = stream as *mut uv_tty_t;
    (*req).handle = stream;

    // Match libuv: reject if already shutting down.
    if (*tty).internal_shutdown.is_some() {
      return super::UV_EALREADY;
    }

    (*tty).internal_shutdown = Some(ShutdownPending { req, cb });

    let inner = get_inner((*tty).loop_);
    let mut handles = inner.tty_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tty)) {
      handles.push(tty);
    }
    (*tty).flags |= UV_HANDLE_ACTIVE;
  }
  0
}

// ---- Polling ----

/// Poll a single TTY handle for I/O readiness and fire callbacks.
/// Callback invoked by the thread pool when the console input handle is
/// signaled (input available). Wakes the tokio event loop so
/// poll_tty_handle runs again. Matches libuv's uv_tty_post_raw_read.
#[cfg(windows)]
unsafe extern "system" fn win_tty_wait_callback(
  context: *mut c_void,
  _timer_or_wait_fired: u8,
) {
  // context is a raw pointer to Box<Mutex<Option<Waker>>> that we
  // leaked in tty_register_wait. We must NOT drop it here -- just
  // clone the waker and wake.
  let mutex = unsafe { &*(context as *const std::sync::Mutex<Option<Waker>>) };
  if let Ok(guard) = mutex.lock()
    && let Some(waker) = guard.as_ref()
  {
    waker.wake_by_ref();
  }
}

/// Register a thread pool wait on the console input handle. When input
/// becomes available, the callback wakes the tokio event loop.
/// Matches libuv's uv__tty_queue_read_raw which uses
/// RegisterWaitForSingleObject.
#[cfg(windows)]
unsafe fn tty_register_wait(tty: *mut uv_tty_t, waker: &Waker) {
  unsafe {
    // Unregister any existing wait first.
    tty_unregister_wait(tty);

    // Create or update the shared waker.
    if (*tty).internal_wait_waker.is_none() {
      (*tty).internal_wait_waker =
        Some(Box::new(std::sync::Mutex::new(Some(waker.clone()))));
    } else if let Some(ref mutex) = (*tty).internal_wait_waker {
      *mutex.lock().unwrap() = Some(waker.clone());
    }

    // Get a raw pointer to the Mutex for the callback context.
    // The Box keeps it alive as long as the tty handle exists.
    let ctx = (*tty).internal_wait_waker.as_ref().unwrap().as_ref()
      as *const std::sync::Mutex<Option<Waker>> as *mut c_void;

    let mut wait_handle: *mut c_void = std::ptr::null_mut();
    let ret = win_console::RegisterWaitForSingleObject(
      &mut wait_handle,
      (*tty).internal_handle,
      win_tty_wait_callback,
      ctx,
      win_console::INFINITE,
      win_console::WT_EXECUTEINWAITTHREAD | win_console::WT_EXECUTEONLYONCE,
    );
    if ret != 0 {
      (*tty).internal_wait_handle = wait_handle;
    }
  }
}

/// Tear down Windows-specific async read machinery before closing
/// the console handle.  Must be called from `stop_tty()` *before*
/// `CloseHandle`/`_close` so the wait callback cannot fire on a
/// closed handle or freed waker.
#[cfg(windows)]
pub(crate) unsafe fn close_tty_read(tty: *mut uv_tty_t) {
  unsafe {
    // 1. Unregister the thread-pool wait so the callback cannot fire
    //    after the handle is closed.
    tty_unregister_wait(tty);

    // 2. Drop the waker box so the callback context pointer is
    //    invalidated deterministically (after the wait is fully
    //    unregistered above).
    (*tty).internal_wait_waker = None;

    // 3. Detach from any in-flight line-mode reader.  The spawned
    //    thread only touches its own Arc clone and a copied handle
    //    value, so closing the console handle will unblock
    //    ReadConsoleW with an error and the thread will exit
    //    gracefully.  Clear our side so poll_tty_handle will not try
    //    to collect the result after the handle is freed.
    (*tty).internal_line_read_result = None;
    (*tty).internal_line_read_pending = false;
  }
}

/// Unregister the thread pool wait. Safe to call even if no wait is
/// registered.
#[cfg(windows)]
unsafe fn tty_unregister_wait(tty: *mut uv_tty_t) {
  unsafe {
    if !(*tty).internal_wait_handle.is_null() {
      // INVALID_HANDLE_VALUE tells UnregisterWaitEx to block until
      // the callback has finished (if running).
      win_console::UnregisterWaitEx(
        (*tty).internal_wait_handle,
        -1isize as *mut c_void,
      );
      (*tty).internal_wait_handle = std::ptr::null_mut();
    }
  }
}

/// Returns `true` if any work was completed.
///
/// # Safety
/// `tty_ptr` must be a valid pointer to an initialized `uv_tty_t`.
pub(crate) unsafe fn poll_tty_handle(
  tty_ptr: *mut uv_tty_t,
  cx: &mut Context<'_>,
) -> bool {
  let mut any_work = false;
  #[cfg(target_os = "macos")]
  let mut drained_select_signal = false;

  // 1. Poll readable stream.
  unsafe {
    if (*tty_ptr).internal_reading {
      let alloc_cb = (*tty_ptr).internal_alloc_cb;
      let read_cb = (*tty_ptr).internal_read_cb;
      if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
        // Check readiness via AsyncFd. We must hold the guard alive during
        // reads so that dropping it (which clears readiness) only happens
        // after we've actually attempted the I/O.
        #[cfg(unix)]
        {
          if let Some(ref reactor) = (*tty_ptr).internal_reactor {
            match reactor.async_fd().poll_read_ready(cx) {
              std::task::Poll::Ready(Ok(mut guard)) => {
                // For the select fallback, drain the signaling byte(s)
                // from the socketpair so the AsyncFd can become
                // not-ready again.
                #[cfg(target_os = "macos")]
                if let TtyReactor::SelectFallback(s) = reactor
                  && drain_fd(s.fake_fd)
                {
                  drained_select_signal = true;
                }
                // Try reading in a loop while we hold the guard.
                loop {
                  if !(*tty_ptr).internal_reading {
                    break;
                  }
                  let mut buf = uv_buf_t {
                    base: std::ptr::null_mut(),
                    len: 0,
                  };
                  alloc_cb(tty_ptr as *mut uv_handle_t, 65536, &mut buf);
                  if buf.base.is_null() || buf.len == 0 {
                    read_cb(
                      tty_ptr as *mut uv_stream_t,
                      UV_ENOBUFS as isize,
                      &buf,
                    );
                    break;
                  }
                  let slice = std::slice::from_raw_parts_mut(
                    buf.base.cast::<u8>(),
                    buf.len,
                  );
                  match tty_try_read(tty_ptr, slice) {
                    Ok(0) => {
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        UV_EOF as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                      break;
                    }
                    Ok(n) => {
                      any_work = true;
                      read_cb(tty_ptr as *mut uv_stream_t, n as isize, &buf);
                    }
                    Err(ref e)
                      if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                      // Signal the caller to free the buffer (nread=0).
                      read_cb(tty_ptr as *mut uv_stream_t, 0, &buf);
                      // Tell tokio we didn't actually read — need to
                      // re-poll for readiness.
                      guard.clear_ready();
                      break;
                    }
                    Err(ref e) => {
                      // Match libuv: report real error codes, not UV_EOF.
                      let status = io_error_to_uv(e);
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        status as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                      break;
                    }
                  }
                }
                // Guard is dropped here — readiness cleared unless we
                // called clear_ready() above already.
              }
              std::task::Poll::Ready(Err(_)) | std::task::Poll::Pending => {
                // Not ready yet or error — will be woken when ready.
              }
            }
          }
        }

        // On non-Unix (Windows), use mode-aware reading.
        // Raw mode: ReadConsoleInputW to process individual INPUT_RECORD
        // structs (matching libuv's uv_process_tty_read_raw_req).
        // Line mode: ReadConsoleW on a worker thread (matching libuv's
        // uv_tty_line_read_thread + QueueUserWorkItem).
        #[cfg(not(unix))]
        {
          if (*tty_ptr).internal_readable && (*tty_ptr).internal_reading {
            if is_raw_tty_mode((*tty_ptr).mode) {
              // === Raw mode ===
              let has_pending = (*tty_ptr).internal_last_key_offset
                < (*tty_ptr).internal_last_key_len
                || (*tty_ptr).internal_last_repeat_count > 0
                || {
                  let mut n: u32 = 0;
                  win_console::GetNumberOfConsoleInputEvents(
                    (*tty_ptr).internal_handle,
                    &mut n,
                  ) != 0
                    && n > 0
                };

              if has_pending {
                let mut buf = uv_buf_t {
                  base: std::ptr::null_mut(),
                  len: 0,
                };
                alloc_cb(tty_ptr as *mut uv_handle_t, 65536, &mut buf);
                if buf.base.is_null() || buf.len == 0 {
                  read_cb(
                    tty_ptr as *mut uv_stream_t,
                    UV_ENOBUFS as isize,
                    &buf,
                  );
                } else {
                  let slice = std::slice::from_raw_parts_mut(
                    buf.base.cast::<u8>(),
                    buf.len,
                  );
                  match tty_try_read_raw(tty_ptr, slice) {
                    Ok(0) => {
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        UV_EOF as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                    }
                    Ok(n) => {
                      any_work = true;
                      read_cb(tty_ptr as *mut uv_stream_t, n as isize, &buf);
                    }
                    Err(ref e)
                      if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                      read_cb(tty_ptr as *mut uv_stream_t, 0, &buf);
                    }
                    Err(ref e) => {
                      // Match libuv: report real error codes, not UV_EOF.
                      let status = io_error_to_uv(e);
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        status as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                    }
                  }
                }
              }

              // Re-register the wait for raw mode.
              if (*tty_ptr).internal_reading {
                tty_register_wait(tty_ptr, cx.waker());
              }
            } else {
              // === Line mode (NORMAL) ===
              // ReadConsoleW blocks until Enter, so it runs on a
              // worker thread matching libuv's
              // uv_tty_line_read_thread.

              // 1. Check for completed line-read result from thread.
              if let Some(ref arc) = (*tty_ptr).internal_line_read_result {
                let result = {
                  if let Ok(mut guard) = arc.lock() {
                    guard.take()
                  } else {
                    None
                  }
                };
                if let Some(result) = result {
                  (*tty_ptr).internal_line_read_pending = false;
                  let mut buf = uv_buf_t {
                    base: std::ptr::null_mut(),
                    len: 0,
                  };
                  match result {
                    Ok(ref data) if data.is_empty() => {
                      alloc_cb(tty_ptr as *mut uv_handle_t, 65536, &mut buf);
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        UV_EOF as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                    }
                    Ok(ref data) => {
                      alloc_cb(
                        tty_ptr as *mut uv_handle_t,
                        data.len(),
                        &mut buf,
                      );
                      if buf.base.is_null() || buf.len == 0 {
                        read_cb(
                          tty_ptr as *mut uv_stream_t,
                          UV_ENOBUFS as isize,
                          &buf,
                        );
                      } else {
                        let copy_len = data.len().min(buf.len);
                        std::ptr::copy_nonoverlapping(
                          data.as_ptr(),
                          buf.base as *mut u8,
                          copy_len,
                        );
                        any_work = true;
                        read_cb(
                          tty_ptr as *mut uv_stream_t,
                          copy_len as isize,
                          &buf,
                        );
                      }
                    }
                    Err(_) => {
                      alloc_cb(tty_ptr as *mut uv_handle_t, 65536, &mut buf);
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        UV_EOF as isize,
                        &buf,
                      );
                      (*tty_ptr).internal_reading = false;
                    }
                  }
                }
              }
              // 2. Spawn a worker thread if no read is in flight.
              if (*tty_ptr).internal_reading
                && !(*tty_ptr).internal_line_read_pending
              {
                let handle = (*tty_ptr).internal_handle;
                let result_arc: LineReadResult =
                  std::sync::Arc::new(std::sync::Mutex::new(None));
                (*tty_ptr).internal_line_read_result = Some(result_arc.clone());
                (*tty_ptr).internal_line_read_pending = true;

                let waker = cx.waker().clone();

                // SAFETY: handle is a duplicated console input
                // HANDLE that remains valid until uv_close.
                let handle_usize = handle as usize;

                let _ = std::thread::Builder::new()
                  .name("tty-line-read".into())
                  .spawn(move || {
                    const MAX_BUF: usize = 8192;
                    let max_chars = MAX_BUF / 3;
                    let mut utf16_buf = vec![0u16; max_chars];
                    let mut chars_read: u32 = 0;

                    let h = handle_usize as *mut c_void;
                    let ok = win_console::ReadConsoleW(
                      h,
                      utf16_buf.as_mut_ptr(),
                      max_chars as u32,
                      &mut chars_read,
                      std::ptr::null_mut(),
                    );

                    let result = if ok != 0 && chars_read > 0 {
                      let utf16 = &utf16_buf[..chars_read as usize];
                      Ok(String::from_utf16_lossy(utf16).into_bytes())
                    } else if ok != 0 {
                      Ok(Vec::new())
                    } else {
                      Err(-1)
                    };

                    if let Ok(mut guard) = result_arc.lock() {
                      *guard = Some(result);
                    }
                    waker.wake();
                  });
              }
            }
          }
        }
      }
    }
  }

  // 2. Drain write queue.
  unsafe {
    if !(*tty_ptr).internal_write_queue.is_empty() {
      // Register write interest with reactor.
      #[cfg(unix)]
      if let Some(ref reactor) = (*tty_ptr).internal_reactor {
        let _ = reactor.async_fd().poll_write_ready(cx);
        // Drain signaling bytes for the select fallback.
        #[cfg(target_os = "macos")]
        if let TtyReactor::SelectFallback(s) = reactor
          && drain_fd(s.fake_fd)
        {
          drained_select_signal = true;
        }
      }

      loop {
        if (*tty_ptr).internal_write_queue.is_empty() {
          break;
        }

        // Check if the front entry has a pre-determined status
        // (deferred from write_tty).
        let pre_status =
          (*tty_ptr).internal_write_queue.front().unwrap().status;
        if let Some(status) = pre_status {
          let pw = (*tty_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, status);
          }
          any_work = true;
          continue;
        }

        let (done, error) = {
          let pw = (*tty_ptr).internal_write_queue.front_mut().unwrap();
          let mut done = false;
          let mut error = false;
          loop {
            if pw.offset >= pw.data.len() {
              done = true;
              break;
            }
            match tty_try_write(tty_ptr, &pw.data[pw.offset..]) {
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

        if done {
          let pw = (*tty_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, 0);
          }
        } else if error {
          let pw = (*tty_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, UV_EPIPE);
          }
        } else {
          break; // WouldBlock -- retry next tick.
        }
      }
    }
  }

  // 3. Complete shutdown once write queue is drained.
  // For TTY there is no half-close like TCP, so we just fire the
  // callback with success once the writes are flushed.
  unsafe {
    if (*tty_ptr).internal_write_queue.is_empty()
      && (*tty_ptr).internal_shutdown.is_some()
    {
      let pending = (*tty_ptr).internal_shutdown.take().unwrap();
      if let Some(cb) = pending.cb {
        cb(pending.req, 0);
      }
      // Deactivate the handle if there's no more pending work.
      // In real libuv, uv__drain stops the I/O watcher so the
      // handle no longer keeps the event loop alive. Our
      // UV_HANDLE_ACTIVE flag serves the same purpose.
      if !(*tty_ptr).internal_reading
        && (*tty_ptr).internal_write_queue.is_empty()
        && (*tty_ptr).internal_shutdown.is_none()
      {
        (*tty_ptr).flags &= !UV_HANDLE_ACTIVE;
        #[cfg(target_os = "macos")]
        update_select_interest(tty_ptr);
      }
      any_work = true;
    }
  }

  // Post the select fallback semaphore so the background thread can
  // continue. Only post when we actually drained a signal from the
  // select thread — this matches libuv's `uv_sem_post(&s->async_sem)`
  // in `uv__stream_osx_select_cb`, which only fires in response to
  // the select thread's `uv_async_send`. Posting unconditionally would
  // accumulate semaphore counts and defeat back-pressure.
  #[cfg(target_os = "macos")]
  unsafe {
    if drained_select_signal
      && let Some(TtyReactor::SelectFallback(s)) =
        (*tty_ptr).internal_reactor.as_ref()
      && (*tty_ptr).flags & super::UV_HANDLE_CLOSING == 0
    {
      s.async_sem.post();
    }
  }

  any_work
}

// ---- Convenience constructor ----

pub fn new_tty() -> uv_tty_t {
  uv_tty_t {
    r#type: uv_handle_type::UV_TTY,
    loop_: std::ptr::null_mut(),
    data: std::ptr::null_mut(),
    flags: 0,
    mode: uv_tty_mode_t::UV_TTY_MODE_NORMAL,
    internal_alloc_cb: None,
    internal_read_cb: None,
    internal_reading: false,
    internal_write_queue: VecDeque::new(),
    internal_shutdown: None,
    #[cfg(unix)]
    internal_fd: -1,
    #[cfg(unix)]
    internal_reactor: None,
    #[cfg(unix)]
    internal_orig_termios: None,
    #[cfg(windows)]
    internal_handle: std::ptr::null_mut(),
    #[cfg(windows)]
    internal_readable: false,
    #[cfg(windows)]
    internal_saved_mode: 0,
    #[cfg(windows)]
    internal_handle_owned: false,
    #[cfg(windows)]
    internal_fd: -1,
    #[cfg(windows)]
    internal_last_key: [0u8; 32],
    #[cfg(windows)]
    internal_last_key_len: 0,
    #[cfg(windows)]
    internal_last_key_offset: 0,
    #[cfg(windows)]
    internal_last_repeat_count: 0,
    #[cfg(windows)]
    internal_utf16_high_surrogate: 0,
    #[cfg(windows)]
    internal_wait_handle: std::ptr::null_mut(),
    #[cfg(windows)]
    internal_wait_waker: None,
    #[cfg(windows)]
    internal_line_read_result: None,
    #[cfg(windows)]
    internal_line_read_pending: false,
  }
}

/// Restore original termios for this fd if it was the globally tracked one.
/// Called during handle close.
#[cfg(unix)]
pub(crate) fn restore_termios_on_close(fd: RawFd) {
  global_termios::acquire();
  global_termios::restore_and_clear(fd);
  global_termios::release();
}

// ---- macOS kqueue fallback ----

/// Probe whether kqueue rejects this fd by attempting a kevent.
/// Returns `true` if kqueue returns EINVAL for this fd, meaning
/// we need the select(2) fallback.
#[cfg(target_os = "macos")]
fn kqueue_rejects_fd(fd: RawFd) -> bool {
  unsafe {
    let kq = libc::kqueue();
    if kq == -1 {
      return false;
    }

    let mut changelist: libc::kevent = std::mem::zeroed();
    changelist.ident = fd as usize;
    changelist.filter = libc::EVFILT_READ;
    changelist.flags = libc::EV_ADD | libc::EV_ENABLE;

    let mut events: libc::kevent = std::mem::zeroed();
    let timeout = libc::timespec {
      tv_sec: 0,
      tv_nsec: 1, // 1ns — just enough to capture errors
    };

    let ret = loop {
      let r = libc::kevent(kq, &changelist, 1, &mut events, 1, &timeout);
      if r != -1 || *errno_location_macos() != libc::EINTR {
        break r;
      }
    };

    libc::close(kq);

    if ret == -1 {
      return false;
    }

    // kevent returns the event with EV_ERROR set and `data` = errno
    // when the fd is incompatible.
    ret > 0
      && (events.flags & libc::EV_ERROR) != 0
      && events.data as c_int == libc::EINVAL
  }
}

#[cfg(target_os = "macos")]
fn errno_location_macos() -> *mut c_int {
  unsafe extern "C" {
    fn __error() -> *mut c_int;
  }
  unsafe { __error() }
}

/// Set up the select(2) fallback for a kqueue-incompatible fd.
/// Creates a socketpair, wraps one end in AsyncFd, and spawns a
/// background thread that polls the real fd with select(2).
///
/// Returns the SelectFallbackState on success or a negative uv error
/// code on failure.
///
/// # Safety
/// `fd` must be a valid, open file descriptor in non-blocking mode.
#[cfg(target_os = "macos")]
fn setup_select_fallback(fd: RawFd) -> Result<SelectFallbackState, c_int> {
  use std::sync::Arc;
  use std::sync::atomic::AtomicBool;

  let mut fds = [0 as c_int; 2];
  if unsafe {
    libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr())
  } != 0
  {
    return Err(unsafe { -*errno_location_macos() });
  }
  let fake_fd = fds[0]; // we poll this end (kqueue-compatible)
  let int_fd = fds[1]; // select thread reads this for interrupts

  // select(2) uses fixed-size fd_set bitmaps (FD_SETSIZE, typically 1024).
  // FD_SET on an fd >= FD_SETSIZE is undefined behavior (out-of-bounds write).
  let max_fd = fd.max(int_fd);
  if max_fd >= libc::FD_SETSIZE as RawFd {
    unsafe {
      libc::close(fake_fd);
      libc::close(int_fd);
    }
    return Err(UV_EINVAL);
  }

  // Set both ends non-blocking.
  for &sock_fd in &[fake_fd, int_fd] {
    let flags = unsafe { libc::fcntl(sock_fd, libc::F_GETFL) };
    if flags == -1
      || unsafe {
        libc::fcntl(sock_fd, libc::F_SETFL, flags | libc::O_NONBLOCK)
      } == -1
    {
      unsafe {
        libc::close(fake_fd);
        libc::close(int_fd);
      }
      return Err(UV_EINVAL);
    }
  }

  // Wrap fake_fd in AsyncFd — socketpairs work fine with kqueue.
  let async_fd = match AsyncFd::new(TtyFd(fake_fd)) {
    Ok(afd) => afd,
    Err(e) => {
      unsafe {
        libc::close(fake_fd);
        libc::close(int_fd);
      }
      return Err(super::io_error_to_uv(&e));
    }
  };

  let close_flag = Arc::new(AtomicBool::new(false));
  let close_flag2 = close_flag.clone();
  let interest = Arc::new(std::sync::atomic::AtomicU8::new(0));
  let interest2 = interest.clone();
  let async_sem = Arc::new(Semaphore::new(0));
  let async_sem2 = async_sem.clone();

  let max_fd = fd.max(int_fd);

  let thread = match std::thread::Builder::new()
    .name("tty-select".into())
    .spawn(move || {
      select_thread_main(
        fd,
        int_fd,
        max_fd,
        close_flag2,
        interest2,
        async_sem2,
      );
    }) {
    Ok(t) => t,
    Err(_) => {
      // Drop async_fd first to deregister from kqueue before closing
      // the raw fd — avoids fd-reuse races.
      drop(async_fd);
      unsafe {
        libc::close(fake_fd);
        libc::close(int_fd);
      }
      return Err(UV_EINVAL);
    }
  };

  Ok(SelectFallbackState {
    async_fd: Some(async_fd),
    fake_fd,
    int_fd,
    thread: Some(thread),
    close: close_flag,
    interest,
    async_sem,
  })
}

/// Background thread that polls the real TTY fd with select(2) and
/// signals readiness by writing a byte to the socketpair's int_fd
/// end, which makes data readable on fake_fd (the AsyncFd side).
///
/// This matches libuv's `uv__stream_osx_select` thread function.
/// Like libuv, we only watch for directions that the stream is
/// actively interested in (read/write), controlled via the shared
/// `interest` atomic.
#[cfg(target_os = "macos")]
fn select_thread_main(
  fd: RawFd,
  int_fd: RawFd,
  max_fd: RawFd,
  close: std::sync::Arc<std::sync::atomic::AtomicBool>,
  interest: std::sync::Arc<std::sync::atomic::AtomicU8>,
  async_sem: std::sync::Arc<Semaphore>,
) {
  use std::sync::atomic::Ordering;

  loop {
    if close.load(Ordering::Acquire) {
      break;
    }

    unsafe {
      // Check what the main thread is interested in.
      let cur_interest = interest.load(Ordering::Acquire);

      // Build fd_sets for select, only watching active directions.
      let mut read_set: libc::fd_set = std::mem::zeroed();
      let mut write_set: libc::fd_set = std::mem::zeroed();

      libc::FD_ZERO(&mut read_set);
      libc::FD_ZERO(&mut write_set);

      if cur_interest & SELECT_INTEREST_READ != 0 {
        libc::FD_SET(fd, &mut read_set);
      }
      if cur_interest & SELECT_INTEREST_WRITE != 0 {
        libc::FD_SET(fd, &mut write_set);
      }
      // Always watch the interrupt fd for shutdown / interest changes.
      libc::FD_SET(int_fd, &mut read_set);

      // Block indefinitely like libuv — the interrupt fd wakes us
      // for shutdown or interest changes. No timeout needed.
      let ret = libc::select(
        max_fd + 1,
        &mut read_set,
        &mut write_set,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
      );

      if ret == -1 {
        let e = *errno_location_macos();
        if e == libc::EINTR {
          continue;
        }
        // Unexpected error — bail out (libuv calls abort() here).
        break;
      }

      if ret == 0 {
        continue;
      }

      // Drain interrupt fd if it was signalled.
      if libc::FD_ISSET(int_fd, &read_set) {
        drain_fd(int_fd);
        if close.load(Ordering::Acquire) {
          break;
        }
      }

      // Signal readiness by writing a byte to int_fd. In a
      // socketpair, writing to int_fd (fds[1]) makes data readable
      // on fake_fd (fds[0]), which is wrapped in AsyncFd.
      let has_read = libc::FD_ISSET(fd, &read_set);
      let has_write = libc::FD_ISSET(fd, &write_set);
      if has_read || has_write {
        let _ = libc::write(int_fd, b"r".as_ptr().cast(), 1);

        // Wait for the main thread to process the events before
        // looping again. This prevents busy-looping when the fd is
        // continuously ready (e.g., writable). Matches libuv's
        // `uv_sem_wait(&s->async_sem)` in `uv__stream_osx_select`.
        async_sem.wait();
      }
    }
  }
}

/// Update the select fallback thread's interest flags based on the
/// current TTY state (reading, pending writes). Also interrupts the
/// select thread so it re-evaluates its fd_sets.
///
/// Matches libuv's `uv__stream_osx_interrupt_select` which is called
/// whenever IO interest changes.
///
/// # Safety
/// `tty` must be a valid pointer to an initialized `uv_tty_t`.
#[cfg(target_os = "macos")]
unsafe fn update_select_interest(tty: *mut uv_tty_t) {
  use std::sync::atomic::Ordering;

  unsafe {
    if let Some(TtyReactor::SelectFallback(s)) =
      (*tty).internal_reactor.as_ref()
    {
      let mut flags: u8 = 0;
      if (*tty).internal_reading {
        flags |= SELECT_INTEREST_READ;
      }
      if !(*tty).internal_write_queue.is_empty()
        || (*tty).internal_shutdown.is_some()
      {
        flags |= SELECT_INTEREST_WRITE;
      }
      s.interest.store(flags, Ordering::Release);
      // Interrupt the select thread so it picks up the new interest.
      // Writing to fake_fd makes data readable on int_fd.
      let _ = libc::write(s.fake_fd, b"i".as_ptr().cast(), 1);
    }
  }
}

/// Drain all pending bytes from a non-blocking fd.
/// Handles EINTR by retrying, matching libuv's drain loop in
/// `uv__stream_osx_select`.
#[cfg(target_os = "macos")]
fn drain_fd(fd: RawFd) -> bool {
  unsafe {
    let mut drained_any = false;
    let mut buf = [0u8; 1024];
    loop {
      let r = libc::read(fd, buf.as_mut_ptr().cast(), buf.len());
      if r == buf.len() as isize {
        // Buffer was full — there may be more data.
        drained_any = true;
        continue;
      }
      if r > 0 {
        // Read less than buffer size — done.
        drained_any = true;
        break;
      }
      if r == 0 {
        break;
      }
      // r == -1: check errno.
      let e = *errno_location_macos();
      if e == libc::EINTR {
        continue;
      }
      // EAGAIN/EWOULDBLOCK or other error — done.
      break;
    }
    drained_any
  }
}

/// Shut down the select fallback thread and close the socketpair.
/// Called during handle close.
#[cfg(target_os = "macos")]
pub(crate) fn shutdown_select_fallback(s: &mut SelectFallbackState) {
  // Signal the thread to stop. Matches libuv's
  // `uv_sem_post(&s->close_sem)`.
  s.close.store(true, std::sync::atomic::Ordering::Release);

  // Post the async semaphore in case the select thread is blocked
  // waiting for the main thread to process events. Matches libuv's
  // `uv_sem_post(&s->async_sem)` during close.
  s.async_sem.post();

  // Write to fake_fd to interrupt the select() call via int_fd.
  // Matches libuv's `uv__stream_osx_interrupt_select(handle)`.
  unsafe {
    let _ = libc::write(s.fake_fd, b"x".as_ptr().cast(), 1);
  }

  // Join the thread.
  if let Some(thread) = s.thread.take() {
    let _ = thread.join();
  }

  // Close the socketpair fds. We must deregister the AsyncFd from
  // kqueue BEFORE closing the raw fd, to avoid an fd-reuse race
  // (another thread could reuse the fd number between close and
  // deregistration).
  //
  // Take the AsyncFd out and drop it to deregister from kqueue.
  // TtyFd doesn't impl Drop, so dropping the AsyncFd only
  // deregisters — it doesn't close the fd.
  drop(s.async_fd.take());
  // Now safe to close the raw fds.
  unsafe {
    libc::close(s.fake_fd);
    libc::close(s.int_fd);
  }
}

// ---- Helpers ----

/// Determine whether `fd` is a PTY slave (as opposed to a PTY master).
/// This matters for `uv_tty_init` because reopening a master won't work
/// (*BSD opens in slave mode, Linux allocates a new pair).
#[cfg(unix)]
unsafe fn tty_is_slave(fd: c_int) -> bool {
  #[cfg(target_os = "linux")]
  {
    // TIOCGPTN returns the slave number; fails on the slave side.
    let mut dummy: c_int = 0;
    unsafe { libc::ioctl(fd, libc::TIOCGPTN as _, &mut dummy) != 0 }
  }
  #[cfg(target_os = "macos")]
  {
    // TIOCPTYGNAME returns the slave device name; fails on the slave side.
    // Value from <sys/ttycom.h>: _IOC(IOC_OUT, 'N', 1, 128) = 0x40804E81
    const TIOCPTYGNAME: u64 = 0x40804E81;
    let mut dummy = [0u8; 256];
    unsafe { libc::ioctl(fd, TIOCPTYGNAME as _, dummy.as_mut_ptr()) != 0 }
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos")))]
  {
    // Fallback: ptsname() returns NULL for slave fds.
    unsafe { libc::ptsname(fd).is_null() }
  }
}

/// Open a file with O_CLOEXEC set.
#[cfg(unix)]
unsafe fn open_cloexec(path: *const libc::c_char, flags: c_int) -> c_int {
  unsafe { libc::open(path, flags | libc::O_CLOEXEC) }
}

/// Retry-on-EINTR wrapper for tcsetattr.
#[cfg(unix)]
fn tcsetattr_eintr(fd: c_int, action: c_int, termios: &libc::termios) -> c_int {
  loop {
    let rc = unsafe { libc::tcsetattr(fd, action, termios) };
    if rc == 0 {
      return 0;
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() != Some(libc::EINTR) {
      return -err.raw_os_error().unwrap_or(libc::EINVAL);
    }
  }
}

/// Collect multiple `uv_buf_t` entries into a single `Vec<u8>`.
///
/// ### Safety
/// `bufs` must point to `nbufs` valid `uv_buf_t` entries.
unsafe fn collect_bufs(bufs: *const uv_buf_t, nbufs: u32) -> Vec<u8> {
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
