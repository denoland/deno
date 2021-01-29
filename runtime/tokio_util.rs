// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

const MAX_WORKER_THREADS: usize = 2;
const MAX_BLOCKING_THREADS: usize = 32;

pub fn create_basic_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    // This limits the number of threads for blocking operations (like for
    // synchronous fs ops) or CPU bound tasks like when we run dprint in
    // parallel for deno fmt.
    // The default value is 512, which is an unhelpfully large thread pool. We
    // don't ever want to have more than a couple dozen threads.
    .max_blocking_threads(MAX_BLOCKING_THREADS)
    .build()
    .unwrap()
}

pub fn create_multi_runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_multi_thread()
    .enable_io()
    .enable_time()
    // This limits the number of std::threads that tokio will
    // create to perform on. This is limited to save on
    // startup time, as we don't get much benefit from
    // a large number of workers.
    .worker_threads(MAX_WORKER_THREADS)
    // This limits the number of threads for blocking operations (like for
    // synchronous fs ops) or CPU bound tasks like when we run dprint in
    // parallel for deno fmt.
    // The default value is 512, which is an unhelpfully large thread pool. We
    // don't ever want to have more than a couple dozen threads.
    .max_blocking_threads(MAX_BLOCKING_THREADS)
    .build()
    .unwrap()
}

pub fn run_multi<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_multi_runtime();
  rt.block_on(future)
}

// TODO(ry) rename to run_local ?
pub fn run_basic<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let rt = create_basic_runtime();
  rt.block_on(future)
}
