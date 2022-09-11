// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

pub fn create_basic_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .thread_keep_alive(std::time::Duration::from_millis(250))
    .build()
    .unwrap()
}

pub fn run_local<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_basic_runtime();
  let local = tokio::task::LocalSet::new();
  local.block_on(&rt, future)
}
