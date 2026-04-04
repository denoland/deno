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
use std::task::Context;

use super::UV_EBADF;
use super::UV_EOF;
use super::UV_HANDLE_ACTIVE;
use super::UV_HANDLE_CLOSING;
use super::stream::uv_alloc_cb;
use super::stream::uv_buf_t;
use super::stream::uv_read_cb;
use super::stream::uv_stream_t;
use super::tcp::WritePending;
use super::tcp::io_error_to_uv;
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

  #[cfg(windows)]
  pub(crate) internal_handle: Option<std::os::windows::io::RawHandle>,
  /// Pending blocking read result from a spawned thread.
  #[cfg(windows)]
  pub(crate) internal_pending_read:
    Option<tokio::task::JoinHandle<std::io::Result<(Vec<u8>, usize)>>>,
  /// Windows named pipe server (waiting for or connected to a client).
  #[cfg(windows)]
  pub(crate) internal_win_server:
    Option<tokio::net::windows::named_pipe::NamedPipeServer>,
  /// Windows named pipe client (connected to a server).
  #[cfg(windows)]
  pub(crate) internal_win_client:
    Option<tokio::net::windows::named_pipe::NamedPipeClient>,
  /// Pending server connect (waiting for client to connect).
  #[cfg(windows)]
  pub(crate) internal_win_server_connecting: bool,

  // Connected stream (from connect or accept)
  #[cfg(unix)]
  pub(crate) internal_stream: Option<tokio::net::UnixStream>,

  // Server listener
  #[cfg(unix)]
  pub(crate) internal_listener: Option<tokio::net::UnixListener>,
  pub(crate) internal_bind_path: Option<String>,

  // Pending connect
  pub(crate) internal_connect: Option<PipeConnectPending>,

  // Connection backlog
  #[cfg(unix)]
  pub(crate) internal_backlog: VecDeque<tokio::net::UnixStream>,
  pub(crate) internal_connection_cb: Option<super::stream::uv_connection_cb>,

  pub(crate) internal_alloc_cb: Option<uv_alloc_cb>,
  pub(crate) internal_read_cb: Option<uv_read_cb>,
  pub(crate) internal_reading: bool,
  pub(crate) internal_write_queue: VecDeque<WritePending>,
  pub(crate) internal_shutdown: Option<super::tcp::ShutdownPending>,
  pub(crate) ipc: bool,
}

/// In-flight pipe connect operation.
pub(crate) struct PipeConnectPending {
  pub req: *mut super::stream::uv_connect_t,
  pub cb: Option<super::stream::uv_connect_cb>,
  #[cfg(unix)]
  pub future: std::pin::Pin<
    Box<
      dyn std::future::Future<Output = std::io::Result<tokio::net::UnixStream>>,
    >,
  >,
}

impl uv_pipe_t {
  /// Get the raw fd if one has been opened on this pipe.
  #[cfg(unix)]
  pub fn fd(&self) -> Option<RawFd> {
    self.internal_fd
  }

