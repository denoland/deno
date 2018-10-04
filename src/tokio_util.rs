// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use resources::Resource;

use futures;
use futures::Future;
use futures::Poll;
use std::io;
use std::mem;
use std::net::SocketAddr;
use tokio;
use tokio::net::TcpStream;
use tokio_executor;

pub fn block_on<F, R, E>(future: F) -> Result<R, E>
where
  F: Send + 'static + Future<Item = R, Error = E>,
  R: Send + 'static,
  E: Send + 'static,
{
  let (tx, rx) = futures::sync::oneshot::channel();
  tokio::spawn(future.then(move |r| tx.send(r).map_err(|_| unreachable!())));
  rx.wait().unwrap()
}

// Set the default executor so we can use tokio::spawn(). It's difficult to
// pass around mut references to the runtime, so using with_default is
// preferable. Ideally Tokio would provide this function.
pub fn init<F>(f: F)
where
  F: FnOnce(),
{
  let rt = tokio::runtime::Runtime::new().unwrap();
  let mut executor = rt.executor();
  let mut enter = tokio_executor::enter().expect("Multiple executors at once");
  tokio_executor::with_default(&mut executor, &mut enter, move |_enter| f());
}

#[derive(Debug)]
enum AcceptState {
  Pending(Resource),
  Empty,
}

/// Simply accepts a connection.
pub fn accept(r: Resource) -> Accept {
  Accept {
    state: AcceptState::Pending(r),
  }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Accept {
  state: AcceptState,
}

impl Future for Accept {
  type Item = (TcpStream, SocketAddr);
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let (stream, addr) = match self.state {
      AcceptState::Pending(ref mut r) => try_ready!(r.poll_accept()),
      AcceptState::Empty => panic!("poll Accept after it's done"),
    };

    match mem::replace(&mut self.state, AcceptState::Empty) {
      AcceptState::Pending(_) => Ok((stream, addr).into()),
      AcceptState::Empty => panic!("invalid internal state"),
    }
  }
}
