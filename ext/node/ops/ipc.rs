// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#[cfg(unix)]
pub use unix::*;

#[cfg(windows)]
pub use windows::*;

pub struct ChildPipeFd(pub i32);

#[cfg(unix)]
mod unix {
  use std::cell::RefCell;
  use std::future::Future;
  use std::io;
  use std::mem;
  use std::os::fd::FromRawFd;
  use std::os::fd::RawFd;
  use std::pin::Pin;
  use std::rc::Rc;
  use std::task::Context;
  use std::task::Poll;

  use deno_core::error::bad_resource_id;
  use deno_core::error::AnyError;
  use deno_core::op2;
  use deno_core::serde_json;
  use deno_core::AsyncRefCell;
  use deno_core::CancelFuture;
  use deno_core::CancelHandle;
  use deno_core::OpState;
  use deno_core::RcRef;
  use deno_core::ResourceId;
  use pin_project_lite::pin_project;
  use tokio::io::AsyncBufRead;
  use tokio::io::AsyncWriteExt;
  use tokio::io::BufReader;
  use tokio::net::unix::OwnedReadHalf;
  use tokio::net::unix::OwnedWriteHalf;
  use tokio::net::UnixStream;

  #[op2(fast)]
  #[smi]
  pub fn op_node_ipc_pipe(
    state: &mut OpState,
    #[smi] fd: i32,
  ) -> Result<ResourceId, AnyError> {
    Ok(state.resource_table.add(IpcJsonStreamResource::new(fd)?))
  }

  // Open IPC pipe from bootstrap options.
  #[op2]
  #[smi]
  pub fn op_node_child_ipc_pipe(
    state: &mut OpState,
  ) -> Result<Option<ResourceId>, AnyError> {
    let fd = match state.try_borrow_mut::<crate::ChildPipeFd>() {
      Some(child_pipe_fd) => child_pipe_fd.0,
      None => return Ok(None),
    };

    Ok(Some(
      state.resource_table.add(IpcJsonStreamResource::new(fd)?),
    ))
  }

  #[op2(async)]
  pub async fn op_node_ipc_write(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
    #[serde] value: serde_json::Value,
  ) -> Result<(), AnyError> {
    let stream = state
      .borrow()
      .resource_table
      .get::<IpcJsonStreamResource>(rid)
      .map_err(|_| bad_resource_id())?;
    stream.write_msg(value).await?;
    Ok(())
  }

  #[op2(async)]
  #[serde]
  pub async fn op_node_ipc_read(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
  ) -> Result<serde_json::Value, AnyError> {
    let stream = state
      .borrow()
      .resource_table
      .get::<IpcJsonStreamResource>(rid)
      .map_err(|_| bad_resource_id())?;

    let cancel = stream.cancel.clone();
    let mut stream = RcRef::map(stream, |r| &r.read_half).borrow_mut().await;
    let msgs = stream.read_msg().or_cancel(cancel).await??;
    Ok(msgs)
  }

  struct IpcJsonStreamResource {
    read_half: AsyncRefCell<IpcJsonStream>,
    write_half: AsyncRefCell<OwnedWriteHalf>,
    cancel: Rc<CancelHandle>,
  }

  impl deno_core::Resource for IpcJsonStreamResource {
    fn close(self: Rc<Self>) {
      self.cancel.cancel();
    }
  }

  impl IpcJsonStreamResource {
    fn new(stream: RawFd) -> Result<Self, std::io::Error> {
      // Safety: The fd is part of a pair of connected sockets create by child process
      // implementation.
      let unix_stream = UnixStream::from_std(unsafe {
        std::os::unix::net::UnixStream::from_raw_fd(stream)
      })?;
      let (read_half, write_half) = unix_stream.into_split();
      Ok(Self {
        read_half: AsyncRefCell::new(IpcJsonStream::new(read_half)),
        write_half: AsyncRefCell::new(write_half),
        cancel: Default::default(),
      })
    }

