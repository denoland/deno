// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::IsTerminal;

/// Represents an underlying handle for a platform. On unix, everything is an `fd`. On Windows, everything
/// is a Windows handle except for sockets (which are `SOCKET`s).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[allow(unused)]
pub enum ResourceHandle {
  /// A file handle/descriptor.
  Fd(ResourceHandleFd),
  /// A socket handle/file descriptor.
  Socket(ResourceHandleSocket),
}

#[cfg(unix)]
pub type ResourceHandleFd = std::os::fd::RawFd;
#[cfg(unix)]
pub type ResourceHandleSocket = std::os::fd::RawFd;
#[cfg(windows)]
pub type ResourceHandleFd = std::os::windows::io::RawHandle;
#[cfg(windows)]
pub type ResourceHandleSocket = std::os::windows::io::RawSocket;

impl ResourceHandle {
  /// Converts a file-like thing to a [`ResourceHandle`].
  #[cfg(windows)]
  pub fn from_fd_like(io: &impl std::os::windows::io::AsRawHandle) -> Self {
    Self::Fd(io.as_raw_handle())
  }

  /// Converts a file-like thing to a [`ResourceHandle`].
  #[cfg(unix)]
  pub fn from_fd_like(io: &impl std::os::unix::io::AsRawFd) -> Self {
    Self::Fd(io.as_raw_fd())
  }

  /// Converts a socket-like thing to a [`ResourceHandle`].
  #[cfg(windows)]
  pub fn from_socket_like(io: &impl std::os::windows::io::AsRawSocket) -> Self {
    Self::Socket(io.as_raw_socket())
  }

  /// Converts a socket-like thing to a [`ResourceHandle`].
  #[cfg(unix)]
  pub fn from_socket_like(io: &impl std::os::unix::io::AsRawFd) -> Self {
    Self::Socket(io.as_raw_fd())
  }

  /// Runs a basic validity check on the handle, but cannot fully determine if the handle is valid for use.
  pub fn is_valid(&self) -> bool {
    #[cfg(windows)]
    {
      match self {
        // NULL or INVALID_HANDLE_VALUE
        Self::Fd(handle) => {
          !handle.is_null()
            && *handle != -1_isize as std::os::windows::io::RawHandle
        }
        // INVALID_SOCKET
        Self::Socket(socket) => {
          *socket != -1_i64 as std::os::windows::io::RawSocket
        }
      }
    }
    #[cfg(unix)]
    {
      match self {
        Self::Fd(fd) => *fd >= 0,
        Self::Socket(fd) => *fd >= 0,
      }
    }
  }

  /// Returns this as a file-descriptor-like handle.
  pub fn as_fd_like(&self) -> Option<ResourceHandleFd> {
    match self {
      Self::Fd(fd) => Some(*fd),
      _ => None,
    }
  }

  /// Returns this as a socket-like handle.
  pub fn as_socket_like(&self) -> Option<ResourceHandleSocket> {
    match self {
      Self::Socket(socket) => Some(*socket),
      _ => None,
    }
  }

  /// Determines if this handle is a terminal. Analagous to [`std::io::IsTerminal`].
  pub fn is_terminal(&self) -> bool {
    match self {
      Self::Fd(fd) if self.is_valid() => {
        #[cfg(windows)]
        {
          // SAFETY: The resource remains open for the for the duration of borrow_raw
          unsafe {
            std::os::windows::io::BorrowedHandle::borrow_raw(*fd).is_terminal()
          }
        }
        #[cfg(unix)]
        {
          // SAFETY: The resource remains open for the for the duration of borrow_raw
          unsafe { std::os::fd::BorrowedFd::borrow_raw(*fd).is_terminal() }
        }
      }
      _ => false,
    }
  }
}
