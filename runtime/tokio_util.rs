// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::task::MaskFutureAsSend;

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

#[inline(always)]
pub fn create_and_run_current_thread<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
  R: Send + 'static,
{
  let rt = create_basic_runtime();
  // Since this is the main future, we want to box it because it tends to be fairly large. We
  // also make this function inline(always) to avoid holding the unboxed, unused future on the
  // stack.
  let future = Box::pin(future);
  // SAFETY: this this is guaranteed to be running on a current-thread executor
  let future = unsafe { MaskFutureAsSend::new(future) };
  let join_handle = rt.spawn(future);
  rt.block_on(join_handle).unwrap().into_inner()
}
