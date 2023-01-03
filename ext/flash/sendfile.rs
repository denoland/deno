// Forked from https://github.com/Thomasdezeeuw/sendfile/blob/024f82cd4dede9048392a5bd6d8afcd4d5aa83d5/src/lib.rs
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{self, Poll};

pub struct SendFile {
  pub io: (RawFd, RawFd),
  pub written: usize,
}

impl SendFile {
  #[inline]
  pub fn try_send(&mut self) -> Result<usize, std::io::Error> {
    #[cfg(target_os = "linux")]
    {
      // This is the maximum the Linux kernel will write in a single call.
      let count = 0x7ffff000;
      let mut offset = self.written as libc::off_t;

      let res =
        // SAFETY: call to libc::sendfile()
        unsafe { libc::sendfile(self.io.1, self.io.0, &mut offset, count) };
      if res == -1 {
        Err(io::Error::last_os_error())
      } else {
        self.written = offset as usize;
        Ok(res as usize)
      }
    }
    #[cfg(target_os = "macos")]
    {
      // Send all bytes.
      let mut length = 0;
      // On macOS `length` is value-result parameter. It determines the number
      // of bytes to write and returns the number of bytes written also in
      // case of `EAGAIN` errors.
      // SAFETY: call to libc::sendfile()
      let res = unsafe {
        libc::sendfile(
          self.io.0,
          self.io.1,
          self.written as libc::off_t,
          &mut length,
          std::ptr::null_mut(),
          0,
        )
      };
      self.written += length as usize;
      if res == -1 {
        Err(io::Error::last_os_error())
      } else {
        Ok(length as usize)
      }
    }
  }
}

impl Future for SendFile {
  type Output = Result<(), std::io::Error>;

  fn poll(
    mut self: Pin<&mut Self>,
    _: &mut task::Context<'_>,
  ) -> Poll<Self::Output> {
    loop {
      match self.try_send() {
        Ok(0) => break Poll::Ready(Ok(())),
        Ok(_) => continue,
        Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
          break Poll::Pending
        }
        Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue, // Try again.
        Err(err) => break Poll::Ready(Err(err)),
      }
    }
  }
}
