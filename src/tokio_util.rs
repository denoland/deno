// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::resources::Resource;

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
      // Similar to try_ready!, but also track/untrack accept task
      // in TcpListener resource.
      // In this way, when the listener is closed, the task can be
      // notified to error out (instead of stuck forever).
      AcceptState::Pending(ref mut r) => match r.poll_accept() {
        Ok(futures::prelude::Async::Ready(t)) => {
          r.untrack_task();
          t
        }
        Ok(futures::prelude::Async::NotReady) => {
          // Would error out if another accept task is being tracked.
          r.track_task()?;
          return Ok(futures::prelude::Async::NotReady);
        }
        Err(e) => {
          r.untrack_task();
          return Err(From::from(e));
        }
      },
      AcceptState::Empty => panic!("poll Accept after it's done"),
    };

    match mem::replace(&mut self.state, AcceptState::Empty) {
      AcceptState::Pending(_) => Ok((stream, addr).into()),
      AcceptState::Empty => panic!("invalid internal state"),
    }
  }
}

/// `futures::future::poll_fn` only support `F: FnMut()->Poll<T, E>`
/// However, we require that `F: FnOnce()->Poll<T, E>`.
/// Therefore, we created our version of `poll_fn`.
pub fn poll_fn<T, E, F>(f: F) -> PollFn<F>
where
  F: FnOnce() -> Poll<T, E>,
{
  PollFn { inner: Some(f) }
}

pub struct PollFn<F> {
  inner: Option<F>,
}

impl<T, E, F> Future for PollFn<F>
where
  F: FnOnce() -> Poll<T, E>,
{
  type Item = T;
  type Error = E;

  fn poll(&mut self) -> Poll<T, E> {
    let f = self.inner.take().expect("Inner fn has been taken.");
    f()
  }
}
