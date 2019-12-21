// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::future::Future;
use tokio;
use tokio::runtime;

pub fn create_threadpool_runtime(
) -> Result<tokio::runtime::Runtime, tokio::io::Error> {
  runtime::Builder::new()
    .threaded_scheduler()
    .enable_all()
    // .panic_handler(|err| std::panic::resume_unwind(err))
    .build()
}

pub fn run<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  // tokio::runtime::current_thread::run(future)
  let mut rt =
    create_threadpool_runtime().expect("Unable to create Tokio runtime");
  rt.block_on(future).unwrap();
}

pub fn run_on_current_thread<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  tokio::runtime::Builder::new()
    .basic_scheduler()
    .build()
    .unwrap()
    .block_on(future)
    .unwrap();
}
