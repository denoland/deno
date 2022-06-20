use super::tty::ConsoleSize;
use deno_core::{op, error::AnyError, Extension, ResourceId, OpState};
#[cfg(unix)]
use std::{
  io::Error as IoError,
  os::unix::prelude::RawFd,
  ptr::null_mut,
};
use crate::ops::io::StdFileResource;

pub fn init() -> Extension {
  Extension::builder().ops(vec![op_pty_open::decl()]).build()
}

pub struct Pty {
  #[cfg(unix)]
  pub master_fd: RawFd,
  #[cfg(unix)]
  pub slave_fd: RawFd,
}

#[cfg(unix)]
impl Pty {
  pub fn new(size: ConsoleSize) -> Result<Pty, IoError> {
    let mut pty = Pty {
      master_fd: -1,
      slave_fd: -1,
    };
    let mut size = libc::winsize {
      ws_row: size.rows as u16,
      ws_col: size.columns as u16,
      ws_ypixel: 0,
      ws_xpixel: 0,
    };
    if unsafe {
      libc::openpty(
        &mut pty.master_fd,
        &mut pty.slave_fd,
        null_mut(),
        null_mut(),
        &mut size,
      )
    } != 0
    {
      Err(IoError::last_os_error())
    } else {
      Ok(pty)
    }
  }

  #[cfg(unix)]
  pub fn read(&self, buf: &mut [u8]) -> Result<usize, IoError> {
    let size = unsafe { libc::read(self.master_fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    if size == -1 {
      Err(IoError::last_os_error())
    } else {
      println!("{}", self.master_fd);
      Ok(size as usize)
    }
  }

  #[cfg(unix)]
  pub fn write(&self, buf: &[u8]) -> Result<usize, IoError> {
    let size = unsafe { libc::write(self.master_fd, buf.as_ptr() as *const _, buf.len()) };
    if size == -1 {
      Err(IoError::last_os_error())
    } else {
      Ok(size as usize)
    }
  }

  #[cfg(unix)]
  pub fn close(&mut self) {
    unsafe {
      libc::close(self.slave_fd);
      libc::close(self.master_fd);
    }
    self.slave_fd = -1;
    self.master_fd = -1;
  }
}

impl Clone for Pty {
  fn clone(&self) -> Self {
    Pty {
      master_fd: self.master_fd.clone(),
      slave_fd: self.slave_fd.clone(),
    }
  }
}

#[op]
fn op_pty_open(
  state: &mut OpState,
  args: ConsoleSize,
) -> Result<ResourceId, AnyError> {
  super::check_unstable(state, "Deno.openPty");
  let pty = Pty::new(args)?;
  let resource = StdFileResource::pty(pty);
  let rid = state.resource_table.add(resource);
  Ok(rid)
}
