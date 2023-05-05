// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use core::future::Future;
use core::marker::PhantomData;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

pub struct LocalRuntime {
  inner: Runtime,
  _not_send: PhantomData<*mut ()>,
}

impl LocalRuntime {
  pub fn new() -> Self {
    Self {
      inner: create_basic_runtime(),
      _not_send: PhantomData,
    }
  }

  pub fn spawn<F>(&self, fut: F) -> JoinHandle<F::Output>
  where
    F: 'static + Future + Send,
    // We could eliminate this one if necessary, but it's tedious to do so.
    F::Output: 'static + Send,
  {
    // SAFETY: The `LocalRuntime` type is neither Send nor Sync, so it's
    // not possible to move it across threads. The futures inside it are
    // only ever polled inside `block_on` calls on the runtime, so since
    // those calls must happen on the same thread, the futures are not
    // polled no the wrong thread.
    //
    // Note that `Handle::block_on` does not execute tasks on a current-
    // thread runtime, so you can't use it to poll tasks from another
    // thread.
    unsafe { self.inner.spawn(fut) }
  }

  pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
    self.inner.block_on(fut)
  }
}

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
    .global_queue_interval(4096)
    .event_interval(1024)
    .build()
    .unwrap()
}

pub fn run_local2<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
  F::Output: Send + 'static,
{
  let local = LocalRuntime::new();

  local.spawn(async move {
    let handle = tokio::runtime::Handle::current();
    let runtime_monitor = tokio_metrics::RuntimeMonitor::new(&handle);

    // print runtime metrics every 500ms
    let frequency = std::time::Duration::from_millis(500);
    tokio::spawn(async move {
      for metrics in runtime_monitor.intervals() {
        println!("Metrics = {:?}", metrics);
        tokio::time::sleep(frequency).await;
      }
    });
  });

  let join_handle =
    local.spawn(unsafe { make_me_send::MakeMeSend::new(future) });
  local.block_on(async move { join_handle.await }).unwrap()
}

pub fn run_local3<F>(future: F)
where
  F: std::future::Future + 'static,
  F::Output: Send + 'static,
{
  let local = LocalRuntime::new();
  local.spawn(unsafe { make_me_send::MakeMeSend::new(future) });
}

pub fn run_local<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R>,
{
  let local = LocalRuntime::new();
  local.block_on(future)
}

pub mod make_me_send {
  use core::future::Future;
  use core::pin::Pin;
  use core::task::Context;
  use core::task::Poll;

  pub struct MakeMeSend<F> {
    future: F,
  }

  impl<F> MakeMeSend<F> {
    /// SAFETY: You must ensure that the future is not used
    /// on the wrong thread.
    pub unsafe fn new(future: F) -> Self {
      Self { future }
    }
  }

  unsafe impl<F> Send for MakeMeSend<F> {}

  impl<F: Future> Future for MakeMeSend<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
      unsafe {
        let me = Pin::into_inner_unchecked(self);
        let future = Pin::new_unchecked(&mut me.future);
        future.poll(cx)
      }
    }
  }
}
