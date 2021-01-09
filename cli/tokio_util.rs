// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

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

// TODO(ry) rename to run_local ?
pub fn run_basic<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_basic_runtime();
  rt.block_on(future)
}
