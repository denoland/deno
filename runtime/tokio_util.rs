// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use std::fmt::Debug;
use std::str::FromStr;

use deno_core::unsync::MaskFutureAsSend;
#[cfg(tokio_unstable)]
use tokio_metrics::RuntimeMonitor;

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
fn create_and_run_current_thread_inner<F, R>(
  future: F,
  metrics_enabled: bool,
) -> R
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

  #[cfg(tokio_unstable)]
  let join_handle = if metrics_enabled {
    rt.spawn(async move {
      let metrics_interval: u64 = std::env::var("DENO_TOKIO_METRICS_INTERVAL")
        .ok()
        .and_then(|val| val.parse().ok())
        .unwrap_or(1000);
      let handle = tokio::runtime::Handle::current();
      let runtime_monitor = RuntimeMonitor::new(&handle);
      tokio::spawn(async move {
        for interval in runtime_monitor.intervals() {
          println!("{:#?}", interval);
          // wait 500ms
          tokio::time::sleep(std::time::Duration::from_millis(
            metrics_interval,
          ))
          .await;
        }
      });
      future.await
    })
  } else {
    rt.spawn(future)
  };

  #[cfg(not(tokio_unstable))]
  let join_handle = rt.spawn(future);

  let r = rt.block_on(join_handle).unwrap().into_inner();
  // Forcefully shutdown the runtime - we're done executing JS code at this
  // point, but there might be outstanding blocking tasks that were created and
  // latered "unrefed". They won't terminate on their own, so we're forcing
  // termination of Tokio runtime at this point.
  rt.shutdown_background();
  r
}

#[inline(always)]
pub fn create_and_run_current_thread<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
  R: Send + 'static,
{
  create_and_run_current_thread_inner(future, false)
}

#[inline(always)]
pub fn create_and_run_current_thread_with_maybe_metrics<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
  R: Send + 'static,
{
  let metrics_enabled = std::env::var("DENO_TOKIO_METRICS").ok().is_some();
  create_and_run_current_thread_inner(future, metrics_enabled)
}
