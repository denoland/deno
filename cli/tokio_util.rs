// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno::ErrBox;
use futures;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std::future::Future;
use tokio;
use tokio::runtime;

pub fn create_threadpool_runtime(
) -> Result<tokio::runtime::Runtime, tokio::io::Error> {
  runtime::Builder::new()
    .panic_handler(|err| std::panic::resume_unwind(err))
    .build()
}

pub fn run<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  // tokio::runtime::current_thread::run(future)
  let rt = create_threadpool_runtime().expect("Unable to create Tokio runtime");
  rt.block_on_all(future.boxed().compat()).unwrap();
}

pub fn run_on_current_thread<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  tokio::runtime::current_thread::run(future.boxed().compat());
}

/// THIS IS A HACK AND SHOULD BE AVOIDED.
///
/// This spawns a new thread and creates a single-threaded tokio runtime on that thread,
/// to execute the given future.
///
/// This is useful when we want to block the main runtime to
/// resolve a future without worrying that we'll use up all the threads in the
/// main runtime.
pub fn block_on<F, R>(future: F) -> Result<R, ErrBox>
where
  F: Send + 'static + Future<Output = Result<R, ErrBox>> + Unpin,
  R: Send + 'static,
{
  use std::sync::mpsc::channel;
  use std::thread;
  let (sender, receiver) = channel();
  // Create a new runtime to evaluate the future asynchronously.
  thread::spawn(move || {
    let r = tokio::runtime::current_thread::block_on_all(future.compat());
    sender
      .send(r)
      .expect("Unable to send blocking future result")
  });
  receiver
    .recv()
    .expect("Unable to receive blocking future result")
}

// Set the default executor so we can use tokio::spawn(). It's difficult to
// pass around mut references to the runtime, so using with_default is
// preferable. Ideally Tokio would provide this function.
#[cfg(test)]
pub fn init<F>(f: F)
where
  F: FnOnce(),
{
  let rt = create_threadpool_runtime().expect("Unable to create Tokio runtime");
  let mut executor = rt.executor();
  let mut enter = tokio_executor::enter().expect("Multiple executors at once");
  tokio_executor::with_default(&mut executor, &mut enter, move |_enter| f());
}

pub fn panic_on_error<I, E, F>(f: F) -> impl Future<Output = Result<I, ()>>
where
  F: Future<Output = Result<I, E>>,
  E: std::fmt::Debug,
{
  f.map_err(|err| panic!("Future got unexpected error: {:?}", err))
}

#[cfg(test)]
pub fn run_in_task<F>(f: F)
where
  F: FnOnce() + Send + 'static,
{
  let fut = futures::future::lazy(move |_cx| {
    f();
    Ok(())
  });

  run(fut)
}
