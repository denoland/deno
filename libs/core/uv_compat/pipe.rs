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

#[cfg(windows)]
use sys_traits::BaseFsMetadata;
#[cfg(windows)]
use sys_traits::impls::RealSys;

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
  #[allow(
    clippy::type_complexity,
    reason = "JoinHandle<Result<(Vec,usize)>> is inherently complex"
  )]
  pub(crate) internal_pending_read:
    Option<tokio::task::JoinHandle<std::io::Result<(Vec<u8>, usize)>>>,
  /// Windows named pipe server (waiting for or connected to a client).
  /// Wrapped in Arc so the connect future can hold its own reference
  /// without self-referential lifetime issues.
  #[cfg(windows)]
  pub(crate) internal_win_server:
    Option<std::sync::Arc<tokio::net::windows::named_pipe::NamedPipeServer>>,
  /// Windows named pipe client (connected to a server).
  #[cfg(windows)]
  pub(crate) internal_win_client:
    Option<tokio::net::windows::named_pipe::NamedPipeClient>,
  /// Pending `NamedPipeServer::connect()` future. When ready, a client
  /// has connected and the connection callback should fire.
  #[cfg(windows)]
  #[allow(
    clippy::type_complexity,
    reason = "pinned boxed async connect future"
  )]
  pub(crate) internal_win_connect_fut: Option<
    std::pin::Pin<
      Box<dyn std::future::Future<Output = std::io::Result<()>> + Send>,
    >,
  >,

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
  pub(crate) pending_instances: i32,
  pub(crate) ipc: bool,
  pub(crate) internal_waker:
    Option<std::sync::Arc<crate::uv_compat::waker::PipeHandleWaker>>,
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

