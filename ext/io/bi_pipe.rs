// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::rc::Rc;

use deno_core::error::AnyError;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub type RawBiPipeHandle = super::RawIoHandle;

/// One end of a bidirectional pipe. This implements the
/// `Resource` trait.
pub struct BiPipeResource {
  read_half: AsyncRefCell<BiPipeRead>,
  write_half: AsyncRefCell<BiPipeWrite>,
  cancel: CancelHandle,
  raw_handle: RawBiPipeHandle,
}

#[cfg(windows)]
// workaround because `RawHandle` doesn't impl `AsRawHandle`
mod as_raw_handle {
  use super::RawBiPipeHandle;
  pub(super) struct RawHandleWrap(pub(super) RawBiPipeHandle);
  impl std::os::windows::io::AsRawHandle for RawHandleWrap {
    fn as_raw_handle(&self) -> std::os::windows::prelude::RawHandle {
      self.0
    }
  }
}

impl deno_core::Resource for BiPipeResource {
  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }

  fn backing_handle(self: Rc<Self>) -> Option<deno_core::ResourceHandle> {
    #[cfg(unix)]
    {
      Some(deno_core::ResourceHandle::from_fd_like(&self.raw_handle))
    }
    #[cfg(windows)]
    {
      Some(deno_core::ResourceHandle::from_fd_like(
        &as_raw_handle::RawHandleWrap(self.raw_handle),
      ))
    }
  }

  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();
}

impl BiPipeResource {
  pub fn from_raw_handle(raw: RawBiPipeHandle) -> Result<Self, std::io::Error> {
    let pipe = BiPipe::from_raw(raw)?;
    let (read, write) = pipe.split();
    Ok(Self {
      raw_handle: raw,
      read_half: AsyncRefCell::new(read),
      write_half: AsyncRefCell::new(write),
      cancel: Default::default(),
    })
  }

  pub async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, AnyError> {
    let mut rd = RcRef::map(&self, |r| &r.read_half).borrow_mut().await;
    let cancel_handle = RcRef::map(&self, |r| &r.cancel);
    Ok(rd.read(data).try_or_cancel(cancel_handle).await?)
  }

  pub async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, AnyError> {
    let mut wr = RcRef::map(self, |r| &r.write_half).borrow_mut().await;
    let nwritten = wr.write(data).await?;
    wr.flush().await?;
    Ok(nwritten)
  }
}

/// One end of a bidirectional pipe
#[pin_project::pin_project]
pub struct BiPipe {
  #[pin]
  read_end: BiPipeRead,
  #[pin]
  write_end: BiPipeWrite,
}

impl BiPipe {
  pub fn from_raw(raw: RawBiPipeHandle) -> Result<Self, std::io::Error> {
    let (read_end, write_end) = from_raw(raw)?;
    Ok(Self {
      read_end,
      write_end,
    })
  }

  pub fn split(self) -> (BiPipeRead, BiPipeWrite) {
    (self.read_end, self.write_end)
  }

  pub fn unsplit(read_end: BiPipeRead, write_end: BiPipeWrite) -> Self {
    Self {
      read_end,
      write_end,
    }
  }
}

#[pin_project::pin_project]
pub struct BiPipeRead {
  #[cfg(unix)]
  #[pin]
  inner: tokio::net::unix::OwnedReadHalf,
  #[cfg(windows)]
  #[pin]
  inner: tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>,
}

#[cfg(unix)]
impl From<tokio::net::unix::OwnedReadHalf> for BiPipeRead {
  fn from(value: tokio::net::unix::OwnedReadHalf) -> Self {
    Self { inner: value }
  }
}
#[cfg(windows)]
impl From<tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>>
  for BiPipeRead
{
  fn from(
    value: tokio::io::ReadHalf<
      tokio::net::windows::named_pipe::NamedPipeClient,
    >,
  ) -> Self {
    Self { inner: value }
  }
}

#[pin_project::pin_project]
pub struct BiPipeWrite {
  #[cfg(unix)]
  #[pin]
  inner: tokio::net::unix::OwnedWriteHalf,
  #[cfg(windows)]
  #[pin]
  inner: tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>,
}

#[cfg(unix)]
impl From<tokio::net::unix::OwnedWriteHalf> for BiPipeWrite {
  fn from(value: tokio::net::unix::OwnedWriteHalf) -> Self {
    Self { inner: value }
  }
}

#[cfg(windows)]
impl
  From<tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>>
  for BiPipeWrite
{
  fn from(
    value: tokio::io::WriteHalf<
      tokio::net::windows::named_pipe::NamedPipeClient,
    >,
  ) -> Self {
    Self { inner: value }
  }
}

#[cfg(unix)]
fn from_raw(
  stream: RawBiPipeHandle,
) -> Result<(BiPipeRead, BiPipeWrite), std::io::Error> {
  use std::os::fd::FromRawFd;
  // Safety: The fd is part of a pair of connected sockets
  let unix_stream = tokio::net::UnixStream::from_std(unsafe {
    std::os::unix::net::UnixStream::from_raw_fd(stream)
  })?;
  let (read, write) = unix_stream.into_split();
  Ok((BiPipeRead { inner: read }, BiPipeWrite { inner: write }))
}

#[cfg(windows)]
fn from_raw(
  handle: RawBiPipeHandle,
) -> Result<(BiPipeRead, BiPipeWrite), std::io::Error> {
  // Safety: We cannot use `get_osfhandle` because Deno statically links to msvcrt. It is not guaranteed that the
  // fd handle map will be the same.
  let pipe = unsafe {
    tokio::net::windows::named_pipe::NamedPipeClient::from_raw_handle(
      handle as _,
    )?
  };
  let (read, write) = tokio::io::split(pipe);
  Ok((BiPipeRead { inner: read }, BiPipeWrite { inner: write }))
}

