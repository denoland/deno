// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
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
