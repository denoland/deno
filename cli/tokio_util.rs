// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::resources::Resource;
use futures;
use futures::future::FutureExt;
use std::future::Future;
use std::io;
use std::mem;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;
use tokio::net::TcpStream;
use tokio::runtime;

pub fn create_threadpool_runtime() -> tokio::runtime::Runtime {
  runtime::Builder::new()
    .panic_handler(|err| std::panic::resume_unwind(err))
    .build()
    .unwrap()
}

pub fn run<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  // tokio::runtime::current_thread::run(future)
  let rt = create_threadpool_runtime();
  rt.block_on(future).unwrap();
}

/// THIS IS A HACK AND SHOULD BE AVOIDED.
///
/// This creates a new tokio runtime, with many new threads, to execute the
/// given future. This is useful when we want to block the main runtime to
/// resolve a future without worrying that we'll use up all the threads in the
/// main runtime.
pub fn block_on<F, R, E>(future: F) -> Result<R, E>
where
  F: Send + 'static + Future<Output = Result<R, E>>,
  R: Send + 'static,
  E: Send + 'static,
{
  use std::sync::mpsc::channel;
  use std::thread;
  let (sender, receiver) = channel();
  // Create a new runtime to evaluate the future asynchronously.
  thread::spawn(move || {
    let rt = create_threadpool_runtime();
    let r = rt.block_on(future);
    sender.send(r).unwrap();
  });
  receiver.recv().unwrap()
}

pub fn spawn_on_default<F, R, E>(
  future: F,
) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send>>
where
  F: Send + 'static + Future<Output = Result<R, E>> + Unpin,
  R: Send + 'static,
  E: Send + 'static,
{
  use futures::channel::oneshot::channel;
  use tokio::executor::Executor;
  let (sender, receiver) = channel();
  tokio::executor::DefaultExecutor::current()
    .spawn(
      future
        .then(|result| {
          assert!(sender.send(result).is_ok());
          futures::future::ready(())
        })
        .boxed(),
    )
    .unwrap();
  receiver.map(|result| result.unwrap()).boxed()
}

// Set the default executor so we can use tokio::spawn(). It's difficult to
// pass around mut references to the runtime, so using with_default is
// preferable. Ideally Tokio would provide this function.
#[cfg(test)]
pub fn init<F>(f: F)
where
  F: FnOnce(),
{
  let rt = create_threadpool_runtime();
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
  type Output = Result<(TcpStream, SocketAddr), io::Error>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = Pin::get_mut(self);
    let (stream, addr) = match inner.state {
      // Similar to try_ready!, but also track/untrack accept task
      // in TcpListener resource.
      // In this way, when the listener is closed, the task can be
      // notified to error out (instead of stuck forever).
      AcceptState::Pending(ref mut r) => match r.poll_accept(cx) {
        Poll::Ready(Ok(t)) => {
          r.untrack_task();
          t
        }
        Poll::Pending => {
          // Would error out if another accept task is being tracked.
          r.track_task(cx)?;
          return Poll::Pending;
        }
        Poll::Ready(Err(e)) => {
          r.untrack_task();
          return Poll::Ready(Err(e));
        }
      },
      AcceptState::Empty => panic!("poll Accept after it's done"),
    };

    match mem::replace(&mut inner.state, AcceptState::Empty) {
      AcceptState::Pending(_) => Poll::Ready(Ok((stream, addr).into())),
      AcceptState::Empty => panic!("invalid internal state"),
    }
  }
}

/// `futures::future::poll_fn` only support `F: FnMut()->Poll<T, E>`
/// However, we require that `F: FnOnce()->Poll<T, E>`.
/// Therefore, we created our version of `poll_fn`.
pub fn poll_fn<T, E, F>(f: F) -> PollFn<F>
where
  F: FnOnce() -> Poll<Result<T, E>> + Unpin,
{
  PollFn { inner: Some(f) }
}

pub struct PollFn<F> {
  inner: Option<F>,
}

impl<T, E, F> Future for PollFn<F>
where
  F: FnOnce() -> Poll<Result<T, E>> + Unpin,
{
  type Output = Result<T, E>;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
    let inner = Pin::get_mut(self);
    let f = inner.inner.take().expect("Inner fn has been taken.");
    f()
  }
}

pub fn panic_on_error<O, E, F>(f: F) -> impl Future<Output = O>
where
  F: Future<Output = Result<O, E>>,
  E: std::fmt::Debug,
{
  f.map(|result| match result {
    Err(err) => panic!("Future got unexpected error: {:?}", err),
    Ok(v) => v,
  })
}
