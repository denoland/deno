// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::os::fd::FromRawFd;
use std::os::fd::RawFd;
use std::rc::Rc;

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
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedReadHalf;
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::UnixStream;

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

  async fn write_msg(
    self: Rc<Self>,
    msg: serde_json::Value,
  ) -> Result<(), AnyError> {
    let mut write_half = RcRef::map(self, |r| &r.write_half).borrow_mut().await;
    // Perf note: We do not benefit from writev here because
    // we are always allocating a buffer for serialization anyways.
    let mut buf = Vec::new();
    serde_json::to_writer(&mut buf, &msg)?;
    buf.push(b'\n');
    write_half.write_all(&buf).await?;
    Ok(())
  }
}

// JSON serialization stream over IPC pipe.
//
// `\n` is used as a delimiter between messages.
struct IpcJsonStream {
  pipe: OwnedReadHalf,
  buffer: RefCell<Vec<u8>>,
}

impl IpcJsonStream {
  fn new(pipe: OwnedReadHalf) -> Self {
    Self {
      pipe,
      buffer: RefCell::new(Vec::new()),
    }
  }

  async fn read_msgs(&mut self) -> Result<Vec<serde_json::Value>, AnyError> {
    let mut buf = [0u8; 1024]; // TODO: Use a single growable buffer.
    let mut msgs = Vec::new();
    loop {
      let n = self.pipe.read(&mut buf).await?;
      if n == 0 {
        break Ok(vec![]); // EOF
      }

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
}

#[op2(fast)]
#[smi]
pub fn op_node_ipc_pipe(
  state: &mut OpState,
  #[smi] fd: i32,
) -> Result<ResourceId, AnyError> {
  Ok(state.resource_table.add(IpcJsonStreamResource::new(fd)?))
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
) -> Result<Vec<serde_json::Value>, AnyError> {
  let stream = state
    .borrow()
    .resource_table
    .get::<IpcJsonStreamResource>(rid)
    .map_err(|_| bad_resource_id())?;

  let cancel = stream.cancel.clone();
  let mut stream = RcRef::map(stream, |r| &r.read_half).borrow_mut().await;
  let msgs = stream.read_msgs().or_cancel(cancel).await??;
  Ok(msgs)
}

#[cfg(test)]
mod tests {
  use super::IpcJsonStreamResource;
  use deno_core::serde_json::json;
  use deno_core::RcRef;
  use std::os::unix::io::AsRawFd;
  use std::rc::Rc;

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
    let ipc = Rc::new(IpcJsonStreamResource::new(fd1.as_raw_fd())?);

    ipc.clone().write_msg(json!("hello")).await?;

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
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

    let ipc = Rc::new(IpcJsonStreamResource::new(fd1.as_raw_fd())?);
    ipc.clone().write_msg(json!("hello")).await?;
    ipc.clone().write_msg(json!("world")).await?;

    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
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

    let ipc = Rc::new(IpcJsonStreamResource::new(fd1.as_raw_fd())?);
    let mut ipc = RcRef::map(ipc, |r| &r.read_half).borrow_mut().await;
    let err = ipc.read_msgs().await.unwrap_err();
    assert!(err.is::<deno_core::serde_json::Error>());

    child.await??;

    Ok(())
  }
}
