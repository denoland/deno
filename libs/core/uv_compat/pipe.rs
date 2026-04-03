// Copyright 2018-2026 the Deno authors. MIT license.

//! Pipe handle implementation for uv_compat.
//!
//! `uv_pipe_open(fd)` wraps an existing OS file descriptor for async I/O
//! through the event loop. On Unix, uses `tokio::io::AsyncFd` for
//! non-blocking reads/writes. Closing the handle cancels pending I/O
//! and closes the fd.

use std::collections::VecDeque;
use std::ffi::c_int;
use std::ffi::c_void;
#[cfg(unix)]
use std::os::unix::io::RawFd;
#[cfg(unix)]
use std::task::Context;

#[cfg(unix)]
use super::UV_EBADF;
#[cfg(unix)]
use super::UV_EOF;
use super::UV_HANDLE_ACTIVE;
#[cfg(unix)]
use super::UV_HANDLE_CLOSING;
use super::stream::uv_alloc_cb;
#[cfg(unix)]
use super::stream::uv_buf_t;
use super::stream::uv_read_cb;
#[cfg(unix)]
use super::stream::uv_stream_t;
use super::tcp::WritePending;
#[cfg(unix)]
use super::tcp::io_error_to_uv;
#[cfg(unix)]
use super::uv_handle_t;
use super::uv_handle_type;
use super::uv_loop_t;

/// Pipe handle, analogous to libuv's `uv_pipe_t`.
///
/// Currently supports `uv_pipe_open(fd)` for wrapping raw fds.
#[repr(C)]
pub struct uv_pipe_t {
  pub r#type: uv_handle_type,
  pub loop_: *mut uv_loop_t,
  pub data: *mut c_void,
  pub flags: u32,

  #[cfg(unix)]
  pub(crate) internal_fd: Option<RawFd>,
  #[cfg(unix)]
  pub(crate) internal_async_fd: Option<tokio::io::unix::AsyncFd<RawFdWrapper>>,

  pub(crate) internal_alloc_cb: Option<uv_alloc_cb>,
  pub(crate) internal_read_cb: Option<uv_read_cb>,
  pub(crate) internal_reading: bool,
  pub(crate) internal_write_queue: VecDeque<WritePending>,
  pub(crate) ipc: bool,
}

/// Wrapper to implement `AsRawFd` for `AsyncFd`.
#[cfg(unix)]
pub struct RawFdWrapper(pub RawFd);

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for RawFdWrapper {
  fn as_raw_fd(&self) -> RawFd {
    self.0
  }
}

pub fn new_pipe(ipc: bool) -> uv_pipe_t {
  uv_pipe_t {
    r#type: uv_handle_type::UV_NAMED_PIPE,
    loop_: std::ptr::null_mut(),
    data: std::ptr::null_mut(),
    flags: 0,
    #[cfg(unix)]
    internal_fd: None,
    #[cfg(unix)]
    internal_async_fd: None,
    internal_alloc_cb: None,
    internal_read_cb: None,
    internal_reading: false,
    internal_write_queue: VecDeque::new(),
    ipc,
  }
}

/// Initialize a pipe handle.
///
/// # Safety
/// `loop_` and `pipe` must be valid pointers.
pub unsafe fn uv_pipe_init(
  loop_: *mut uv_loop_t,
  pipe: *mut uv_pipe_t,
  ipc: c_int,
) -> c_int {
  unsafe {
    *pipe = new_pipe(ipc != 0);
    (*pipe).loop_ = loop_;
  }
  0
}

/// Open an existing fd as a pipe handle.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
/// `fd` must be a valid OS file descriptor. Ownership of the fd transfers
/// to the pipe handle -- closing the handle closes the fd.
#[cfg(unix)]
pub unsafe fn uv_pipe_open(pipe: *mut uv_pipe_t, fd: c_int) -> c_int {
  if fd < 0 {
    return UV_EBADF;
  }

  // Set non-blocking mode.
  let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
  if flags == -1 {
    return UV_EBADF;
  }
  if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
    return UV_EBADF;
  }

  // Wrap in AsyncFd for event loop integration.
  let async_fd = match tokio::io::unix::AsyncFd::new(RawFdWrapper(fd)) {
    Ok(afd) => afd,
    Err(_) => return UV_EBADF,
  };

  unsafe {
    (*pipe).internal_fd = Some(fd);
    (*pipe).internal_async_fd = Some(async_fd);
    (*pipe).flags |= UV_HANDLE_ACTIVE;
  }
  0
}

