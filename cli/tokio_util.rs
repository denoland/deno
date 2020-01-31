// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use futures::Future;

#[cfg(test)]
pub fn run<F>(future: F)
where
  F: std::future::Future<Output = ()> + Send + 'static,
{
  let mut rt = tokio::runtime::Builder::new()
    .threaded_scheduler()
    .enable_all()
    .thread_name("deno")
    .build()
    .expect("Unable to create Tokio runtime");
  rt.block_on(future);
}

pub fn run_basic<F, R>(future: F) -> R
where
  F: std::future::Future<Output = R> + 'static,
{
  let rt = tokio::runtime::Builder::new()
    .basic_scheduler()
    .build()
    .unwrap();
  rt.block_on(future)
}

/*
pub fn spawn_basic_thread<R>(fut: impl Future<Output=R>) -> R
{
  let (load_sender, load_receiver) =
    tokio::sync::oneshot::channel::<JsonResult>();
  std::thread::spawn(move || {
    async {
      let r = fut.await;
      load_sender.send(r).unwrap();
    }
    let r = f();
    run_basic(fut);
  });
  load_receiver.wait()
}
*/
pub fn spawn_thread<F, R>(f: F) -> impl Future<Output = R>
where
  F: 'static + Send + FnOnce() -> R,
  R: 'static + Send,
{
  let (sender, receiver) = tokio::sync::oneshot::channel::<R>();
  std::thread::spawn(move || {
    let result = f();
    sender.send(result)
  });
  let fut = async { receiver.await.unwrap() };
  fut
}