    #[cfg(test)]
    fn from_unix_stream(stream: UnixStream) -> Self {
      let (read_half, write_half) = stream.into_split();
      Self {
        read_half: AsyncRefCell::new(IpcJsonStream::new(read_half)),
        write_half: AsyncRefCell::new(write_half),
        cancel: Default::default(),
      }
    }

    async fn write_msg(
      self: Rc<Self>,
      msg: serde_json::Value,
    ) -> Result<(), AnyError> {
      let mut write_half =
        RcRef::map(self, |r| &r.write_half).borrow_mut().await;
      // Perf note: We do not benefit from writev here because
      // we are always allocating a buffer for serialization anyways.
      let mut buf = Vec::new();
      serde_json::to_writer(&mut buf, &msg)?;
      buf.push(b'\n');
      write_half.write_all(&buf).await?;
      Ok(())
    }
  }

  #[inline]
  fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    // Safety: haystack of valid length. neon_memchr can handle unaligned
    // data.
    return unsafe { neon::neon_memchr(haystack, needle, haystack.len()) };

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    return haystack.iter().position(|&b| b == needle);
  }

  // Initial capacity of the buffered reader and the JSON backing buffer.
  //
  // This is a tradeoff between memory usage and performance on large messages.
  //
  // 64kb has been chosen after benchmarking 64 to 66536 << 6 - 1 bytes per message.
  const INITIAL_CAPACITY: usize = 1024 * 64;

  // JSON serialization stream over IPC pipe.
  //
  // `\n` is used as a delimiter between messages.
  struct IpcJsonStream {
    pipe: BufReader<OwnedReadHalf>,
    buffer: Vec<u8>,
  }

  impl IpcJsonStream {
    fn new(pipe: OwnedReadHalf) -> Self {
      Self {
        pipe: BufReader::with_capacity(INITIAL_CAPACITY, pipe),
        buffer: Vec::with_capacity(INITIAL_CAPACITY),
      }
    }

    async fn read_msg(&mut self) -> Result<serde_json::Value, AnyError> {
      let mut json = None;
      let nread =
        read_msg_inner(&mut self.pipe, &mut self.buffer, &mut json).await?;
      if nread == 0 {
        // EOF.
        return Ok(serde_json::Value::Null);
      }

      let json = match json {
        Some(v) => v,
        None => {
          // Took more than a single read and some buffering.
          simd_json::from_slice(&mut self.buffer[..nread])?
        }
      };

      // Safety: Same as `Vec::clear` but without the `drop_in_place` for
      // each element (nop for u8). Capacity remains the same.
      unsafe {
        self.buffer.set_len(0);
      }

      Ok(json)
    }
  }

  pin_project! {
      #[must_use = "futures do nothing unless you `.await` or poll them"]
      struct ReadMsgInner<'a, R: ?Sized> {
          reader: &'a mut R,
          buf: &'a mut Vec<u8>,
          json: &'a mut Option<serde_json::Value>,
          // The number of bytes appended to buf. This can be less than buf.len() if
          // the buffer was not empty when the operation was started.
          read: usize,
      }
  }

  fn read_msg_inner<'a, R>(
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
    json: &'a mut Option<serde_json::Value>,
  ) -> ReadMsgInner<'a, R>
  where
    R: AsyncBufRead + ?Sized + Unpin,
  {
    ReadMsgInner {
      reader,
      buf,
      json,
      read: 0,
    }
  }

  fn read_msg_internal<R: AsyncBufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
    json: &mut Option<serde_json::Value>,
    read: &mut usize,
  ) -> Poll<io::Result<usize>> {
    loop {
      let (done, used) = {
        let available = match reader.as_mut().poll_fill_buf(cx) {
          std::task::Poll::Ready(t) => t?,
          std::task::Poll::Pending => return std::task::Poll::Pending,
        };

        if let Some(i) = memchr(b'\n', available) {
          if *read == 0 {
            // Fast path: parse and put into the json slot directly.
            //
            // Safety: It is ok to overwrite the  contents because
            // we don't need to copy it into the buffer and the length will be reset.
            let available = unsafe {
              std::slice::from_raw_parts_mut(
                available.as_ptr() as *mut u8,
                available.len(),
              )
            };
            json.replace(
              simd_json::from_slice(&mut available[..i + 1])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
          } else {
            // This is not the first read, so we have to copy the data
            // to make it contiguous.
            buf.extend_from_slice(&available[..=i]);
          }
          (true, i + 1)
        } else {
          buf.extend_from_slice(available);
          (false, available.len())
        }
      };

      reader.as_mut().consume(used);
      *read += used;
      if done || used == 0 {
        return Poll::Ready(Ok(mem::replace(read, 0)));
      }
    }
  }

  impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadMsgInner<'_, R> {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      let me = self.project();
      read_msg_internal(Pin::new(*me.reader), cx, me.buf, me.json, me.read)
    }
  }

  #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
  mod neon {
    use std::arch::aarch64::*;

    pub unsafe fn neon_memchr(
      str: &[u8],
      c: u8,
      length: usize,
    ) -> Option<usize> {
      let end = str.as_ptr().wrapping_add(length);

      // Alignment handling
      let mut ptr = str.as_ptr();
      while ptr < end && (ptr as usize) & 0xF != 0 {
        if *ptr == c {
          return Some(ptr as usize - str.as_ptr() as usize);
        }
        ptr = ptr.wrapping_add(1);
      }

      let search_char = vdupq_n_u8(c);

      while ptr.wrapping_add(16) <= end {
        let chunk = vld1q_u8(ptr);
        let comparison = vceqq_u8(chunk, search_char);

        // Check first 64 bits
        let result0 = vgetq_lane_u64(vreinterpretq_u64_u8(comparison), 0);
        if result0 != 0 {
          return Some(
            (ptr as usize - str.as_ptr() as usize)
              + result0.trailing_zeros() as usize / 8,
          );
        }

        // Check second 64 bits
        let result1 = vgetq_lane_u64(vreinterpretq_u64_u8(comparison), 1);
        if result1 != 0 {
          return Some(
            (ptr as usize - str.as_ptr() as usize)
              + 8
              + result1.trailing_zeros() as usize / 8,
          );
        }

        ptr = ptr.wrapping_add(16);
      }

      // Handle remaining unaligned characters
      while ptr < end {
        if *ptr == c {
          return Some(ptr as usize - str.as_ptr() as usize);
        }
        ptr = ptr.wrapping_add(1);
      }

      None
    }
  }

  #[cfg(test)]
  mod tests {
    use super::IpcJsonStreamResource;
    use deno_core::serde_json;
    use deno_core::serde_json::json;
    use deno_core::RcRef;
    use std::rc::Rc;

    #[tokio::test]
    async fn bench_ipc() -> Result<(), Box<dyn std::error::Error>> {
      // A simple round trip benchmark for quick dev feedback.
      //
      // Only ran when the env var is set.
      if std::env::var_os("BENCH_IPC_DENO").is_none() {
        return Ok(());
      }

      let (fd1, mut fd2) = tokio::net::UnixStream::pair()?;
      let child = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;

        let size = 1024 * 1024;

        let stri = "x".repeat(size);
        let data = format!("\"{}\"\n", stri);
        for _ in 0..100 {
          fd2.write_all(data.as_bytes()).await?;
        }
        Ok::<_, std::io::Error>(())
      });

      let ipc = Rc::new(IpcJsonStreamResource::from_unix_stream(fd1));

      let start = std::time::Instant::now();
      let mut bytes = 0;

      let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
      loop {
        let msgs = ipc.read_msg().await?;
        if msgs == serde_json::Value::Null {
          break;
        }
        bytes += msgs.as_str().unwrap().len();
        if start.elapsed().as_secs() > 5 {
          break;
        }
      }
      let elapsed = start.elapsed();
      let mb = bytes as f64 / 1024.0 / 1024.0;
      println!("{} mb/s", mb / elapsed.as_secs_f64());

      child.await??;

      Ok(())
    }

    #[tokio::test]
    async fn unix_ipc_json() -> Result<(), Box<dyn std::error::Error>> {
      let (fd1, mut fd2) = tokio::net::UnixStream::pair()?;
      let child = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncWriteExt;

        let mut buf = [0u8; 1024];
        let n = fd2.read(&mut buf).await?;
        assert_eq!(&buf[..n], b"\"hello\"\n");
        fd2.write_all(b"\"world\"\n").await?;
        Ok::<_, std::io::Error>(())
      });

      /* Similar to how ops would use the resource */
      let ipc = Rc::new(IpcJsonStreamResource::from_unix_stream(fd1));

      ipc.clone().write_msg(json!("hello")).await?;

      let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
      let msgs = ipc.read_msg().await?;
      assert_eq!(msgs, json!("world"));

      child.await??;

      Ok(())
    }

    #[tokio::test]
    async fn unix_ipc_json_multi() -> Result<(), Box<dyn std::error::Error>> {
      let (fd1, mut fd2) = tokio::net::UnixStream::pair()?;
      let child = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncWriteExt;

        let mut buf = [0u8; 1024];
        let n = fd2.read(&mut buf).await?;
        assert_eq!(&buf[..n], b"\"hello\"\n\"world\"\n");
        fd2.write_all(b"\"foo\"\n\"bar\"\n").await?;
        Ok::<_, std::io::Error>(())
      });

      let ipc = Rc::new(IpcJsonStreamResource::from_unix_stream(fd1));
      ipc.clone().write_msg(json!("hello")).await?;
      ipc.clone().write_msg(json!("world")).await?;

      let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
      let msgs = ipc.read_msg().await?;
      assert_eq!(msgs, json!("foo"));

      child.await??;

      Ok(())
    }

    #[tokio::test]
    async fn unix_ipc_json_invalid() -> Result<(), Box<dyn std::error::Error>> {
      let (fd1, mut fd2) = tokio::net::UnixStream::pair()?;
      let child = tokio::spawn(async move {
        tokio::io::AsyncWriteExt::write_all(&mut fd2, b"\n\n").await?;
        Ok::<_, std::io::Error>(())
      });

      let ipc = Rc::new(IpcJsonStreamResource::from_unix_stream(fd1));
      let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
      let _err = ipc.read_msg().await.unwrap_err();

      child.await??;

      Ok(())
    }

    #[test]
    fn memchr() {
      let str = b"hello world";
      assert_eq!(super::memchr(b'h', str), Some(0));
      assert_eq!(super::memchr(b'w', str), Some(6));
      assert_eq!(super::memchr(b'd', str), Some(10));
      assert_eq!(super::memchr(b'x', str), None);

      let empty = b"";
      assert_eq!(super::memchr(b'\n', empty), None);
    }
  }
}

#[cfg(windows)]
mod windows {
  use deno_core::error::AnyError;
  use deno_core::op2;

  #[op2(fast)]
  pub fn op_node_ipc_pipe() -> Result<(), AnyError> {
    Err(deno_core::error::not_supported())
  }

  #[op2(fast)]
  #[smi]
  pub fn op_node_child_ipc_pipe() -> Result<i32, AnyError> {
    Ok(-1)
  }

  #[op2(async)]
  pub async fn op_node_ipc_write() -> Result<(), AnyError> {
    Err(deno_core::error::not_supported())
  }

  #[op2(async)]
  pub async fn op_node_ipc_read() -> Result<(), AnyError> {
    Err(deno_core::error::not_supported())
  }
}