/// Close the pipe handle: close the fd and clean up.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
pub(crate) unsafe fn close_pipe(pipe: *mut uv_pipe_t) {
  unsafe {
    (*pipe).internal_reading = false;
    (*pipe).internal_alloc_cb = None;
    (*pipe).internal_read_cb = None;

    #[cfg(unix)]
    {
      // Drop AsyncFd first (deregisters from epoll/kqueue).
      (*pipe).internal_async_fd = None;

      // Close the OS fd.
      if let Some(fd) = (*pipe).internal_fd.take() {
        libc::close(fd);
      }
    }
  }
}

#[cfg(unix)]
pub(crate) unsafe fn read_start_pipe(
  pipe: *mut uv_pipe_t,
  alloc_cb: Option<uv_alloc_cb>,
  read_cb: Option<uv_read_cb>,
) -> c_int {
  unsafe {
    if alloc_cb.is_none() || read_cb.is_none() {
      return super::UV_EINVAL;
    }
    if (*pipe).flags & UV_HANDLE_CLOSING != 0 {
      return super::UV_EINVAL;
    }
    (*pipe).internal_alloc_cb = alloc_cb;
    (*pipe).internal_read_cb = read_cb;
    (*pipe).internal_reading = true;
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
  }
  0
}

#[cfg(unix)]
pub(crate) unsafe fn read_stop_pipe(pipe: *mut uv_pipe_t) -> c_int {
  unsafe {
    (*pipe).internal_reading = false;
    // Don't clear ACTIVE -- the handle is still open, just not reading.
  }
  0
}

/// Poll a pipe handle for read readiness and perform reads.
/// Called from the event loop tick, same pattern as `poll_tcp_handle`.
///
/// # Safety
/// `pipe_ptr` must be a valid pointer to an initialized, active `uv_pipe_t`.
#[cfg(unix)]
pub(crate) unsafe fn poll_pipe_handle(
  pipe_ptr: *mut uv_pipe_t,
  cx: &mut Context<'_>,
) -> bool {
  let mut any_work = false;

  unsafe {
    // Poll writes first.
    while let Some(pw) = (*pipe_ptr).internal_write_queue.front() {
      // If already completed synchronously, just fire the callback.
      if let Some(status) = pw.status {
        any_work = true;
        let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
        if let Some(cb) = pw.cb {
          cb(pw.req, status);
        }
        continue;
      }
      if (*pipe_ptr).internal_fd.is_none() {
        break;
      }
      let fd = (*pipe_ptr).internal_fd.unwrap();
      let remaining = &pw.data[pw.offset..];
      match libc::write(
        fd,
        remaining.as_ptr() as *const c_void,
        remaining.len(),
      ) {
        n if n >= 0 => {
          any_work = true;
          let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, 0);
          }
        }
        _ => {
          let err = std::io::Error::last_os_error();
          if err.kind() == std::io::ErrorKind::WouldBlock {
            // Register interest for write readiness.
            if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
              let _ = afd.poll_write_ready(cx);
            }
            break;
          } else {
            let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
            if let Some(cb) = pw.cb {
              cb(pw.req, io_error_to_uv(&err));
            }
          }
        }
      }
    }

    // Poll reads.
    if (*pipe_ptr).internal_reading && (*pipe_ptr).internal_async_fd.is_some() {
      let alloc_cb = (*pipe_ptr).internal_alloc_cb;
      let read_cb = (*pipe_ptr).internal_read_cb;

      if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
        // Register read interest.
        if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
          let _ = afd.poll_read_ready(cx);
        }

        let mut count = 32;
        let fd = (*pipe_ptr).internal_fd.unwrap();

        loop {
          if !(*pipe_ptr).internal_reading
            || (*pipe_ptr).internal_async_fd.is_none()
          {
            break;
          }
          let mut buf = uv_buf_t {
            base: std::ptr::null_mut(),
            len: 0,
          };
          alloc_cb(pipe_ptr as *mut uv_handle_t, 65536, &mut buf);
          if buf.base.is_null() || buf.len == 0 {
            read_cb(
              pipe_ptr as *mut uv_stream_t,
              super::UV_ENOBUFS as isize,
              &buf,
            );
            break;
          }

          let nread = libc::read(fd, buf.base as *mut c_void, buf.len);

          if nread > 0 {
            any_work = true;
            let buflen = buf.len;
            read_cb(pipe_ptr as *mut uv_stream_t, nread as isize, &buf);
            count -= 1;
            if count == 0 || (nread as usize) < buflen {
              break;
            }
          } else if nread == 0 {
            // EOF
            read_cb(pipe_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
            (*pipe_ptr).internal_reading = false;
            break;
          } else {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::WouldBlock {
              read_cb(pipe_ptr as *mut uv_stream_t, 0, &buf);
              break;
            } else {
              let status = io_error_to_uv(&err);
              read_cb(pipe_ptr as *mut uv_stream_t, status as isize, &buf);
              (*pipe_ptr).internal_reading = false;
              break;
            }
          }
        }
      }
    }
  }

  any_work
}
