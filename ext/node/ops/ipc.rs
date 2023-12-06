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
use deno_core::OpState;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::rc::Rc;

fn sendmsg(fd: RawFd, data: &[u8]) -> Result<usize, std::io::Error> {
  let iov = [IoSlice::new(&data)];
  loop {
    match nix::sys::socket::sendmsg::<()>(
      fd,
      &iov,
      &[],
      nix::sys::socket::MsgFlags::empty(),
      None,
    ) {
      Ok(n) => {
        if n == 0 {
          return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "could not send",
          ));
        }
        return Ok(n);
      }
      Err(nix::errno::Errno::EINTR) => continue,
      Err(e) => return Err(e.into()),
    }
  }
}

fn readmsg(fd: RawFd, data: &mut [u8]) -> Result<usize, std::io::Error> {
  let mut iov = [IoSliceMut::new(data)];
  loop {
    match nix::sys::socket::recvmsg::<()>(
      fd,
      &mut iov,
      None,
      nix::sys::socket::MsgFlags::empty(),
    ) {
      Ok(n) => {
        if n.bytes == 0 {
          return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "could not read",
          ));
        }
        return Ok(n.bytes);
      }
      Err(nix::errno::Errno::EINTR) => continue,
      Err(e) => return Err(e.into()),
    }
  }
}

struct IpcPipe {
  inner: AsyncFd<RawFd>,
}

impl IpcPipe {
  fn new(fd: RawFd) -> Result<Self, std::io::Error> {
    Ok(Self {
      inner: AsyncFd::new(fd)?,
    })
  }

  async fn write(self: Rc<Self>, data: &[u8]) -> Result<usize, std::io::Error> {
    let mut offset = 0;
    loop {
      let mut guard = self.inner.writable().await?;
      match guard.try_io(|inner| sendmsg(inner.as_raw_fd(), &data[offset..])) {
        Ok(Ok(n)) => {
          offset += n;
          if offset >= data.len() {
            return Ok(offset);
          }
        }
        Ok(Err(e)) => return Err(e),
        Err(_) => continue,
      }
    }
  }

  async fn read(&self, data: &mut [u8]) -> Result<usize, std::io::Error> {
    let mut offset = 0;
    loop {
      let mut guard = self.inner.readable().await?;
      match guard
        .try_io(|inner| readmsg(inner.as_raw_fd(), &mut data[offset..]))
      {
        Ok(Ok(n)) => {
          offset += n;
          if offset >= data.len() {
            return Ok(offset);
          }
        }
        Ok(Err(e)) => return Err(e),
        Err(_) => continue,
      }
    }
  }
}

struct IpcJsonStream {
  pipe: Rc<IpcPipe>,
  buffer: RefCell<Vec<u8>>,
}

impl IpcJsonStream {
  fn new(fd: RawFd) -> Result<Self, std::io::Error> {
    Ok(Self {
      pipe: Rc::new(IpcPipe::new(fd)?),
      buffer: RefCell::new(Vec::new()),
    })
  }

  async fn read(self: Rc<Self>) -> Result<Vec<serde_json::Value>, AnyError> {
    let mut buf = [0u8; 1024];
    let mut msgs = Vec::new();
    loop {
      println!("reading");
      let n = self.pipe.read(&mut buf).await?;
      println!("read {} bytes", n);

      let read = &buf[..n];
      let mut chunk_boundary = 0;
      for byte in read {
        if *byte == b'\n' {
          let chunk = &read[chunk_boundary..];
          self.buffer.borrow_mut().extend_from_slice(chunk);
          chunk_boundary = 0;
          if chunk.is_empty() {
            break;
          }
          msgs.push(serde_json::from_slice(&self.buffer.borrow())?);
          self.buffer.borrow_mut().clear();
        } else {
          chunk_boundary += 1;
        }
      }

      if chunk_boundary > 0 {
        self.buffer.borrow_mut().clear();
        self
          .buffer
          .borrow_mut()
          .extend_from_slice(&read[..chunk_boundary]);
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

impl deno_core::Resource for IpcJsonStream {}

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
  let msgs = stream.read().await?;
  Ok(msgs)
}
