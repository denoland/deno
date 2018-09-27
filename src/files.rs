// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use futures;
use futures::Poll;
use std;
use std::collections::HashMap;
use std::io::Error;
use std::io::{Read, Write};
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tokio;
use tokio::io::{AsyncRead, AsyncWrite};

// These store Deno's file descriptors. These are not necessarally the operating
// system ones.
type FdTable = HashMap<i32, Repr>;

lazy_static! {
  // Starts at 3 because stdio is [0-2].
  static ref NEXT_FD: AtomicIsize = AtomicIsize::new(3);
  static ref FD_TABLE: Mutex<FdTable> = Mutex::new({
    let mut m = HashMap::new();
    // TODO Load these lazily during lookup?
    m.insert(0, Repr::Stdin(tokio::io::stdin()));
    m.insert(1, Repr::Stdout(tokio::io::stdout()));
    m.insert(2, Repr::Stderr(tokio::io::stderr()));
    m
  });
}

// Internal representation of DFile.
enum Repr {
  Stdin(tokio::io::Stdin),
  Stdout(tokio::io::Stdout),
  Stderr(tokio::io::Stderr),
  FsFile(tokio::fs::File),
}

// Abstract async file interface.
// fd does not necessarally correspond to an OS fd.
// Ideally in unix, if DFile represents an OS fd, it will be the same.
pub struct DFile {
  pub fd: i32,
}

impl Read for DFile {
  fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
    unimplemented!();
  }
}

impl AsyncRead for DFile {
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, Error> {
    let mut table = FD_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.fd);
    match maybe_repr {
      None => panic!("bad fd"),
      Some(repr) => match repr {
        Repr::FsFile(ref mut f) => f.poll_read(buf),
        Repr::Stdin(ref mut f) => f.poll_read(buf),
        Repr::Stdout(_) | Repr::Stderr(_) => {
          panic!("Cannot read from stdout/stderr")
        }
      },
    }
  }
}

impl Write for DFile {
  fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
    unimplemented!()
  }

  fn flush(&mut self) -> std::io::Result<()> {
    unimplemented!()
  }
}

impl AsyncWrite for DFile {
  fn poll_write(&mut self, buf: &[u8]) -> Poll<usize, Error> {
    let mut table = FD_TABLE.lock().unwrap();
    let maybe_repr = table.get_mut(&self.fd);
    match maybe_repr {
      None => panic!("bad fd"),
      Some(repr) => match repr {
        Repr::FsFile(ref mut f) => f.poll_write(buf),
        Repr::Stdout(ref mut f) => f.poll_write(buf),
        Repr::Stderr(ref mut f) => f.poll_write(buf),
        Repr::Stdin(_) => panic!("Cannot write to stdin"),
      },
    }
  }

  fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
    unimplemented!()
  }
}

fn new_fd() -> i32 {
  // TODO If on unix, just extract the real FD of fs_file.
  // let fd = AsRawFd::as_raw_fd(fs_file.std());
  let next_fd = NEXT_FD.fetch_add(1, Ordering::SeqCst);
  next_fd as i32
}

pub fn add_fs_file(fs_file: tokio::fs::File) -> DFile {
  let fd = new_fd();
  let mut tg = FD_TABLE.lock().unwrap();
  match tg.insert(fd, Repr::FsFile(fs_file)) {
    Some(_) => panic!("There is already a file with that fd"),
    None => DFile { fd },
  }
}

pub fn lookup(fd: i32) -> Option<DFile> {
  let table = FD_TABLE.lock().unwrap();
  table.get(&fd).map(|_| DFile { fd })
}
