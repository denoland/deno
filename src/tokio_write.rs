// TODO Submit this file upstream into tokio-io/src/io/write.rs
use std::io;
use std::mem;

use futures::{Future, Poll};
use tokio::io::AsyncWrite;

/// A future used to write some data to a stream.
///
/// This is created by the [`write`] top-level method.
///
/// [`write`]: fn.write.html
#[derive(Debug)]
pub struct Write<A, T> {
  state: State<A, T>,
}

#[derive(Debug)]
enum State<A, T> {
  Pending { a: A, buf: T },
  Empty,
}

/// Creates a future that will write some of the buffer `buf` to
/// the stream `a` provided.
///
/// Any error which happens during writing will cause both the stream and the
/// buffer to get destroyed.
pub fn write<A, T>(a: A, buf: T) -> Write<A, T>
where
  A: AsyncWrite,
  T: AsRef<[u8]>,
{
  Write {
    state: State::Pending { a, buf },
  }
}

impl<A, T> Future for Write<A, T>
where
  A: AsyncWrite,
  T: AsRef<[u8]>,
{
  type Item = (A, T, usize);
  type Error = io::Error;

  fn poll(&mut self) -> Poll<(A, T, usize), io::Error> {
    let nwritten = match self.state {
      State::Pending {
        ref mut a,
        ref mut buf,
      } => try_ready!(a.poll_write(buf.as_ref())),
      State::Empty => panic!("poll a Read after it's done"),
    };

    match mem::replace(&mut self.state, State::Empty) {
      State::Pending { a, buf } => Ok((a, buf, nwritten).into()),
      State::Empty => panic!("invalid internal state"),
    }
  }
}