  /// Get the bind path if one was set.
  pub fn bind_path(&self) -> Option<&str> {
    self.internal_bind_path.as_deref()
  }
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
    #[cfg(windows)]
    internal_handle: None,
    #[cfg(windows)]
    internal_pending_read: None,
    #[cfg(windows)]
    internal_win_server: None,
    #[cfg(windows)]
    internal_win_client: None,
    #[cfg(windows)]
    internal_win_server_connecting: false,
    #[cfg(unix)]
    internal_stream: None,
    #[cfg(unix)]
    internal_listener: None,
    internal_bind_path: None,
    internal_connect: None,
    #[cfg(unix)]
    internal_backlog: VecDeque::new(),
    internal_connection_cb: None,
    internal_alloc_cb: None,
    internal_read_cb: None,
    internal_reading: false,
    internal_write_queue: VecDeque::new(),
    internal_shutdown: None,
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
    // Match libuv: handles start ref'd so they keep the event loop alive.
    (*pipe).flags = super::UV_HANDLE_REF;
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

/// Open an existing fd as a pipe handle (Windows).
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
/// `fd` must be a valid CRT file descriptor.
#[cfg(windows)]
pub unsafe fn uv_pipe_open(pipe: *mut uv_pipe_t, fd: c_int) -> c_int {
  if fd < 0 {
    return UV_EBADF;
  }

  // Convert CRT fd to OS HANDLE.
  // SAFETY: libc::get_osfhandle returns the OS handle for a CRT fd.
  let handle = unsafe { libc::get_osfhandle(fd) };
  if handle == -1 {
    return UV_EBADF;
  }

  unsafe {
    (*pipe).internal_handle = Some(handle as std::os::windows::io::RawHandle);
    (*pipe).flags |= UV_HANDLE_ACTIVE;
  }
  0
}

/// Bind to a Unix domain socket path.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
pub unsafe fn uv_pipe_bind(pipe: *mut uv_pipe_t, path: &str) -> c_int {
  unsafe {
    (*pipe).internal_bind_path = Some(path.to_string());
  }
  0
}

/// Start listening for connections on a bound pipe.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized, bound `uv_pipe_t`.
#[cfg(unix)]
pub unsafe fn uv_pipe_listen(
  pipe: *mut uv_pipe_t,
  _backlog: c_int,
  cb: Option<super::stream::uv_connection_cb>,
) -> c_int {
  unsafe {
    let path = match &(*pipe).internal_bind_path {
      Some(p) => p.clone(),
      None => return super::UV_EINVAL,
    };

    // Remove existing socket file if present (matching libuv behavior).
    let _ = std::fs::remove_file(&path);

    let listener = match tokio::net::UnixListener::bind(&path) {
      Ok(l) => l,
      Err(e) => return io_error_to_uv(&e),
    };

    (*pipe).internal_listener = Some(listener);
    (*pipe).internal_connection_cb = cb;
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
  }
  0
}

/// Accept a pending connection from the backlog into a client pipe handle.
///
/// # Safety
/// `server` and `client` must be valid pointers to initialized `uv_pipe_t`.
#[cfg(unix)]
pub unsafe fn uv_pipe_accept(
  server: *mut uv_pipe_t,
  client: *mut uv_pipe_t,
) -> c_int {
  unsafe {
    if let Some(stream) = (*server).internal_backlog.pop_front() {
      use std::os::unix::io::AsRawFd;
      let fd = stream.as_raw_fd();
      (*client).internal_fd = Some(fd);
      (*client).internal_stream = Some(stream);
      (*client).flags |= UV_HANDLE_ACTIVE;
      // Add to pipe_handles so writes/reads are polled.
      // Note: this may be called from within poll_pipe_handle's callback
      // chain, so pipe_handles might already be borrowed. Use try_borrow_mut.
      (*client).loop_ = (*server).loop_;
      let inner = super::get_inner((*client).loop_);
      if let Ok(mut handles) = inner.pipe_handles.try_borrow_mut() {
        if !handles.iter().any(|&h| std::ptr::eq(h, client)) {
          handles.push(client);
        }
      }
      0
    } else {
      super::UV_EAGAIN
    }
  }
}

/// Start an async connect to a Unix domain socket path.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
/// `req` must be a valid pointer to a `uv_connect_t`.
#[cfg(unix)]
pub unsafe fn uv_pipe_connect(
  req: *mut super::stream::uv_connect_t,
  pipe: *mut uv_pipe_t,
  path: &str,
  cb: Option<super::stream::uv_connect_cb>,
) -> c_int {
  let path = path.to_string();
  let future =
    Box::pin(async move { tokio::net::UnixStream::connect(&path).await });

  unsafe {
    (*pipe).internal_connect = Some(PipeConnectPending { req, cb, future });
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
  }
  0
}

/// Start listening for connections on a Windows named pipe.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized, bound `uv_pipe_t`.
#[cfg(windows)]
pub unsafe fn uv_pipe_listen(
  pipe: *mut uv_pipe_t,
  _backlog: c_int,
  cb: Option<super::stream::uv_connection_cb>,
) -> c_int {
  unsafe {
    let path = match &(*pipe).internal_bind_path {
      Some(p) => p.clone(),
      None => return super::UV_EINVAL,
    };

    let mut opts = tokio::net::windows::named_pipe::ServerOptions::new();
    opts.first_pipe_instance(true);
    let server = match opts.create(&path) {
      Ok(s) => s,
      Err(e) => return io_error_to_uv(&e),
    };

    (*pipe).internal_win_server = Some(server);
    (*pipe).internal_win_server_connecting = true;
    (*pipe).internal_connection_cb = cb;
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
  }
  0
}

/// Accept a pending connection on Windows. Moves the connected server
/// to the client handle, creates a new server for the next connection.
///
/// # Safety
/// `server` and `client` must be valid pointers to initialized `uv_pipe_t`.
#[cfg(windows)]
pub unsafe fn uv_pipe_accept(
  server: *mut uv_pipe_t,
  client: *mut uv_pipe_t,
) -> c_int {
  unsafe {
    // Move the connected server pipe to the client handle.
    if let Some(connected_server) = (*server).internal_win_server.take() {
      (*client).internal_win_server = Some(connected_server);
      (*client).flags |= UV_HANDLE_ACTIVE;

      // Add client to pipe_handles for polling.
      (*client).loop_ = (*server).loop_;
      let inner = super::get_inner((*client).loop_);
      if let Ok(mut handles) = inner.pipe_handles.try_borrow_mut() {
        if !handles.iter().any(|&h| std::ptr::eq(h, client)) {
          handles.push(client);
        }
      }

      // Create a new server instance for the next connection.
      if let Some(ref path) = (*server).internal_bind_path {
        let mut opts = tokio::net::windows::named_pipe::ServerOptions::new();
        opts.first_pipe_instance(false);
        if let Ok(new_server) = opts.create(path.as_str()) {
          (*server).internal_win_server = Some(new_server);
          (*server).internal_win_server_connecting = true;
        }
      }
      0
    } else {
      super::UV_EAGAIN
    }
  }
}

/// Connect to a Windows named pipe server.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
/// `req` must be a valid pointer to a `uv_connect_t`.
#[cfg(windows)]
pub unsafe fn uv_pipe_connect(
  req: *mut super::stream::uv_connect_t,
  pipe: *mut uv_pipe_t,
  path: &str,
  cb: Option<super::stream::uv_connect_cb>,
) -> c_int {
  unsafe {
    let opts = tokio::net::windows::named_pipe::ClientOptions::new();
    match opts.open(path) {
      Ok(client) => {
        (*pipe).internal_win_client = Some(client);
        (*pipe).flags |= UV_HANDLE_ACTIVE;

        // Named pipe client connect is synchronous on Windows.
        // Fire the callback immediately via microtask.
        if !req.is_null() {
          (*req).handle = pipe as *mut super::stream::uv_stream_t;
        }
        if let Some(cb) = cb {
          // Defer callback to next event loop tick.
          (*pipe).internal_connect =
            Some(PipeConnectPending { req, cb: Some(cb) });
        }

        let inner = super::get_inner((*pipe).loop_);
        let mut handles = inner.pipe_handles.borrow_mut();
        if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
          handles.push(pipe);
        }
        0
      }
      Err(e) => io_error_to_uv(&e),
    }
  }
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
    (*pipe).internal_connection_cb = None;
    (*pipe).internal_connect = None;
    (*pipe).internal_bind_path = None;

