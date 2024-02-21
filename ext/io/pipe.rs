use std::io;
use std::pin::Pin;

pub struct PipeRead {
  file: std::fs::File,
}

pub struct AsyncPipeRead {
  #[cfg(windows)]
  read: tokio::process::ChildStdout,
  #[cfg(not(windows))]
  read: tokio::net::unix::pipe::Receiver,
}

pub struct PipeWrite {
  file: std::fs::File,
}

pub struct AsyncPipeWrite {
  #[cfg(windows)]
  write: tokio::process::ChildStdin,
  #[cfg(not(windows))]
  write: tokio::net::unix::pipe::Sender
}

impl PipeRead {
  #[cfg(windows)]
  pub fn into_async(self) -> AsyncPipeRead {
    let owned: std::os::windows::io::OwnedHandle = self.file.into();
    let stdout = std::process::ChildStdout::from(owned);
    AsyncPipeRead { read: tokio::process::ChildStdout::from_std(stdout).unwrap() }
  }
  #[cfg(not(windows))]
  pub fn into_async(self) -> AsyncPipeRead {
    AsyncPipeRead { read: tokio::net::unix::pipe::Receiver::from_file(self.file).unwrap() }
  }
}

impl AsyncPipeRead {
  #[cfg(windows)]
  pub fn into_sync(self) -> PipeRead {
    let owned = self.read.into_owned_handle().unwrap();
    PipeRead { file: owned.into() }
  }
  #[cfg(not(windows))]
  pub fn into_sync(self) -> PipeRead {
    let file = self.read.into_nonblocking_fd().unwrap().into();
    PipeRead { file }
  }
}

impl std::io::Read for PipeRead {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.file.read(buf)
  }

  fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
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
  #[cfg(windows)]
  pub fn into_async(self) -> AsyncPipeWrite {
    let owned: std::os::windows::io::OwnedHandle = self.file.into();
    let stdin = std::process::ChildStdin::from(owned);
    AsyncPipeWrite { write: tokio::process::ChildStdin::from_std(stdin).unwrap() }
  }
  #[cfg(not(windows))]
  pub fn into_async(self) -> AsyncPipeWrite {
    AsyncPipeWrite { write: tokio::net::unix::pipe::Sender::from_file(self.file).unwrap() }
  }
}

impl AsyncPipeWrite {
  #[cfg(windows)]
  pub fn into_sync(self) -> PipeWrite {
    let owned = self.write.into_owned_handle().unwrap();
    PipeWrite { file: owned.into() }
  }
  #[cfg(not(windows))]
  pub fn into_sync(self) -> PipeWrite {
    let file = self.write.into_nonblocking_fd().unwrap().into();
    PipeWrite { file }
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
  fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), io::Error>> {
    Pin::new(&mut self.get_mut().write).poll_flush(cx)
  }

  #[inline(always)]
  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), io::Error>> {
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

/// Create a unidirectional pipe pair that starts off as a pair of synchronous file handles,
/// but either side may be promoted to an async-capable reader/writer.
pub fn pipe() -> io::Result<(PipeRead, PipeWrite)> {
  pipe_impl()
}

/// Creates a unidirectional pipe on top of a naned pipe (which is technically bidirectional).
#[cfg(windows)]
pub fn pipe_impl() -> io::Result<(PipeRead, PipeWrite)> {
  // SAFETY: We're careful with handles here
  unsafe {
    use std::os::windows::io::OwnedHandle;
    use std::os::windows::io::FromRawHandle;
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

  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;
  use std::io::Read;
  use std::io::Write;
  
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
    let mut read = read.into_async();
    let mut write = write.into_async();

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

    let mut read = read.into_async();
    let mut write = write.into_async();

    // Async
    write.write_all(b"hello").await.unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"hello");

    let mut read = read.into_sync();
    let mut write = write.into_sync();

    // Sync
    write.write_all(b"hello").unwrap();
    let mut buf: [u8; 5] = Default::default();
    read.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");
  }

  #[tokio::test]
  async fn test_async_pipe_is_nonblocking() {
    let (read, write) = pipe().unwrap();
    let mut read = read.into_async();
    let mut write = write.into_async();

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
