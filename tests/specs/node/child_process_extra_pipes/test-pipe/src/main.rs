use std::io::prelude::*;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;

fn main() {
  #[cfg(unix)]
  {
    let mut stream = unsafe { UnixStream::from_raw_fd(4) };

    stream.write_all(b"hello world\n").unwrap();
  }
}
