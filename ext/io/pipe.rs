// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use std::io;
use std::pin::Pin;
use std::process::Stdio;

pub type RawPipeHandle = super::RawIoHandle;

// The synchronous read end of a unidirectional pipe.
pub struct PipeRead {
  file: std::fs::File,
}

// The asynchronous read end of a unidirectional pipe.
pub struct AsyncPipeRead {
  #[cfg(windows)]
  /// We use a `ChildStdout` here as it's a much better fit for a Windows named pipe on Windows. We
  /// might also be able to use `tokio::net::windows::named_pipe::NamedPipeClient` in the future
  /// if those can be created from raw handles down the road.
  read: tokio::process::ChildStdout,
  #[cfg(not(windows))]
  read: tokio::net::unix::pipe::Receiver,
}

// The synchronous write end of a unidirectional pipe.
pub struct PipeWrite {
  file: std::fs::File,
}

// The asynchronous write end of a unidirectional pipe.
pub struct AsyncPipeWrite {
  #[cfg(windows)]
  /// We use a `ChildStdin` here as it's a much better fit for a Windows named pipe on Windows. We
  /// might also be able to use `tokio::net::windows::named_pipe::NamedPipeClient` in the future
  /// if those can be created from raw handles down the road.
  write: tokio::process::ChildStdin,
  #[cfg(not(windows))]
  write: tokio::net::unix::pipe::Sender,
}

impl PipeRead {
  /// Converts this sync reader into an async reader. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(windows)]
  pub fn into_async(self) -> io::Result<AsyncPipeRead> {
    let owned: std::os::windows::io::OwnedHandle = self.file.into();
    let stdout = std::process::ChildStdout::from(owned);
    Ok(AsyncPipeRead {
      read: tokio::process::ChildStdout::from_std(stdout)?,
    })
  }

  /// Converts this sync reader into an async reader. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(not(windows))]
  pub fn into_async(self) -> io::Result<AsyncPipeRead> {
    Ok(AsyncPipeRead {
      read: tokio::net::unix::pipe::Receiver::from_file(self.file)?,
    })
  }

  /// Creates a new [`PipeRead`] instance that shares the same underlying file handle
  /// as the existing [`PipeRead`] instance.
  pub fn try_clone(&self) -> io::Result<Self> {
    Ok(Self {
      file: self.file.try_clone()?,
    })
  }
}

impl AsyncPipeRead {
  /// Converts this async reader into an sync reader. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(windows)]
  pub fn into_sync(self) -> io::Result<PipeRead> {
    let owned = self.read.into_owned_handle()?;
    Ok(PipeRead { file: owned.into() })
  }

  /// Converts this async reader into an sync reader. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(not(windows))]
  pub fn into_sync(self) -> io::Result<PipeRead> {
    let file = self.read.into_nonblocking_fd()?.into();
    Ok(PipeRead { file })
  }
}

impl std::io::Read for PipeRead {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.file.read(buf)
  }

  fn read_vectored(
    &mut self,
    bufs: &mut [io::IoSliceMut<'_>],
  ) -> io::Result<usize> {
    self.file.read_vectored(bufs)
  }
}

impl tokio::io::AsyncRead for AsyncPipeRead {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().read).poll_read(cx, buf)
  }
}

impl PipeWrite {
  /// Converts this sync writer into an async writer. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(windows)]
  pub fn into_async(self) -> io::Result<AsyncPipeWrite> {
    let owned: std::os::windows::io::OwnedHandle = self.file.into();
    let stdin = std::process::ChildStdin::from(owned);
    Ok(AsyncPipeWrite {
      write: tokio::process::ChildStdin::from_std(stdin)?,
    })
  }

  /// Converts this sync writer into an async writer. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(not(windows))]
  pub fn into_async(self) -> io::Result<AsyncPipeWrite> {
    Ok(AsyncPipeWrite {
      write: tokio::net::unix::pipe::Sender::from_file(self.file)?,
    })
  }

  /// Creates a new [`PipeWrite`] instance that shares the same underlying file handle
  /// as the existing [`PipeWrite`] instance.
  pub fn try_clone(&self) -> io::Result<Self> {
    Ok(Self {
      file: self.file.try_clone()?,
    })
  }
}

impl AsyncPipeWrite {
  /// Converts this async writer into an sync writer. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(windows)]
  pub fn into_sync(self) -> io::Result<PipeWrite> {
    let owned = self.write.into_owned_handle()?;
    Ok(PipeWrite { file: owned.into() })
  }

  /// Converts this async writer into an sync writer. May fail if the Tokio runtime is
  /// unavailable.
  #[cfg(not(windows))]
  pub fn into_sync(self) -> io::Result<PipeWrite> {
    let file = self.write.into_nonblocking_fd()?.into();
    Ok(PipeWrite { file })
  }
}

impl std::io::Write for PipeWrite {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.file.write(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.file.flush()
  }

  fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
    self.file.write_vectored(bufs)
  }
}

impl tokio::io::AsyncWrite for AsyncPipeWrite {
  #[inline(always)]
  fn poll_write(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, io::Error>> {
    Pin::new(&mut self.get_mut().write).poll_write(cx, buf)
  }

  #[inline(always)]
  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), io::Error>> {
    Pin::new(&mut self.get_mut().write).poll_flush(cx)
  }

