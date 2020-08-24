// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

pub fn create_basic_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_io()
    .enable_time()
    .build()
    .unwrap()
}

// TODO(ry) rename to run_local ?
pub fn run_basic<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let mut rt = create_basic_runtime();
  rt.block_on(future)
}
