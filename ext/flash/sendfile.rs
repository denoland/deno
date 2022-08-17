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
    let mut length = 0; // Send all bytes.
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
