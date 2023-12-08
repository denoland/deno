// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::io;
use std::os::fd::FromRawFd;
use std::os::fd::RawFd;
use std::rc::Rc;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::ResourceId;
use tokio::net::UnixStream;

struct IpcPipe {
  // Better name?
  inner: UnixStream,
}

impl IpcPipe {
  fn new(fd: RawFd) -> Result<Self, std::io::Error> {
    Ok(Self {
      inner: UnixStream::from_std(unsafe {
        std::os::unix::net::UnixStream::from_raw_fd(fd)
      })?,
    })
  }

  async fn write(&self, data: &[u8]) -> Result<usize, std::io::Error> {
    let mut offset = 0;
    loop {
      self.inner.writable().await?;
      match self.inner.try_write(&data[offset..]) {
        Ok(n) => {
          offset += n;
          if offset >= data.len() {
            return Ok(offset);
          }
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
          continue;
        }
        Err(e) => return Err(e),
      }
    }
  }

  async fn read(&self, data: &mut [u8]) -> Result<usize, std::io::Error> {
    loop {
      self.inner.readable().await?;
      match self.inner.try_read(&mut data[..]) {
        Ok(n) => {
          return Ok(n);
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
          continue;
        }
        Err(e) => return Err(e),
      }
    }
  }
}

// JSON serialization stream over IPC pipe.
//
// `\n` is used as a delimiter between messages.
struct IpcJsonStream {
  pipe: IpcPipe,
  buffer: RefCell<Vec<u8>>,
  cancel: CancelHandle,
}

impl IpcJsonStream {
  fn new(fd: RawFd) -> Result<Self, std::io::Error> {
    Ok(Self {
      pipe: IpcPipe::new(fd)?,
      buffer: RefCell::new(Vec::new()),
      cancel: CancelHandle::default(),
    })
  }

  async fn read_msgs(&self) -> Result<Vec<serde_json::Value>, AnyError> {
    let mut buf = [0u8; 1024]; // TODO: Use a single growable buffer.
    let mut msgs = Vec::new();
    loop {
      let n = self.pipe.read(&mut buf).await?;

      let mut read = &buf[..n];
      let mut chunk_boundary = 0;
      for byte in read {
        if *byte == b'\n'
            /* Ignore empty messages otherwise we enter infinite loop with `\n\n` */
            && chunk_boundary > 0
        {
          let chunk = &read[..chunk_boundary];
          self.buffer.borrow_mut().extend_from_slice(chunk);
          read = &read[chunk_boundary + 1..];
          chunk_boundary = 0;
          if chunk.is_empty() {
            // Last chunk.
            break;
          }
          msgs.push(serde_json::from_slice(&self.buffer.borrow())?);
          self.buffer.borrow_mut().clear();
        } else {
          chunk_boundary += 1;
        }
      }

      if chunk_boundary > 0 {
        let buffer = &mut self.buffer.borrow_mut();
        buffer.clear();
        buffer.extend_from_slice(&read[..chunk_boundary]);
      }

      if !msgs.is_empty() {
        return Ok(msgs);
      }
    }
  }

  async fn write_msg(&self, msg: serde_json::Value) -> Result<(), AnyError> {
    // Perf note: We do not benefit from writev here because
    // we are always allocating a buffer for serialization anyways.
    let mut buf = Vec::new();
    serde_json::to_writer(&mut buf, &msg)?;
    buf.push(b'\n');
    self.pipe.write(&buf).await?;
    Ok(())
  }
}

impl deno_core::Resource for IpcJsonStream {
  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[op2(fast)]
#[smi]
pub fn op_node_ipc_pipe(
  state: &mut OpState,
  #[smi] fd: i32,
) -> Result<ResourceId, AnyError> {
  Ok(state.resource_table.add(IpcJsonStream::new(fd)?))
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
    .get::<IpcJsonStream>(rid)
    .map_err(|_| bad_resource_id())?;
  stream.write_msg(value).await?;
  Ok(())
}

#[op2(async)]
#[serde]
pub async fn op_node_ipc_read(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Vec<serde_json::Value>, AnyError> {
  let stream = state
    .borrow()
    .resource_table
    .get::<IpcJsonStream>(rid)
    .map_err(|_| bad_resource_id())?;

  let cancel = RcRef::map(stream.clone(), |r| &r.cancel);
  let msgs = stream.read_msgs().or_cancel(cancel).await??;
  Ok(msgs)
}

#[cfg(test)]
mod tests {
  use super::IpcJsonStream;
  use super::IpcPipe;
  use deno_core::serde_json::json;
  use std::os::unix::io::AsRawFd;
  use std::rc::Rc;

  #[tokio::test]
  async fn unix_ipc_raw() -> Result<(), Box<dyn std::error::Error>> {
    let (fd1, mut fd2) = tokio::net::UnixStream::pair()?;
    let child = tokio::spawn(async move {
      use tokio::io::AsyncReadExt;
      use tokio::io::AsyncWriteExt;

      let mut buf = [0u8; 1024];
      let n = fd2.read(&mut buf).await?;
      assert_eq!(&buf[..n], b"hello");
      fd2.write_all(b"world").await?;
      Ok::<_, std::io::Error>(())
    });

    /* Similar to how ops would use the resource */
    let ipc = Rc::new(IpcPipe::new(fd1.as_raw_fd())?);
    ipc.write(b"hello").await?;
    let mut buf = [0u8; 1024];
    let n = ipc.read(&mut buf).await?;
    assert_eq!(&buf[..n], b"world");

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
    let ipc = Rc::new(IpcJsonStream::new(fd1.as_raw_fd())?);
    ipc.write_msg(json!("hello")).await?;
    let msgs = ipc.read_msgs().await?;
    assert_eq!(msgs, vec![json!("world")]);

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

    let ipc = Rc::new(IpcJsonStream::new(fd1.as_raw_fd())?);
    ipc.write_msg(json!("hello")).await?;
    ipc.write_msg(json!("world")).await?;
    let msgs = ipc.read_msgs().await?;
    assert_eq!(msgs, vec![json!("foo"), json!("bar")]);

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

    let ipc = Rc::new(IpcJsonStream::new(fd1.as_raw_fd())?);
    let err = ipc.read_msgs().await.unwrap_err();
    assert!(err.is::<deno_core::serde_json::Error>());

    child.await??;

    Ok(())
  }
}
