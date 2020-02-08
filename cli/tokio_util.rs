// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) rename to run_local ?
pub fn run_basic<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
{
  let mut rt = tokio::runtime::Builder::new()
    .basic_scheduler()
    .enable_io()
    .enable_time()
    .build()
    .unwrap();
  rt.block_on(future)
}

// TODO(ry) maybe replace with tokio::task::spawn_blocking
#[cfg(test)]
pub fn spawn_thread<F, R>(f: F) -> impl std::future::Future<Output = R>
where
  F: 'static + Send + FnOnce() -> R,
  R: 'static + Send,
{
  let (sender, receiver) = tokio::sync::oneshot::channel::<R>();
  std::thread::spawn(move || {
    let result = f();
    sender.send(result)
  });
  async { receiver.await.unwrap() }
}