impl tokio::io::AsyncRead for BiPipeRead {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    self.project().inner.poll_read(cx, buf)
  }
}
impl tokio::io::AsyncRead for BiPipe {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    self.project().read_end.poll_read(cx, buf)
  }
}

// implement `AsyncWrite` for `$name`, delegating
// the impl to `$field`. `$name` must have a `project` method
// with a projected `$field` (e.g. with `pin_project::pin_project`)
macro_rules! impl_async_write {
  (for $name: ident -> self.$field: ident) => {
    impl tokio::io::AsyncWrite for $name {
      fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
      ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.project().$field.poll_write_vectored(cx, bufs)
      }

      fn is_write_vectored(&self) -> bool {
        self.$field.is_write_vectored()
      }

      fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
      ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.project().$field.poll_write(cx, buf)
      }

      fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
      ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().$field.poll_flush(cx)
      }

      fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
      ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().$field.poll_shutdown(cx)
      }
    }
  };
}

impl_async_write!(for BiPipeWrite -> self.inner);
impl_async_write!(for BiPipe -> self.write_end);

/// Creates both sides of a bidirectional pipe, returning the raw
/// handles to the underlying OS resources.
pub fn bi_pipe_pair_raw() -> Result<(RawBiPipeHandle, RawBiPipeHandle), AnyError>
{
  #[cfg(unix)]
  {
    // SockFlag is broken on macOS
    // https://github.com/nix-rust/nix/issues/861
    let mut fds = [-1, -1];
    #[cfg(not(target_os = "macos"))]
    let flags = libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK;

    #[cfg(target_os = "macos")]
    let flags = 0;

    // SAFETY: libc call, fds are correct size+align
    let ret = unsafe {
      libc::socketpair(
        libc::AF_UNIX,
        libc::SOCK_STREAM | flags,
        0,
        fds.as_mut_ptr(),
      )
    };
    if ret != 0 {
      return Err(std::io::Error::last_os_error().into());
    }

    if cfg!(target_os = "macos") {
      let fcntl = |fd: i32, flag: libc::c_int| -> Result<(), std::io::Error> {
        // SAFETY: libc call, fd is valid
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };

        if flags == -1 {
          return Err(fail(fds));
        }
        // SAFETY: libc call, fd is valid
        let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | flag) };
        if ret == -1 {
          return Err(fail(fds));
        }
        Ok(())
      };

      fn fail(fds: [i32; 2]) -> std::io::Error {
        // SAFETY: libc call, fds are valid
        unsafe {
          libc::close(fds[0]);
          libc::close(fds[1]);
        }
        std::io::Error::last_os_error()
      }

      // SOCK_NONBLOCK is not supported on macOS.
      (fcntl)(fds[0], libc::O_NONBLOCK)?;
      (fcntl)(fds[1], libc::O_NONBLOCK)?;

      // SOCK_CLOEXEC is not supported on macOS.
      (fcntl)(fds[0], libc::FD_CLOEXEC)?;
      (fcntl)(fds[1], libc::FD_CLOEXEC)?;
    }

    let fd1 = fds[0];
    let fd2 = fds[1];
    Ok((fd1, fd2))
  }
  #[cfg(windows)]
  {
    // TODO(nathanwhit): more granular unsafe blocks
    // SAFETY: win32 calls
    unsafe {
      use windows_sys::Win32::Foundation::CloseHandle;
      use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
      use windows_sys::Win32::Foundation::ERROR_PIPE_CONNECTED;
      use windows_sys::Win32::Foundation::GENERIC_READ;
      use windows_sys::Win32::Foundation::GENERIC_WRITE;
      use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
      use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
      use windows_sys::Win32::Storage::FileSystem::CreateFileW;
      use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_FIRST_PIPE_INSTANCE;
      use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
      use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
      use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
      use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
      use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
      use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
      use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;

      use std::io;
      use std::os::windows::ffi::OsStrExt;
      use std::path::Path;
      use std::ptr;

      let (path, hd1) = loop {
        let name = format!("\\\\.\\pipe\\{}", uuid::Uuid::new_v4());
        let mut path = Path::new(&name)
          .as_os_str()
          .encode_wide()
          .collect::<Vec<_>>();
        path.push(0);

        let hd1 = CreateNamedPipeW(
          path.as_ptr(),
          PIPE_ACCESS_DUPLEX
            | FILE_FLAG_FIRST_PIPE_INSTANCE
            | FILE_FLAG_OVERLAPPED,
          PIPE_TYPE_BYTE | PIPE_READMODE_BYTE,
          1,
          65536,
          65536,
          0,
          std::ptr::null_mut(),
        );

        if hd1 == INVALID_HANDLE_VALUE {
          let err = io::Error::last_os_error();
          /* If the pipe name is already in use, try again. */
          if err.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
            continue;
          }

          return Err(err.into());
        }

        break (path, hd1);
      };

      /* Create child pipe handle. */
      let s = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
      };
      let hd2 = CreateFileW(
        path.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        0,
        &s,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        0,
      );
      if hd2 == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error().into());
      }

      // Will not block because we have create the pair.
      if ConnectNamedPipe(hd1, ptr::null_mut()) == 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
          CloseHandle(hd2);
          return Err(err.into());
        }
      }

      Ok((hd1 as _, hd2 as _))
    }
  }
}
