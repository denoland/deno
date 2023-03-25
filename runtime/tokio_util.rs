// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::time::Duration;

pub fn create_basic_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    // This limits the number of threads for blocking operations (like for
    // synchronous fs ops) or CPU bound tasks like when we run dprint in
    // parallel for deno fmt.
    // The default value is 512, which is an unhelpfully large thread pool. We
    // don't ever want to have more than a couple dozen threads.
    .max_blocking_threads(32)
    .build()
    .unwrap()
}

pub fn run_local<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_basic_runtime();
  let local = tokio::task::LocalSet::new();
  let ret = local.block_on(&rt, future);

  // Any call to `spawn_blocking` on the tokio runtime, which includes file I/O
  // and non-blocking FFI, would usually keep the runtime from shutting down
  // until they finish. With FFI this can keep the process from ever shutting
  // down, so instead we set a maximum waiting time of half a second.
  rt.shutdown_timeout(Duration::from_secs_f64(0.5));
  ret
}