  #[inline(always)]
  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), io::Error>> {
    Pin::new(&mut self.get_mut().write).poll_shutdown(cx)
  }

  #[inline(always)]
  fn is_write_vectored(&self) -> bool {
    self.write.is_write_vectored()
  }

  #[inline(always)]
  fn poll_write_vectored(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    bufs: &[io::IoSlice<'_>],
  ) -> std::task::Poll<Result<usize, io::Error>> {
    Pin::new(&mut self.get_mut().write).poll_write_vectored(cx, bufs)
  }
}

impl From<PipeRead> for Stdio {
  fn from(val: PipeRead) -> Self {
    Stdio::from(val.file)
  }
}

impl From<PipeWrite> for Stdio {
  fn from(val: PipeWrite) -> Self {
    Stdio::from(val.file)
  }
}

impl From<PipeRead> for std::fs::File {
  fn from(val: PipeRead) -> Self {
    val.file
  }
}

impl From<PipeWrite> for std::fs::File {
  fn from(val: PipeWrite) -> Self {
    val.file
  }
}

#[cfg(not(windows))]
impl From<PipeRead> for std::os::unix::io::OwnedFd {
  fn from(val: PipeRead) -> Self {
    val.file.into()
  }
}

#[cfg(not(windows))]
impl From<PipeWrite> for std::os::unix::io::OwnedFd {
  fn from(val: PipeWrite) -> Self {
    val.file.into()
  }
}

#[cfg(windows)]
impl From<PipeRead> for std::os::windows::io::OwnedHandle {
  fn from(val: PipeRead) -> Self {
    val.file.into()
  }
}

#[cfg(windows)]
impl From<PipeWrite> for std::os::windows::io::OwnedHandle {
  fn from(val: PipeWrite) -> Self {
    val.file.into()
  }
}

/// Create a unidirectional pipe pair that starts off as a pair of synchronous file handles,
/// but either side may be promoted to an async-capable reader/writer.
///
/// On Windows, we use a named pipe because that's the only way to get reliable async I/O
/// support. On Unix platforms, we use the `os_pipe` library, which uses `pipe2` under the hood
/// (or `pipe` on OSX).
pub fn pipe() -> io::Result<(PipeRead, PipeWrite)> {
  pipe_impl()
}

/// Creates a unidirectional pipe on top of a named pipe (which is technically bidirectional).
#[cfg(windows)]
pub fn pipe_impl() -> io::Result<(PipeRead, PipeWrite)> {
  // SAFETY: We're careful with handles here
  unsafe {
    use std::os::windows::io::FromRawHandle;
    use std::os::windows::io::OwnedHandle;
    let (server, client) = crate::winpipe::create_named_pipe()?;
    let read = std::fs::File::from(OwnedHandle::from_raw_handle(client));
    let write = std::fs::File::from(OwnedHandle::from_raw_handle(server));
    Ok((PipeRead { file: read }, PipeWrite { file: write }))
  }
}

/// Creates a unidirectional pipe for unix platforms.
#[cfg(not(windows))]
pub fn pipe_impl() -> io::Result<(PipeRead, PipeWrite)> {
  use std::os::unix::io::OwnedFd;
  let (read, write) = os_pipe::pipe()?;
  let read = std::fs::File::from(Into::<OwnedFd>::into(read));
  let write = std::fs::File::from(Into::<OwnedFd>::into(write));
  Ok((PipeRead { file: read }, PipeWrite { file: write }))
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::io::Read;
  use std::io::Write;
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;

  #[test]
  fn test_pipe() {
    let (mut read, mut write) = pipe().unwrap();
    // Write to the server and read from the client
    write.write_all(b"hello").unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }

  #[tokio::test]
  async fn test_async_pipe() {
    let (read, write) = pipe().unwrap();
    let mut read = read.into_async().unwrap();
    let mut write = write.into_async().unwrap();

    write.write_all(b"hello").await.unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"hello");
  }

  /// Test a round-trip through async mode and back.
  #[tokio::test]
  async fn test_pipe_transmute() {
    let (mut read, mut write) = pipe().unwrap();

    // Sync
    write.write_all(b"hello").unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");

    let mut read = read.into_async().unwrap();
    let mut write = write.into_async().unwrap();

    // Async
    write.write_all(b"hello").await.unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"hello");

    let mut read = read.into_sync().unwrap();
    let mut write = write.into_sync().unwrap();

    // Sync
    write.write_all(b"hello").unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }

  #[tokio::test]
  async fn test_async_pipe_is_nonblocking() {
    let (read, write) = pipe().unwrap();
    let mut read = read.into_async().unwrap();
    let mut write = write.into_async().unwrap();

    let a = tokio::spawn(async move {
      let mut buf: [u8; 5] = Default::default();
      read.read_exact(&mut buf).await.unwrap();
      assert_eq!(&buf, b"hello");
    });
    let b = tokio::spawn(async move {
      write.write_all(b"hello").await.unwrap();
    });

    a.await.unwrap();
    b.await.unwrap();
  }
}
