// Copyright (c) 2019 Tokio Contributors. All rights reserved. MIT license.
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Forked from: https://github.com/tokio-rs/tokio/blob/9b3f8564af4bb1aee07fab3c401eb412ca5eeac5/tokio-io/src/io/read.rs
use crate::resources::DenoAsyncRead;
use deno::ErrBox;
use futures::{Future, Poll};
use std::mem;

/// This is almost the same implementation as in tokio, the only difference is
/// that error type is `ErrBox` instead of `std::io::Error`.

#[derive(Debug)]
enum State<R, T> {
  Pending { rd: R, buf: T },
  Empty,
}

/// Tries to read some bytes directly into the given `buf` in asynchronous
/// manner, returning a future type.
///
/// The returned future will resolve to both the I/O stream and the buffer
/// as well as the number of bytes read once the read operation is completed.
pub fn read<R, T>(rd: R, buf: T) -> Read<R, T>
where
  R: DenoAsyncRead,
  T: AsMut<[u8]>,
{
  Read {
    state: State::Pending { rd, buf },
  }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Read<R, T> {
  state: State<R, T>,
}

impl<R, T> Future for Read<R, T>
where
  R: DenoAsyncRead,
  T: AsMut<[u8]>,
{
  type Item = (R, T, usize);
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<(R, T, usize), ErrBox> {
    let nread = match self.state {
      State::Pending {
        ref mut rd,
        ref mut buf,
      } => try_ready!(rd.poll_read(&mut buf.as_mut()[..])),
      State::Empty => panic!("poll a Read after it's done"),
    };

    match mem::replace(&mut self.state, State::Empty) {
      State::Pending { rd, buf } => Ok((rd, buf, nread).into()),
      State::Empty => panic!("invalid internal state"),
    }
  }
}
