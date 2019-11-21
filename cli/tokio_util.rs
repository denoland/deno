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