    // Cancel pending writes.
    while let Some(pw) = (*pipe).internal_write_queue.pop_front() {
      if let Some(cb) = pw.cb {
        cb(pw.req, super::UV_ECANCELED);
      }
    }

    #[cfg(unix)]
    {
      (*pipe).internal_async_fd = None;
      (*pipe).internal_stream = None;
      (*pipe).internal_listener = None;
      (*pipe).internal_backlog.clear();

      if let Some(fd) = (*pipe).internal_fd.take() {
        libc::close(fd);
      }
    }

    #[cfg(windows)]
    {
      if let Some(handle) = (*pipe).internal_pending_read.take() {
        handle.abort();
      }
      if let Some(handle) = (*pipe).internal_handle.take() {
        windows_sys::Win32::Foundation::CloseHandle(handle as _);
      }
      // Drop named pipe server/client (closes handles).
      (*pipe).internal_win_server = None;
      (*pipe).internal_win_client = None;
      (*pipe).internal_win_server_connecting = false;
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
  }
  0
}

#[cfg(windows)]
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

#[cfg(windows)]
pub(crate) unsafe fn read_stop_pipe(pipe: *mut uv_pipe_t) -> c_int {
  unsafe {
    (*pipe).internal_reading = false;
    // Abort pending read if any.
    if let Some(handle) = (*pipe).internal_pending_read.take() {
      handle.abort();
    }
  }
  0
}

