// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io::IoSlice;
use std::io::IoSliceMut;
use std::io::{self};
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use tokio::io::unix::AsyncFd;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::os::fd::FromRawFd;
use std::rc::Rc;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

struct IpcPipe {
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

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, std::io::Error> {
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

struct IpcJsonStream {
  pipe: Rc<IpcPipe>,
  buffer: RefCell<Vec<u8>>,
  cancel: CancelHandle,
}

impl IpcJsonStream {
  fn new(fd: RawFd) -> Result<Self, std::io::Error> {
    Ok(Self {
      pipe: Rc::new(IpcPipe::new(fd)?),
      buffer: RefCell::new(Vec::new()),
      cancel: CancelHandle::default(),
    })
  }

  async fn read(self: Rc<Self>) -> Result<Vec<serde_json::Value>, AnyError> {
    let mut buf = [0u8; 1024]; // TODO: Use a single growable buffer.
    let mut msgs = Vec::new();
    loop {
      let n = self.pipe.read(&mut buf).await?;

      let read = &buf[..n];
      let mut chunk_boundary = 0;

      for byte in read {
        if *byte == b'\n' {
          let chunk = &read[..chunk_boundary];
          self.buffer.borrow_mut().extend_from_slice(chunk);

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

  async fn write(
    self: Rc<Self>,
    msg: serde_json::Value,
  ) -> Result<(), AnyError> {
    let mut buf = Vec::new();
    serde_json::to_writer(&mut buf, &msg)?;
    buf.push(b'\n');
    self.pipe.clone().write(&buf).await?;
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
  stream.write(value).await?;
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
  let msgs = stream.read().or_cancel(cancel).await??;
  Ok(msgs)
}
