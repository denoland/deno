// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use self::pty::ConsoleSize;
use self::pty::Pty;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{Extension, Resource};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Read;
use std::io::Write;
use std::os::unix::prelude::RawFd;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init() -> Extension {
  Extension::builder().ops(vec![op_pty_open::decl()]).build()
}

pub mod pty {
  #[cfg(unix)]
  use libc::{
    self, ioctl, openpty, read, winsize, write, TIOCGWINSZ, TIOCSWINSZ,
  };
  #[cfg(unix)]
  use std::{
    io::Error as IoError,
    mem::zeroed,
    os::unix::prelude::{FromRawFd, RawFd},
    process::Stdio,
    ptr::null_mut,
  };

  pub struct ConsoleSize {
    pub rows: u16,
    pub columns: u16,
  }
  pub struct Pty {
    #[cfg(unix)]
    pub master_fd: RawFd,
    #[cfg(unix)]
    pub slave_fd: RawFd,
  }

  impl Pty {
    #[cfg(unix)]
    pub fn new(size: ConsoleSize) -> Result<Pty, IoError> {
      let mut pty = Pty {
        master_fd: -1,
        slave_fd: -1,
      };
      let mut size = winsize {
        ws_row: size.rows,
        ws_col: size.columns,
        ws_xpixel: 0,
        ws_ypixel: 0,
      };
      if unsafe {
        openpty(
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
    pub fn resize(&self, size: ConsoleSize) -> Result<(), IoError> {
      let size = winsize {
        ws_row: size.rows,
        ws_col: size.columns,
        ws_xpixel: 0,
        ws_ypixel: 0,
      };
      if unsafe { ioctl(self.master_fd, TIOCSWINSZ as _, &size as *const _) }
        != 0
      {
        Err(IoError::last_os_error())
      } else {
        Ok(())
      }
    }

    #[cfg(unix)]
    pub fn get_size(&self) -> Result<ConsoleSize, IoError> {
      let mut size: winsize = unsafe { zeroed() };
      if unsafe { ioctl(self.master_fd, TIOCGWINSZ as _, &mut size as *mut _) }
        != 0
      {
        Err(IoError::last_os_error())
      } else {
        Ok(ConsoleSize {
          rows: size.ws_row,
          columns: size.ws_col,
        })
      }
    }

    #[cfg(unix)]
    pub fn read(fd: RawFd, buf: &mut [u8]) -> Result<usize, IoError> {
      let size = unsafe { read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
      if size == -1 {
        Err(IoError::last_os_error())
      } else {
        Ok(size as usize)
      }
    }

    #[cfg(unix)]
    pub fn read_sync(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
      Self::read(self.master_fd, buf)
    }

    #[cfg(unix)]
    pub fn write(fd: RawFd, buf: &[u8]) -> Result<usize, IoError> {
      let size = unsafe { write(fd, buf.as_ptr() as *const _, buf.len()) };
      if size == -1 {
        Err(IoError::last_os_error())
      } else {
        Ok(size as usize)
      }
    }

    #[cfg(unix)]
    pub fn write_sync(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
      Self::write(self.master_fd, buf)
    }

    #[cfg(unix)]
    pub fn as_stdio(&self) -> std::process::Stdio {
      unsafe { std::process::Stdio::from_raw_fd(self.master_fd) }
    }

    #[cfg(unix)]
    pub fn close(&mut self) {
      unsafe {
        libc::close(self.slave_fd);
        libc::close(self.master_fd);
      }
      self.master_fd = -1;
      self.slave_fd = -1;
    }
  }
}

struct FdReader {
  fd: RawFd,
}
struct FdWriter {
  fd: RawFd,
}

impl Read for FdReader {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    pty::Pty::read(self.fd, buf)
  }
}

impl Write for FdWriter {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    pty::Pty::write(self.fd, buf)
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

pub struct PtyResource {
  pub pty: RefCell<Pty>,
  reader: Arc<Mutex<Box<dyn Read + Send>>>,
  writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl PtyResource {
  fn new(pty: Pty) -> PtyResource {
    let fd = pty.master_fd.clone();
    PtyResource {
      pty: RefCell::new(pty),
      reader: Arc::new(Mutex::new(Box::new(FdReader { fd }))),
      writer: Arc::new(Mutex::new(Box::new(FdWriter { fd }))),
    }
  }
}

impl Resource for PtyResource {
  fn name(&self) -> Cow<str> {
    return "pty".into();
  }

  #[cfg(unix)]
  fn close(self: Rc<Self>) {
    if self.pty.borrow().master_fd == -1 {
      // Already closed.
      return;
    }
    self.pty.borrow_mut().close();
  }

  #[cfg(unix)]
  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    let reader = self.reader.clone();
    Box::pin(async move {
      tokio::task::spawn_blocking(move || {
        let mut r = reader.lock().unwrap();
        r.read(&mut buf)
      })
      .await?
      .map_err(AnyError::from)
    })
  }

  #[cfg(unix)]
  fn write(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    let writer = self.writer.clone();
    Box::pin(async move {
      tokio::task::spawn_blocking(move || {
        let mut w = writer.lock().unwrap();
        w.write(&mut buf)
      })
      .await?
      .map_err(AnyError::from)
    })
  }
}

#[derive(Deserialize)]
pub struct SizeArgs {
  rows: u16,
  columns: u16,
}

#[op]
fn op_pty_open(
  state: &mut OpState,
  args: SizeArgs,
) -> Result<ResourceId, AnyError> {
  super::check_unstable(state, "Deno.openPty");
  // todo(everyone): discuss permissions required to open a pty.
  //   considering that it doesn't add the ability to run any
  //   arbitrary code or executables.

  Ok(
    state
      .resource_table
      .add(PtyResource::new(pty::Pty::new(ConsoleSize {
        rows: args.rows,
        columns: args.columns,
      })?)),
  )
}