/// Windows: spawn a blocking read on a thread if one isn't already pending.
#[cfg(windows)]
unsafe fn maybe_spawn_win_read(pipe_ptr: *mut uv_pipe_t) {
  unsafe {
    if (*pipe_ptr).internal_pending_read.is_some() {
      return; // already reading
    }
    let handle = match (*pipe_ptr).internal_handle {
      Some(h) => h as usize, // usize is Send
      None => return,
    };

    let join = deno_core::unsync::spawn_blocking(move || {
      use std::io::Read;
      // SAFETY: handle is a valid OS handle stored by uv_pipe_open.
      // We reconstruct a File temporarily, read from it, then leak it
      // back to avoid double-close.
      let mut file = unsafe {
        std::fs::File::from_raw_handle(
          handle as std::os::windows::io::RawHandle,
        )
      };
      let mut buf = vec![0u8; 65536];
      let result = file.read(&mut buf);
      // Don't drop the File -- the handle is owned by uv_pipe_t.
      let _ = std::os::windows::io::IntoRawHandle::into_raw_handle(file);
      result.map(|n| (buf, n))
    });
    (*pipe_ptr).internal_pending_read = Some(join);
  }
}

/// Poll a Windows pipe handle for connects, accepts, reads, and writes.
#[cfg(windows)]
pub(crate) unsafe fn poll_pipe_handle(
  pipe_ptr: *mut uv_pipe_t,
  cx: &mut Context<'_>,
) -> bool {
  use std::task::Poll;
  let mut any_work = false;

  unsafe {
    // 1. Poll deferred connect callback (from uv_pipe_connect on Windows).
    if let Some(pending) = (*pipe_ptr).internal_connect.take() {
      if let Some(cb) = pending.cb {
        cb(pending.req, 0);
      }
      any_work = true;
    }

    // 2. Poll server waiting for client connection.
    if (*pipe_ptr).internal_win_server_connecting {
      if let Some(ref server) = (*pipe_ptr).internal_win_server {
        match server.poll_connect(cx) {
          Poll::Ready(Ok(())) => {
            (*pipe_ptr).internal_win_server_connecting = false;
            if let Some(cb) = (*pipe_ptr).internal_connection_cb {
              cb(pipe_ptr as *mut uv_stream_t, 0);
            }
            any_work = true;
          }
          Poll::Ready(Err(ref e)) => {
            (*pipe_ptr).internal_win_server_connecting = false;
            if let Some(cb) = (*pipe_ptr).internal_connection_cb {
              cb(pipe_ptr as *mut uv_stream_t, io_error_to_uv(e));
            }
            any_work = true;
          }
          Poll::Pending => {}
        }
      }
    }

    // 3. Poll writes.
    while let Some(pw) = (*pipe_ptr).internal_write_queue.front() {
      if let Some(status) = pw.status {
        any_work = true;
        let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
        if let Some(cb) = pw.cb {
          cb(pw.req, status);
        }
        continue;
      }
      // Try writing to whichever pipe type we have.
      let remaining = &pw.data[pw.offset..];
      use std::io::Write;
      let write_result = if let Some(ref handle) = (*pipe_ptr).internal_handle {
        let mut file = std::fs::File::from_raw_handle(*handle);
        let r = file.write(remaining);
        let _ = std::os::windows::io::IntoRawHandle::into_raw_handle(file);
        r
      } else if let Some(ref server) = (*pipe_ptr).internal_win_server {
        server.try_write(remaining)
      } else if let Some(ref client) = (*pipe_ptr).internal_win_client {
        client.try_write(remaining)
      } else {
        break;
      };
      match write_result {
        Ok(_n) => {
          any_work = true;
          let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, 0);
          }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          break;
        }
        Err(ref e) => {
          let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, io_error_to_uv(e));
          }
        }
      }
    }

    // 4. Poll reads.
    if (*pipe_ptr).internal_reading {
      // For raw handle (uv_pipe_open path), use blocking read thread.
      if (*pipe_ptr).internal_handle.is_some() {
        maybe_spawn_win_read(pipe_ptr);
        if let Some(ref mut join) = (*pipe_ptr).internal_pending_read {
          if let Poll::Ready(result) = std::pin::Pin::new(join).poll(cx) {
            (*pipe_ptr).internal_pending_read = None;
            let alloc_cb = (*pipe_ptr).internal_alloc_cb;
            let read_cb = (*pipe_ptr).internal_read_cb;
            if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
              let mut buf = uv_buf_t {
                base: std::ptr::null_mut(),
                len: 0,
              };
              alloc_cb(pipe_ptr as *mut uv_handle_t, 65536, &mut buf);
              match result {
                Ok(Ok((data, n))) if n > 0 => {
                  any_work = true;
                  if !buf.base.is_null() && buf.len > 0 {
                    let copy_len = n.min(buf.len);
                    std::ptr::copy_nonoverlapping(
                      data.as_ptr(),
                      buf.base as *mut u8,
                      copy_len,
                    );
                    read_cb(
                      pipe_ptr as *mut uv_stream_t,
                      copy_len as isize,
                      &buf,
                    );
                  }
                  if (*pipe_ptr).internal_reading {
                    maybe_spawn_win_read(pipe_ptr);
                  }
                }
                Ok(Ok((_, 0))) => {
                  read_cb(pipe_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
                  (*pipe_ptr).internal_reading = false;
                }
                Ok(Err(ref e)) => {
                  read_cb(
                    pipe_ptr as *mut uv_stream_t,
                    io_error_to_uv(e) as isize,
                    &buf,
                  );
                  (*pipe_ptr).internal_reading = false;
                }
                Err(_) => {
                  read_cb(
                    pipe_ptr as *mut uv_stream_t,
                    super::UV_ECANCELED as isize,
                    &buf,
                  );
                  (*pipe_ptr).internal_reading = false;
                }
              }
            }
          }
        }
      }

      // For named pipe server/client, use try_read.
      let has_named_pipe = (*pipe_ptr).internal_win_server.is_some()
        || (*pipe_ptr).internal_win_client.is_some();
      if has_named_pipe {
        // Register read interest.
        if let Some(ref server) = (*pipe_ptr).internal_win_server {
          let _ = server.poll_read_ready(cx);
        }
        if let Some(ref client) = (*pipe_ptr).internal_win_client {
          let _ = client.poll_read_ready(cx);
        }

        let alloc_cb = (*pipe_ptr).internal_alloc_cb;
        let read_cb = (*pipe_ptr).internal_read_cb;
        if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
          let mut buf = uv_buf_t {
            base: std::ptr::null_mut(),
            len: 0,
          };
          alloc_cb(pipe_ptr as *mut uv_handle_t, 65536, &mut buf);
          if !buf.base.is_null() && buf.len > 0 {
            let slice =
              std::slice::from_raw_parts_mut(buf.base.cast::<u8>(), buf.len);
            let read_result =
              if let Some(ref server) = (*pipe_ptr).internal_win_server {
                server.try_read(slice)
              } else if let Some(ref client) = (*pipe_ptr).internal_win_client {
                client.try_read(slice)
              } else {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
              };
            match read_result {
              Ok(0) => {
                read_cb(pipe_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
                (*pipe_ptr).internal_reading = false;
              }
              Ok(n) => {
                any_work = true;
                read_cb(pipe_ptr as *mut uv_stream_t, n as isize, &buf);
              }
              Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                read_cb(pipe_ptr as *mut uv_stream_t, 0, &buf);
              }
              Err(ref e) => {
                read_cb(
                  pipe_ptr as *mut uv_stream_t,
                  io_error_to_uv(e) as isize,
                  &buf,
                );
                (*pipe_ptr).internal_reading = false;
              }
            }
          }
        }
      }
    }

    // 5. Process deferred shutdown.
    if (*pipe_ptr).internal_write_queue.is_empty()
      && (*pipe_ptr).internal_shutdown.is_some()
    {
      let pending = (*pipe_ptr).internal_shutdown.take().unwrap();
      // Named pipe shutdown: just drop the write side.
      // The reader will see EOF.
      if let Some(cb) = pending.cb {
        cb(pending.req, 0);
      }
      any_work = true;
    }
  }

  any_work
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
  use std::task::Poll;
  let mut any_work = false;

  unsafe {
    // 1. Poll pending connect.
    let connect_result =
      if let Some(ref mut pending) = (*pipe_ptr).internal_connect {
        match pending.future.as_mut().poll(cx) {
          Poll::Ready(result) => Some((pending.req, pending.cb, result)),
          Poll::Pending => None,
        }
      } else {
        None
      };
    if let Some((req, cb, result)) = connect_result {
      let status = match result {
        Ok(stream) => {
          use std::os::unix::io::AsRawFd;
          (*pipe_ptr).internal_fd = Some(stream.as_raw_fd());
          (*pipe_ptr).internal_stream = Some(stream);
          0
        }
        Err(ref e) => io_error_to_uv(e),
      };
      (*pipe_ptr).internal_connect = None;
      (*req).handle = pipe_ptr as *mut uv_stream_t;
      if let Some(cb) = cb {
        cb(req, status);
      }
      any_work = true;
    }

    // 2. Poll listener for new connections.
    let has_cb = (*pipe_ptr).internal_connection_cb.is_some();
    if (*pipe_ptr).internal_listener.is_some()
      && has_cb
      && (*pipe_ptr).internal_backlog.is_empty()
    {
      let listener = (*pipe_ptr).internal_listener.as_ref().unwrap();
      while let Poll::Ready(result) = listener.poll_accept(cx) {
        match result {
          Ok((stream, _)) => {
            (*pipe_ptr).internal_backlog.push_back(stream);
            any_work = true;
          }
          Err(_e) => {
            break;
          }
        }
      }
    }
    while !(*pipe_ptr).internal_backlog.is_empty() {
      if let Some(cb) = (*pipe_ptr).internal_connection_cb {
        let backlog_len = (*pipe_ptr).internal_backlog.len();
        cb(pipe_ptr as *mut uv_stream_t, 0);
        // If callback didn't accept (backlog didn't shrink), stop.
        if (*pipe_ptr).internal_backlog.len() >= backlog_len {
          break;
        }
      } else {
        break;
      }
    }

    // 3. Poll writes.
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
      let remaining = &pw.data[pw.offset..];
      // Use libc::write directly. This bypasses tokio's readiness
      // tracking but works reliably for all fd types.
      let write_result = if let Some(fd) = (*pipe_ptr).internal_fd {
        let n =
          libc::write(fd, remaining.as_ptr() as *const c_void, remaining.len());
        if n >= 0 {
          Ok(n as usize)
        } else {
          Err(std::io::Error::last_os_error())
        }
      } else {
        break;
      };
      match write_result {
        Ok(_n) => {
          any_work = true;
          let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, 0);
          }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          // Register interest for write readiness.
          if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
            let _ = afd.poll_write_ready(cx);
          }
          if let Some(ref stream) = (*pipe_ptr).internal_stream {
            let _ = stream.poll_write_ready(cx);
          }
          break;
        }
        Err(ref e) => {
          let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
          if let Some(cb) = pw.cb {
            cb(pw.req, io_error_to_uv(e));
          }
        }
      }
    }

    // 4. Poll reads.
    // Reads can come from either internal_async_fd (uv_pipe_open) or
    // internal_stream (uv_pipe_connect / uv_pipe_accept).
    let can_read = (*pipe_ptr).internal_reading
      && ((*pipe_ptr).internal_async_fd.is_some()
        || (*pipe_ptr).internal_stream.is_some());
    if can_read {
      let alloc_cb = (*pipe_ptr).internal_alloc_cb;
      let read_cb = (*pipe_ptr).internal_read_cb;

      if let (Some(alloc_cb), Some(read_cb)) = (alloc_cb, read_cb) {
        // Register read interest.
        if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
          let _ = afd.poll_read_ready(cx);
        }
        if let Some(ref stream) = (*pipe_ptr).internal_stream {
          let _ = stream.poll_read_ready(cx);
        }

        let mut count = 32;

        loop {
          if !(*pipe_ptr).internal_reading {
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

          let slice =
            std::slice::from_raw_parts_mut(buf.base.cast::<u8>(), buf.len);
          // Use try_read on UnixStream if available (connect/accept path),
          // fall back to libc::read for opened fds (uv_pipe_open path).
          let read_result =
            if let Some(ref stream) = (*pipe_ptr).internal_stream {
              stream.try_read(slice)
            } else if let Some(fd) = (*pipe_ptr).internal_fd {
              let n = libc::read(fd, buf.base as *mut c_void, buf.len);
              if n >= 0 {
                Ok(n as usize)
              } else {
                Err(std::io::Error::last_os_error())
              }
            } else {
              break;
            };
          match read_result {
            Ok(0) => {
              // EOF
              read_cb(pipe_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
              (*pipe_ptr).internal_reading = false;
              break;
            }
            Ok(n) => {
              any_work = true;
              let buflen = buf.len;
              read_cb(pipe_ptr as *mut uv_stream_t, n as isize, &buf);
              count -= 1;
              if count == 0 || n < buflen {
                break;
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
              // Not ready yet; callback with nread=0 so alloc buf is freed.
              read_cb(pipe_ptr as *mut uv_stream_t, 0, &buf);
              break;
            }
            Err(ref e) => {
              let status = io_error_to_uv(e);
              read_cb(pipe_ptr as *mut uv_stream_t, status as isize, &buf);
              (*pipe_ptr).internal_reading = false;
              break;
            }
          }
        }
      }
    }

    // 5. Process deferred shutdown (after write queue drains).
    if (*pipe_ptr).internal_write_queue.is_empty()
      && (*pipe_ptr).internal_shutdown.is_some()
    {
      let pending = (*pipe_ptr).internal_shutdown.take().unwrap();
      if let Some(ref stream) = (*pipe_ptr).internal_stream {
        use std::os::unix::io::AsRawFd;
        // SAFETY: stream is a valid UnixStream with a valid fd.
        libc::shutdown(stream.as_raw_fd(), libc::SHUT_WR);
      }
      if let Some(cb) = pending.cb {
        cb(pending.req, 0);
      }
      any_work = true;
    }
  }

  any_work
}