/// Set the number of pending pipe instances for Windows named pipes.
/// On Unix this is a no-op.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
pub unsafe fn uv_pipe_set_pending_instances(
  pipe: *mut uv_pipe_t,
  instances: i32,
) {
  unsafe {
    (*pipe).pending_instances = instances;
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
    internal_win_connect_fut: None,
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
    pending_instances: 4, // libuv default
    ipc,
    internal_waker: None,
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
    let shared = super::get_inner(loop_).shared.clone();
    (*pipe).internal_waker = Some(
      crate::uv_compat::waker::PipeHandleWaker::new(pipe as usize, shared),
    );
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

  unsafe {
    (*pipe).internal_fd = Some(fd);
    // Create AsyncFd eagerly so the reactor tracks this fd from the
    // start. This avoids edge-triggered readiness races where creating
    // AsyncFd lazily can miss events that fired before registration.
    if let Ok(afd) = tokio::io::unix::AsyncFd::new(RawFdWrapper(fd)) {
      (*pipe).internal_async_fd = Some(afd);
    }
    // Note: do NOT set UV_HANDLE_ACTIVE here. In libuv, uv_pipe_open
    // only associates the fd with the handle; the handle becomes active
    // only when uv_read_start or uv_write is called. Setting ACTIVE here
    // would prevent the event loop from exiting when the pipe is idle
    // (e.g. process.stdout as a pipe with no pending I/O).
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
  use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_PIPE;
  use windows_sys::Win32::Storage::FileSystem::GetFileType;

  if fd < 0 {
    return UV_EBADF;
  }

  // Convert CRT fd to OS HANDLE.
  // SAFETY: libc::get_osfhandle returns the OS handle for a CRT fd.
  let handle = unsafe { libc::get_osfhandle(fd) };
  if handle == -1 {
    return UV_EBADF;
  }

  // If this is a named pipe, wrap it in a tokio NamedPipeClient so reads
  // and writes go through the async reactor. A sync `ReadFile` on a pipe
  // opened with `FILE_FLAG_OVERLAPPED` aborts inside Rust's std when the
  // operation returns `ERROR_IO_PENDING`, which is the common case when
  // no data is immediately available. We cannot know the overlapped flag
  // after the fact, so we fall through to the raw-handle path only for
  // non-pipe handles.
  unsafe {
    let h = handle as *mut std::ffi::c_void;
    if GetFileType(h) == FILE_TYPE_PIPE {
      match tokio::net::windows::named_pipe::NamedPipeClient::from_raw_handle(
        handle as std::os::windows::io::RawHandle,
      ) {
        Ok(client) => {
          (*pipe).internal_win_client = Some(client);
          return 0;
        }
        Err(_) => {
          // Fall through to raw-handle path on failure (e.g. handle not
          // overlapped). NamedPipeClient owns the handle on success; on
          // failure it does not, so it is safe to reuse `handle` below.
        }
      }
    }

    (*pipe).internal_handle = Some(handle as std::os::windows::io::RawHandle);
    // Note: do NOT set UV_HANDLE_ACTIVE here. See Unix uv_pipe_open.
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

    // On Unix, create a socket and bind it now (matching libuv).
    // The socket is NOT listening yet - that happens in uv_pipe_listen.
    // The fd is available immediately for the `fd` property.
    #[cfg(unix)]
    {
      // Create a Unix domain stream socket.
      let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
      if fd < 0 {
        return io_error_to_uv(&std::io::Error::last_os_error());
      }

      // Bind it to the path.
      let c_path = match std::ffi::CString::new(path) {
        Ok(p) => p,
        Err(_) => {
          libc::close(fd);
          return super::UV_EINVAL;
        }
      };
      let mut addr: libc::sockaddr_un = std::mem::zeroed();
      addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
      let path_bytes = c_path.as_bytes_with_nul();
      if path_bytes.len() > addr.sun_path.len() {
        libc::close(fd);
        return super::UV_EINVAL;
      }
      std::ptr::copy_nonoverlapping(
        path_bytes.as_ptr(),
        addr.sun_path.as_mut_ptr() as *mut u8,
        path_bytes.len(),
      );
      let addr_len =
        std::mem::size_of::<libc::sa_family_t>() + path_bytes.len();
      if libc::bind(
        fd,
        &addr as *const _ as *const libc::sockaddr,
        addr_len as libc::socklen_t,
      ) != 0
      {
        let err = std::io::Error::last_os_error();
        libc::close(fd);
        return io_error_to_uv(&err);
      }

      (*pipe).internal_fd = Some(fd);
    }
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
    // Use the pre-bound fd from uv_pipe_bind.
    let fd = match (*pipe).internal_fd {
      Some(fd) => fd,
      None => return super::UV_EINVAL,
    };

    // Start listening on the raw socket.
    let backlog = if _backlog > 0 { _backlog } else { 128 };
    if libc::listen(fd, backlog) != 0 {
      return io_error_to_uv(&std::io::Error::last_os_error());
    }

    // Set non-blocking for tokio.
    let flags = libc::fcntl(fd, libc::F_GETFL);
    if flags != -1 {
      libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // Wrap as tokio UnixListener using from_std.
    use std::os::unix::io::FromRawFd;
    let std_listener = std::os::unix::net::UnixListener::from_raw_fd(fd);
    let listener = match tokio::net::UnixListener::from_std(std_listener) {
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
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
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
      if let Ok(mut handles) = inner.pipe_handles.try_borrow_mut()
        && !handles.iter().any(|&h| std::ptr::eq(h, client))
      {
        handles.push(client);
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

  // If the pipe has a pre-bound fd (from uv_pipe_bind), use it
  // for the connect. Otherwise create a fresh connection.
  let bound_fd = unsafe { (*pipe).internal_fd };
  let future = Box::pin(async move {
    if let Some(fd) = bound_fd {
      // Non-blocking connect on the pre-bound socket.
      // SAFETY: fd is a valid socket from uv_pipe_bind.
      unsafe {
        let c_path = std::ffi::CString::new(path.as_str()).map_err(|_| {
          std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path")
        })?;
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let path_bytes = c_path.as_bytes_with_nul();
        std::ptr::copy_nonoverlapping(
          path_bytes.as_ptr(),
          addr.sun_path.as_mut_ptr() as *mut u8,
          path_bytes.len(),
        );
        let addr_len =
          std::mem::size_of::<libc::sa_family_t>() + path_bytes.len();

        // Set non-blocking before connect.
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags != -1 {
          libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        let ret = libc::connect(
          fd,
          &addr as *const _ as *const libc::sockaddr,
          addr_len as libc::socklen_t,
        );
        if ret != 0 {
          let err = std::io::Error::last_os_error();
          // EINPROGRESS is expected for non-blocking connect.
          if err.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(err);
          }

          // Wait for connect to complete by polling for write readiness.
          let async_fd = tokio::io::unix::AsyncFd::new(RawFdWrapper(fd))
            .map_err(std::io::Error::other)?;
          let _guard =
            async_fd.writable().await.map_err(std::io::Error::other)?;

          // Check SO_ERROR to see if connect succeeded.
          let mut err_val: libc::c_int = 0;
          let mut err_len: libc::socklen_t =
            std::mem::size_of::<libc::c_int>() as libc::socklen_t;
          let gso_ret = libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            &mut err_val as *mut _ as *mut c_void,
            &mut err_len,
          );
          if gso_ret != 0 {
            return Err(std::io::Error::last_os_error());
          }
          if err_val != 0 {
            return Err(std::io::Error::from_raw_os_error(err_val));
          }

          // Drop AsyncFd before creating UnixStream to avoid
          // double reactor registration. RawFdWrapper does not
          // close the fd on drop, so this is safe.
          drop(_guard);
          drop(async_fd);
        }

        // Wrap the connected fd as a tokio UnixStream.
        use std::os::unix::io::FromRawFd;
        let std_stream = std::os::unix::net::UnixStream::from_raw_fd(fd);
        tokio::net::UnixStream::from_std(std_stream)
      }
    } else {
      tokio::net::UnixStream::connect(&path).await
    }
  });

  unsafe {
    // Drop any AsyncFd created by a prior read_start_pipe call.
    // The connect future will create a UnixStream that registers
    // the same fd with the reactor; having both would corrupt
    // readiness tracking.
    (*pipe).internal_async_fd = None;

    (*pipe).internal_connect = Some(PipeConnectPending { req, cb, future });
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
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
    opts
      .first_pipe_instance(true)
      .max_instances((*pipe).pending_instances as usize);
    let server = match opts.create(&path) {
      Ok(s) => s,
      Err(e) => return io_error_to_uv(&e),
    };

    // Wrap in Arc so the connect future can hold its own reference.
    // Without awaiting connect(), the server never notices a client
    // attaching, so poll_write_ready / poll_read_ready stay Pending.
    let server = std::sync::Arc::new(server);
    let fut_server = server.clone();
    let connect_fut = Box::pin(async move { fut_server.connect().await });
    (*pipe).internal_win_server = Some(server);
    (*pipe).internal_win_connect_fut = Some(connect_fut);
    (*pipe).internal_connection_cb = cb;
    (*pipe).flags |= UV_HANDLE_ACTIVE;

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
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
      if let Ok(mut handles) = inner.pipe_handles.try_borrow_mut()
        && !handles.iter().any(|&h| std::ptr::eq(h, client))
      {
        handles.push(client);
      }

      // Create a new server instance for the next connection, and
      // queue up its connect future so we notice the next client.
      if let Some(ref path) = (*server).internal_bind_path {
        let mut opts = tokio::net::windows::named_pipe::ServerOptions::new();
        opts.first_pipe_instance(false);
        if let Ok(new_server) = opts.create(path.as_str()) {
          let new_server = std::sync::Arc::new(new_server);
          let fut_server = new_server.clone();
          let connect_fut = Box::pin(async move { fut_server.connect().await });
          (*server).internal_win_server = Some(new_server);
          (*server).internal_win_connect_fut = Some(connect_fut);
          (*server).flags |= UV_HANDLE_ACTIVE;
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
        // Verify the opened handle is actually a named pipe, not a
        // regular file. GetFileType returns FILE_TYPE_PIPE (3) for
        // named pipes. If it's something else, return ENOTSOCK.
        {
          use std::os::windows::io::AsRawHandle;
          let handle = client.as_raw_handle();
          let file_type =
            windows_sys::Win32::Storage::FileSystem::GetFileType(handle as _);
          if file_type
            != windows_sys::Win32::Storage::FileSystem::FILE_TYPE_PIPE
          {
            drop(client);
            return super::UV_ENOTSOCK;
          }
        }
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
        if let Some(w) = (*pipe).internal_waker.as_ref() {
          w.mark_ready();
        }
        0
      }
      Err(e) => {
        // If the path exists but isn't a named pipe, return ENOTSOCK
        // (matching libuv behavior). ClientOptions::open returns NotFound
        // for non-pipe paths even if the file exists.
        if e.kind() == std::io::ErrorKind::NotFound
          && RealSys.base_fs_exists_no_err(std::path::Path::new(path))
        {
          return super::UV_ENOTSOCK;
        }
        io_error_to_uv(&e)
      }
    }
  }
}

/// Attempt a non-blocking write to a pipe. Returns the number of bytes
/// written (>= 0) or a negative `UV_*` error code. `UV_EAGAIN` when the
/// pipe would block. Mirrors `uv__try_write()` in libuv.
///
/// # Safety
/// `pipe` must be a valid pointer to an initialized `uv_pipe_t`.
pub(crate) unsafe fn try_write_pipe(
  pipe: *mut uv_pipe_t,
  data: &[u8],
) -> c_int {
  unsafe {
    // Queued writes must complete in order; don't front-run them.
    if !(*pipe).internal_write_queue.is_empty()
      || (*pipe).internal_connect.is_some()
    {
      return super::UV_EAGAIN;
    }

    #[cfg(unix)]
    {
      if let Some(ref stream) = (*pipe).internal_stream {
        return match stream.try_write(data) {
          Ok(n) => n as c_int,
          Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            super::UV_EAGAIN
          }
          Err(ref e) => io_error_to_uv(e),
        };
      }
      if let Some(fd) = (*pipe).internal_fd {
        let n =
          libc::write(fd, data.as_ptr() as *const std::ffi::c_void, data.len());
        return if n >= 0 {
          n as c_int
        } else {
          let e = std::io::Error::last_os_error();
          if e.kind() == std::io::ErrorKind::WouldBlock {
            super::UV_EAGAIN
          } else {
            io_error_to_uv(&e)
          }
        };
      }
      super::UV_EBADF
    }
    #[cfg(windows)]
    {
      if let Some(ref client) = (*pipe).internal_win_client {
        return match client.try_write(data) {
          Ok(n) => n as c_int,
          Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            super::UV_EAGAIN
          }
          Err(ref e) => io_error_to_uv(e),
        };
      }
      if let Some(ref server) = (*pipe).internal_win_server {
        return match server.try_write(data) {
          Ok(n) => n as c_int,
          Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            super::UV_EAGAIN
          }
          Err(ref e) => io_error_to_uv(e),
        };
      }
      if let Some(handle) = (*pipe).internal_handle {
        use std::io::Write;
        use std::os::windows::io::FromRawHandle;
        let mut file = std::fs::File::from_raw_handle(handle);
        let result = file.write(data);
        let _ = std::os::windows::io::IntoRawHandle::into_raw_handle(file);
        return match result {
          Ok(n) => n as c_int,
          Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            super::UV_EAGAIN
          }
          Err(ref e) => io_error_to_uv(e),
        };
      }
      super::UV_EBADF
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
        // SAFETY: handle is a valid OS handle from get_osfhandle.
        windows_sys::Win32::Foundation::CloseHandle(handle as _);
      }
      // Drop the connect future first so its Arc ref is released
      // before we drop the server Arc.
      (*pipe).internal_win_connect_fut = None;
      // Drop named pipe server/client (closes handles).
      (*pipe).internal_win_server = None;
      (*pipe).internal_win_client = None;
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

    // Lazily create AsyncFd for the fd-based read path (uv_pipe_open).
    // Not needed if internal_stream is set (connect/accept path).
    // Also skip if a connect is pending -- the connect will provide its
    // own UnixStream, and double-registering the fd with tokio's reactor
    // causes readiness tracking corruption.
    if (*pipe).internal_async_fd.is_none()
      && (*pipe).internal_stream.is_none()
      && (*pipe).internal_connect.is_none()
      && (*pipe).internal_fd.is_some()
    {
      let fd = (*pipe).internal_fd.unwrap();
      if let Ok(afd) = tokio::io::unix::AsyncFd::new(RawFdWrapper(fd)) {
        (*pipe).internal_async_fd = Some(afd);
      }
    }

    let inner = super::get_inner((*pipe).loop_);
    let mut handles = inner.pipe_handles.borrow_mut();
    if !handles.iter().any(|&h| std::ptr::eq(h, pipe)) {
      handles.push(pipe);
    }
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
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
    if let Some(w) = (*pipe).internal_waker.as_ref() {
      w.mark_ready();
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

    let join = tokio::task::spawn_blocking(move || {
      use std::io::Read;
      use std::os::windows::io::FromRawHandle;
      // SAFETY: handle is a valid OS handle stored by uv_pipe_open.
      // We reconstruct a File temporarily, read from it, then leak it
      // back to avoid double-close.
      let mut file = std::fs::File::from_raw_handle(
        handle as std::os::windows::io::RawHandle,
      );
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

    // 2. Poll the pending `NamedPipeServer::connect()` future. When it
    // resolves, a client has attached to the named pipe and we fire
    // the connection callback so JS-side accept() can pick it up.
    if let Some(ref mut fut) = (*pipe_ptr).internal_win_connect_fut
      && let Poll::Ready(res) = fut.as_mut().poll(cx)
    {
      (*pipe_ptr).internal_win_connect_fut = None;
      let status = match res {
        Ok(()) => 0,
        Err(ref e) => io_error_to_uv(e),
      };
      if let Some(cb) = (*pipe_ptr).internal_connection_cb {
        cb(pipe_ptr as *mut uv_stream_t, status);
      }
      any_work = true;
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
      use std::os::windows::io::FromRawHandle;
      // Register write-readiness with the reactor so the waker fires
      // when the pipe becomes writable. Without this, try_write can
      // loop forever on WouldBlock because the driver never learns
      // we are interested in write-readiness.
      if let Some(ref server) = (*pipe_ptr).internal_win_server {
        let _ = server.poll_write_ready(cx);
      }
      if let Some(ref client) = (*pipe_ptr).internal_win_client {
        let _ = client.poll_write_ready(cx);
      }
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
        Ok(n) => {
          any_work = true;
          let pw = (*pipe_ptr).internal_write_queue.front_mut().unwrap();
          pw.offset += n;
          if pw.offset >= pw.data.len() {
            let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
            if let Some(cb) = pw.cb {
              cb(pw.req, 0);
            }
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
        if let Some(ref mut join) = (*pipe_ptr).internal_pending_read
          && let Poll::Ready(result) = std::pin::Pin::new(join).poll(cx)
        {
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
              Ok(Ok((_, 0))) => {
                read_cb(pipe_ptr as *mut uv_stream_t, UV_EOF as isize, &buf);
                (*pipe_ptr).internal_reading = false;
              }
              Ok(Ok((data, n))) => {
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

    // 6. Deactivate handle when idle so the event loop can exit.
    // In libuv, a pipe handle is only "active" when it has pending
    // reads, writes, connects, or is listening. Without this, an
    // opened-but-idle pipe (e.g. process.stdout) would keep the
    // event loop alive forever.
    let has_pending_work = !(*pipe_ptr).internal_write_queue.is_empty()
      || (*pipe_ptr).internal_reading
      || (*pipe_ptr).internal_connect.is_some()
      || (*pipe_ptr).internal_win_connect_fut.is_some()
      || ((*pipe_ptr).internal_win_server.is_some()
        && (*pipe_ptr).internal_connection_cb.is_some())
      || (*pipe_ptr).internal_shutdown.is_some();
    if !has_pending_work {
      (*pipe_ptr).flags &= !UV_HANDLE_ACTIVE;
    }

    // Re-register tokio read/write interest for the next edge.
    //
    // `NamedPipeServer::poll_{read,write}_ready` only stores a waker
    // when it returns Pending. If the drain above saw Ready and
    // consumed all available data/space with `try_read` / `try_write`,
    // no waker is registered — and since the ready-queue polling
    // only re-enters `poll_pipe_handle` on a wake, the next event
    // (EOF after child exit, more data arriving, write-side draining)
    // would never reach us. Calling poll_*_ready here after the drain
    // either stores a waker (Pending) or signals us to re-queue
    // (Ready). Mirrors the re-register block at the end of
    // `tcp::poll_tcp_handle`.
    let mut needs_requeue = false;
    if (*pipe_ptr).internal_reading {
      if let Some(ref server) = (*pipe_ptr).internal_win_server
        && matches!(server.poll_read_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
      if let Some(ref client) = (*pipe_ptr).internal_win_client
        && matches!(client.poll_read_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
    }
    if !(*pipe_ptr).internal_write_queue.is_empty() {
      if let Some(ref server) = (*pipe_ptr).internal_win_server
        && matches!(server.poll_write_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
      if let Some(ref client) = (*pipe_ptr).internal_win_client
        && matches!(client.poll_write_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
    }
    if needs_requeue && let Some(w) = (*pipe_ptr).internal_waker.as_ref() {
      w.mark_ready();
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
          // Drop the AsyncFd from uv_pipe_open (if any) since the
          // connected stream now owns the fd.
          (*pipe_ptr).internal_async_fd = None;
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
      // Prefer AsyncFd for proper readiness tracking, then UnixStream,
      // then fall back to raw libc::write.
      let write_result = if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
        match afd.poll_write_ready(cx) {
          Poll::Ready(Ok(mut guard)) => {
            let fd = afd.get_ref().0;
            let n = libc::write(
              fd,
              remaining.as_ptr() as *const c_void,
              remaining.len(),
            );
            if n >= 0 {
              guard.retain_ready();
              Ok(n as usize)
            } else {
              let err = std::io::Error::last_os_error();
              if err.kind() == std::io::ErrorKind::WouldBlock {
                guard.clear_ready();
              }
              Err(err)
            }
          }
          Poll::Ready(Err(e)) => Err(e),
          Poll::Pending => {
            // AsyncFd not ready, but pipe buffer may have space.
            // Try a non-blocking write to check.
            let fd = afd.get_ref().0;
            let n = libc::write(
              fd,
              remaining.as_ptr() as *const c_void,
              remaining.len(),
            );
            if n > 0 {
              Ok(n as usize)
            } else if n == 0 {
              break;
            } else {
              let err = std::io::Error::last_os_error();
              if err.kind() == std::io::ErrorKind::WouldBlock {
                break;
              }
              Err(err)
            }
          }
        }
      } else if let Some(ref stream) = (*pipe_ptr).internal_stream {
        stream.try_write(remaining)
      } else if let Some(fd) = (*pipe_ptr).internal_fd {
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
        Ok(n) => {
          any_work = true;
          let pw = (*pipe_ptr).internal_write_queue.front_mut().unwrap();
          pw.offset += n;
          if pw.offset >= pw.data.len() {
            let pw = (*pipe_ptr).internal_write_queue.pop_front().unwrap();
            if let Some(cb) = pw.cb {
              cb(pw.req, 0);
            }
          }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          // Lazily create AsyncFd for write readiness tracking.
          if (*pipe_ptr).internal_async_fd.is_none()
            && let Some(fd) = (*pipe_ptr).internal_fd
            && let Ok(afd) = tokio::io::unix::AsyncFd::new(RawFdWrapper(fd))
          {
            (*pipe_ptr).internal_async_fd = Some(afd);
            let _ = (*pipe_ptr)
              .internal_async_fd
              .as_ref()
              .unwrap()
              .poll_write_ready(cx);
          } else if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
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
          // Use AsyncFd for proper readiness tracking with the reactor.
          // On Pending, try a raw read anyway for the first iteration
          // (data may already be buffered from before AsyncFd creation).
          let read_result = if let Some(ref afd) = (*pipe_ptr).internal_async_fd
          {
            match afd.poll_read_ready(cx) {
              Poll::Ready(Ok(mut guard)) => {
                let fd = afd.get_ref().0;
                let n = libc::read(fd, buf.base as *mut c_void, buf.len);
                if n >= 0 {
                  guard.retain_ready();
                  Ok(n as usize)
                } else {
                  let err = std::io::Error::last_os_error();
                  if err.kind() == std::io::ErrorKind::WouldBlock {
                    guard.clear_ready();
                  }
                  Err(err)
                }
              }
              Poll::Ready(Err(e)) => Err(e),
              Poll::Pending => {
                // AsyncFd says not ready, but data may already be in
                // the pipe buffer (edge was consumed before registration).
                // Try a non-blocking read to check.
                let fd = afd.get_ref().0;
                let n = libc::read(fd, buf.base as *mut c_void, buf.len);
                if n > 0 {
                  Ok(n as usize)
                } else if n == 0 {
                  Ok(0) // EOF
                } else {
                  let err = std::io::Error::last_os_error();
                  if err.kind() == std::io::ErrorKind::WouldBlock {
                    // Truly no data - free buf and wait for wakeup.
                    read_cb(pipe_ptr as *mut uv_stream_t, 0, &buf);
                    break;
                  }
                  Err(err)
                }
              }
            }
          } else if let Some(ref stream) = (*pipe_ptr).internal_stream {
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
              // Register for read readiness so the event loop wakes us.
              if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
                let _ = afd.poll_read_ready(cx);
              }
              if let Some(ref stream) = (*pipe_ptr).internal_stream {
                let _ = stream.poll_read_ready(cx);
              }
              // Callback with nread=0 so alloc buf is freed.
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
      } else if let Some(fd) = (*pipe_ptr).internal_fd {
        // For fd-based pipes (uv_pipe_open), try shutdown. This works
        // for socketpair fds (used by extra stdio pipes) but is a no-op
        // error for regular pipes (stdin/stdout). Ignore the error.
        libc::shutdown(fd, libc::SHUT_WR);
      }
      if let Some(cb) = pending.cb {
        cb(pending.req, 0);
      }
      any_work = true;
    }

    // 6. Deactivate handle when idle so the event loop can exit.
    // In libuv, a pipe handle is only "active" when it has pending
    // reads, writes, connects, or is listening. Without this, an
    // opened-but-idle pipe (e.g. process.stdout) would keep the
    // event loop alive forever.
    let has_pending_work = !(*pipe_ptr).internal_write_queue.is_empty()
      || (*pipe_ptr).internal_reading
      || (*pipe_ptr).internal_connect.is_some()
      || ((*pipe_ptr).internal_listener.is_some()
        && (*pipe_ptr).internal_connection_cb.is_some())
      || (*pipe_ptr).internal_shutdown.is_some();
    if !has_pending_work {
      (*pipe_ptr).flags &= !UV_HANDLE_ACTIVE;
    }

    // Re-register tokio readiness interest if the poll paths above
    // consumed it without leaving a waker. See the matching comment
    // in tcp::poll_tcp_handle for the full rationale — in short, if
    // poll_*_ready returns Ready and we fully drain it, no waker is
    // registered for the *next* edge, so we either re-register here
    // (if still Pending) or re-queue the handle for another pass
    // (if still Ready).
    let mut needs_requeue = false;
    if (*pipe_ptr).internal_reading {
      if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
        if matches!(afd.poll_read_ready(cx), Poll::Ready(_)) {
          needs_requeue = true;
        }
      } else if let Some(ref stream) = (*pipe_ptr).internal_stream
        && matches!(stream.poll_read_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
    }
    if !(*pipe_ptr).internal_write_queue.is_empty() {
      if let Some(ref afd) = (*pipe_ptr).internal_async_fd {
        if matches!(afd.poll_write_ready(cx), Poll::Ready(_)) {
          needs_requeue = true;
        }
      } else if let Some(ref stream) = (*pipe_ptr).internal_stream
        && matches!(stream.poll_write_ready(cx), Poll::Ready(_))
      {
        needs_requeue = true;
      }
    }
    if needs_requeue && let Some(w) = (*pipe_ptr).internal_waker.as_ref() {
      w.mark_ready();
    }
  }

  any_work
}
