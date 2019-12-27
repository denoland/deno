// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use std::future::Future;
use tokio;
use tokio::runtime;

pub fn run<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  let mut rt = runtime::Builder::new()
    .threaded_scheduler()
    .enable_all()
    .thread_name("deno")
    .build()
    .expect("Unable to create Tokio runtime");
  rt.block_on(future).unwrap();
}

pub fn run_on_current_thread<F>(future: F)
where
  F: Future<Output = Result<(), ()>> + Send + 'static,
{
  let mut rt = runtime::Builder::new()
    .basic_scheduler()
    .enable_all()
    .thread_name("deno")
    .build()
    .expect("Unable to create Tokio runtime");
  rt.block_on(future).unwrap();
}
