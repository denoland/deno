// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::fmt::Debug;
use std::str::FromStr;

use deno_core::task::MaskFutureAsSend;

/// Default configuration for tokio. In the future, this method may have different defaults
/// depending on the platform and/or CPU layout.
const fn tokio_configuration() -> (u32, u32, usize) {
  (61, 31, 1024)
}

fn tokio_env<T: FromStr>(name: &'static str, default: T) -> T
where
  <T as FromStr>::Err: Debug,
{
  match std::env::var(name) {
    Ok(value) => value.parse().unwrap(),
    Err(_) => default,
  }
}

pub fn create_basic_runtime() -> tokio::runtime::Runtime {
  let (event_interval, global_queue_interval, max_io_events_per_tick) =
    tokio_configuration();

  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .event_interval(tokio_env("DENO_TOKIO_EVENT_INTERVAL", event_interval))
    .global_queue_interval(tokio_env(
      "DENO_TOKIO_GLOBAL_QUEUE_INTERVAL",
      global_queue_interval,
    ))
    .max_io_events_per_tick(tokio_env(
      "DENO_TOKIO_MAX_IO_EVENTS_PER_TICK",
      max_io_events_per_tick,
    ))
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

  // Since this is the main future, we want to box it in debug mode because it tends to be fairly
  // large and the compiler won't optimize repeated copies. We also make this runtime factory
  // function #[inline(always)] to avoid holding the unboxed, unused future on the stack.

  #[cfg(debug_assertions)]
  // SAFETY: this this is guaranteed to be running on a current-thread executor
  let future = Box::pin(unsafe { MaskFutureAsSend::new(future) });

  #[cfg(not(debug_assertions))]
  // SAFETY: this this is guaranteed to be running on a current-thread executor
  let future = unsafe { MaskFutureAsSend::new(future) };

  let join_handle = rt.spawn(future);
  rt.block_on(join_handle).unwrap().into_inner()
}
