use std::io::IoSliceMut;
use std::io::{self, IoSlice};
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use tokio::io::unix::AsyncFd;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::ResourceId;
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

impl deno_core::Resource for IpcPipe {
  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();
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

  async fn read(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, std::io::Error> {
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

#[op2(fast)]
#[smi]
pub fn op_node_ipc_pipe(
  state: &mut OpState,
  #[smi] fd: i32,
) -> Result<ResourceId, AnyError> {
  Ok(state.resource_table.add(IpcPipe::new(fd)?))
}
