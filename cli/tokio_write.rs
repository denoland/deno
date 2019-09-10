// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::resources::DenoAsyncWrite;
use deno::ErrBox;
use futures::{Future, Poll};
use std::mem;

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
  A: DenoAsyncWrite,
  T: AsRef<[u8]>,
{
  Write {
    state: State::Pending { a, buf },
  }
}

/// This is almost the same implementation as in tokio, difference is
/// that error type is `ErrBox` instead of `std::io::Error`.
impl<A, T> Future for Write<A, T>
where
  A: DenoAsyncWrite,
  T: AsRef<[u8]>,
{
  type Item = (A, T, usize);
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<(A, T, usize), ErrBox> {
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
