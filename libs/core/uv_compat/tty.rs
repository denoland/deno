// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::VecDeque;
use std::ffi::c_int;
use std::ffi::c_void;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::io::RawFd;
use std::task::Context;

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
#[cfg(unix)]
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
  pub(crate) internal_async_fd: Option<AsyncFd<TtyFd>>,
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
}

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
  #![allow(non_snake_case, non_camel_case_types, dead_code)]

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
  }

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
  if ret == 0 {
    Err(std::io::Error::last_os_error())
  } else {
    Ok(written as usize)
  }
}

#[cfg(windows)]
unsafe fn tty_try_read(
  tty: *const uv_tty_t,
  buf: &mut [u8],
) -> std::io::Result<usize> {
  let handle = unsafe { (*tty).internal_handle };
  let mut read: u32 = 0;
  let ret = unsafe {
    win_console::ReadFile(
      handle,
      buf.as_mut_ptr(),
      buf.len() as u32,
      &mut read,
      std::ptr::null_mut(),
    )
  };
  if ret == 0 {
    Err(std::io::Error::last_os_error())
  } else {
    Ok(read as usize)
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
    let (actual_fd, async_fd) = {
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
      let mut actual_fd = fd;
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
      let async_fd = match AsyncFd::new(TtyFd(actual_fd)) {
        Ok(afd) => afd,
        Err(e) => {
          libc::fcntl(actual_fd, libc::F_SETFL, cur_flags);
          return io_error_to_uv(&e);
        }
      };

      (actual_fd, async_fd)
    };

    #[cfg(windows)]
    let (win_handle, win_readable, win_saved_mode, win_handle_owned) = {
      let raw_handle = win_console::_get_osfhandle(fd);
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
          if fd <= 2 {
            win_console::CloseHandle(handle);
          }
          return UV_EINVAL;
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
      write(addr_of_mut!((*tty).internal_async_fd), Some(async_fd));
      write(addr_of_mut!((*tty).internal_orig_termios), None);
    }

    #[cfg(windows)]
    {
      write(addr_of_mut!((*tty).internal_handle), win_handle);
      write(addr_of_mut!((*tty).internal_readable), win_readable);
      write(addr_of_mut!((*tty).internal_saved_mode), win_saved_mode);
      write(addr_of_mut!((*tty).internal_handle_owned), win_handle_owned);
      write(addr_of_mut!((*tty).internal_fd), fd);
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
        UV_TTY_MODE_NORMAL => (
          win_console::ENABLE_ECHO_INPUT
            | win_console::ENABLE_LINE_INPUT
            | win_console::ENABLE_PROCESSED_INPUT,
          0,
        ),
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
    (*tty).internal_alloc_cb = alloc_cb;
    (*tty).internal_read_cb = read_cb;
    (*tty).internal_reading = true;
    (*tty).flags |= UV_HANDLE_ACTIVE;

    let inner = get_inner((*tty).loop_);
    let mut handles = inner.tty_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tty)) {
      handles.push(tty);
    }
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
      });
      return 0;
    }

    // Fast path: single buffer.
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
                if let Some(cb) = cb {
                  cb(req, 0);
                }
                return 0;
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
              (*tty).internal_write_queue.push_back(WritePending {
                req,
                data: data[offset..].to_vec(),
                offset: 0,
                cb,
              });
              ensure_tty_registered(tty);
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

    // Multi-buffer: collect and write.
    let data = collect_bufs(bufs, nbufs);
    if data.is_empty() {
      if let Some(cb) = cb {
        cb(req, 0);
      }
      return 0;
    }

    let mut offset = 0;
    loop {
      match tty_try_write(tty, &data[offset..]) {
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
          (*tty).internal_write_queue.push_back(WritePending {
            req,
            data: data[offset..].to_vec(),
            offset: 0,
            cb,
          });
          ensure_tty_registered(tty);
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
}

/// Ensure the TTY handle is registered for polling so queued writes
/// (and other deferred work) get drained by the event loop.
///
/// # Safety
/// `tty` must be a valid pointer to an initialized `uv_tty_t`.
unsafe fn ensure_tty_registered(tty: *mut uv_tty_t) {
  unsafe {
    (*tty).flags |= UV_HANDLE_ACTIVE;
    let inner = get_inner((*tty).loop_);
    let mut handles = inner.tty_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, tty)) {
      handles.push(tty);
    }
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
    let tty = stream as *mut uv_tty_t;
    (*req).handle = stream;

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
/// Returns `true` if any work was completed.
///
/// # Safety
/// `tty_ptr` must be a valid pointer to an initialized `uv_tty_t`.
#[allow(unused_variables)]
pub(crate) unsafe fn poll_tty_handle(
  tty_ptr: *mut uv_tty_t,
  cx: &mut Context<'_>,
) -> bool {
  let mut any_work = false;

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
          if let Some(ref async_fd) = (*tty_ptr).internal_async_fd {
            match async_fd.poll_read_ready(cx) {
              std::task::Poll::Ready(Ok(mut guard)) => {
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
                    Err(_) => {
                      read_cb(
                        tty_ptr as *mut uv_stream_t,
                        UV_EOF as isize,
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

        // On non-Unix (Windows), check for available input before reading.
        // TODO: This is still not fully async. libuv uses threaded reads
        // (ReadConsoleW on a worker thread for raw mode,
        // ReadConsoleInputW for line mode). We use
        // GetNumberOfConsoleInputEvents as a readiness gate to avoid
        // blocking the event loop, but a proper implementation should
        // spawn reads onto a blocking thread.
        #[cfg(not(unix))]
        {
          // Check if there are console input events available before
          // attempting a potentially blocking read.
          let mut num_events: u32 = 0;
          let has_input = (*tty_ptr).internal_readable
            && (*tty_ptr).internal_reading
            && win_console::GetNumberOfConsoleInputEvents(
              (*tty_ptr).internal_handle,
              &mut num_events,
            ) != 0
            && num_events > 0;

          while has_input {
            if !(*tty_ptr).internal_reading {
              break;
            }
            let mut buf = uv_buf_t {
              base: std::ptr::null_mut(),
              len: 0,
            };
            alloc_cb(tty_ptr as *mut uv_handle_t, 65536, &mut buf);
            if buf.base.is_null() || buf.len == 0 {
              read_cb(tty_ptr as *mut uv_stream_t, UV_ENOBUFS as isize, &buf);
              break;
            }
            let slice =
              std::slice::from_raw_parts_mut(buf.base.cast::<u8>(), buf.len);
            match tty_try_read(tty_ptr, slice) {
              Ok(0) => {
                read_cb(tty_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
                (*tty_ptr).internal_reading = false;
                break;
              }
              Ok(n) => {
                any_work = true;
                read_cb(tty_ptr as *mut uv_stream_t, n as isize, &buf);
              }
              Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Signal the caller to free the buffer (nread=0).
                read_cb(tty_ptr as *mut uv_stream_t, 0, &buf);
                break;
              }
              Err(_) => {
                read_cb(tty_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
                (*tty_ptr).internal_reading = false;
                break;
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
      if let Some(ref async_fd) = (*tty_ptr).internal_async_fd {
        let _ = async_fd.poll_write_ready(cx);
      }

      loop {
        if (*tty_ptr).internal_write_queue.is_empty() {
          break;
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
      any_work = true;
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
    internal_async_fd: None,
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

/// Duplicate `new_fd` onto `old_fd` with close-on-exec set.
/// Returns 0 on success, negative UV error on failure.
#[cfg(unix)]
unsafe fn dup2_cloexec(new_fd: c_int, old_fd: c_int) -> c_int {
  if new_fd == old_fd {
    return UV_EINVAL;
  }
  // Use dup3 on Linux for atomicity; fall back to dup2 + fcntl elsewhere.
  #[cfg(target_os = "linux")]
  {
    let r =
      unsafe { libc::syscall(libc::SYS_dup3, new_fd, old_fd, libc::O_CLOEXEC) };
    if r == -1 {
      return -std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EINVAL);
    }
    return 0;
  }
  #[cfg(not(target_os = "linux"))]
  {
    let r = unsafe { libc::dup2(new_fd, old_fd) };
    if r == -1 {
      return -std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EINVAL);
    }
    // Set close-on-exec.
    unsafe { libc::fcntl(old_fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    0
  }
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
