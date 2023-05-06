// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
    .event_interval(10000)
    .global_queue_interval(10000 * 4)
    .build()
    .unwrap()
}

/// Runs the provided future in a "current thread" flavor of Tokio runtime.
/// This function blocks until the provided future resolves. It should be used
/// as an entry-point for asynchronous code.
///
/// Since `deno_core::JsRuntime` is not `Send` we are forced to use current
/// thread flavor. You can still use `tokio::spawn` for non-`Send` futures if
/// you wrap them in `deno_core::MaskFutureAsSend`.
pub fn run_in_current_thread_runtime<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
  F::Output: Send + 'static,
{
  let rt = create_basic_runtime();
  let join_handle =
    rt.spawn(unsafe { deno_core::MaskFutureAsSend::new(future) });
  rt.block_on(join_handle).unwrap()
}
