// Forked from https://github.com/Thomasdezeeuw/sendfile/blob/024f82cd4dede9048392a5bd6d8afcd4d5aa83d5/src/lib.rs
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{self, Poll};

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct UnixIoSlice<'a> {
  iov: libc::iovec,
  _p: PhantomData<&'a [u8]>,
}

impl<'a> UnixIoSlice<'a> {
  #[inline]
  pub fn new(buf: &'a [u8]) -> Self {
    let iov = libc::iovec {
      iov_base: buf.as_ptr() as *mut _,
      iov_len: buf.len(),
    };
    UnixIoSlice {
      iov,
      _p: PhantomData,
    }
  }

  #[inline]
  pub fn advance(&mut self, n: usize) {
    if self.iov.iov_len < n {
      panic!("advancing IoSlice beyond its length");
    }
    self.iov.iov_len -= n;
    // SAFETY: iov_base base pointer and resulting pointer at `n` bytes are guaranteed to be valid. Backing &[u8] is tied
    // to the lifetime of Self.
    self.iov.iov_base = unsafe { self.iov.iov_base.add(n) };
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.iov.iov_len
  }
}

#[inline]
fn advance_io_vec(io_vec: &mut &mut [UnixIoSlice], length: usize) -> usize {
  // Number of buffers to remove.
  let mut remove = 0;
  // Total length of all the to be removed buffers.
  let mut accumulated_len = 0;
  for buf in io_vec.iter() {
    if accumulated_len + buf.len() > length as usize {
      break;
    } else {
      accumulated_len += buf.len();
      remove += 1;
    }
  }

  *io_vec = &mut std::mem::take(io_vec)[remove..];
  if !io_vec.is_empty() {
    io_vec[0].advance(length as usize - accumulated_len);
  }
  accumulated_len
}

pub struct SendFile<'a> {
  pub io: (RawFd, RawFd),
  pub written: usize,
  pub slices: &'a mut [UnixIoSlice<'a>],
  pub sending_headers: bool,
}

impl<'a> SendFile<'a> {
  #[inline]
  pub fn try_send(&mut self) -> Result<usize, std::io::Error> {
    #[cfg(target_os = "linux")]
    {
      // This is the maximum the Linux kernel will write in a single call.
      let count = 0x7ffff000;
      let mut offset = self.written as libc::off_t;

      // sendfile() with TCP_CORK
      if self.sending_headers {
        let opt = 1;
        libc::setsockopt(self.io.1, libc::SOL_SOCKET, libc::TCP_CORK, &opt as *const _ as _, 4);
        let length = libc::writev(
          self.io.1,
          self.slices.as_ptr() as _,
          self.slices.len() as i32,
        );

        let io_vec = &mut self.slices;
        let accumulated_len = advance_io_vec(io_vec, length as usize);
        if io_vec.is_empty() {
          self.sending_headers = false;
          self.written += length as usize - accumulated_len;
        }
        return Ok(length as usize);
      }

      let res =
        // SAFETY: call to libc::sendfile()
        unsafe { libc::sendfile(self.io.1, self.io.0, &mut offset, count) };

      let opt = 0;
      libc::setsockopt(self.io.1, libc::SOL_SOCKET, libc::TCP_CORK, &opt as *const _ as _, 4);

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

      if self.sending_headers {
        let mut hdtr = libc::sf_hdtr {
          headers: self.slices.as_mut_ptr() as _,
          hdr_cnt: self.slices.len() as libc::c_int,
          trailers: std::ptr::null_mut(),
          trl_cnt: 0,
        };
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
            &mut hdtr,
            0,
          )
        };
        if res == -1 {
          return Err(io::Error::last_os_error());
        }

        let io_vec = &mut self.slices;
        let accumulated_len = advance_io_vec(io_vec, length as usize);
        if io_vec.is_empty() {
          self.sending_headers = false;
          self.written += length as usize - accumulated_len;
        }

        return Ok(length as usize);
      }

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

impl<'a> Future for SendFile<'a> {
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
