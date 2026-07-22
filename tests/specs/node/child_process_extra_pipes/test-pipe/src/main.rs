use std::fs::File;
use std::io::prelude::*;
use std::os::fd::FromRawFd;

fn main() {
  #[cfg(unix)]
  {
    let mut pipe = unsafe { File::from_raw_fd(4) };

    let mut read = 0;
    let mut buf = [0u8; 1024];
    loop {
      if read > 4 {
        assert_eq!(&buf[..5], b"start");
        break;
      }
      match pipe.read(&mut buf) {
        Ok(n) => {
          read += n;
        }
        Ok(0) => {
          return;
        }
        Err(e) => {
          eprintln!("GOT ERROR: {e:?}");
        }
      }
    }
    pipe.write_all(b"hello world").unwrap();
  }
}
