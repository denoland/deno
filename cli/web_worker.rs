// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ops;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use crate::worker::WorkerChannels;
use deno_core;
use deno_core::ErrBox;
use deno_core::StartupData;
use futures::future::FutureExt;
use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

/// This worker is implementation of `Worker` Web API
///
/// At the moment this type of worker supports only
/// communication with parent and creating new workers.
///
/// Each `WebWorker` is either a child of `MainWorker` or other
/// `WebWorker`.
#[derive(Clone)]
pub struct WebWorker(Worker);

impl WebWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
    external_channels: WorkerChannels,
  ) -> Self {
    let state_ = state.clone();
    let worker = Worker::new(name, startup_data, state_, external_channels);
    {
      let mut isolate = worker.isolate.try_lock().unwrap();
      ops::runtime::init(&mut isolate, &state);
      ops::web_worker::init(&mut isolate, &state);
      ops::worker_host::init(&mut isolate, &state);
      ops::errors::init(&mut isolate, &state);
      ops::timers::init(&mut isolate, &state);
      ops::fetch::init(&mut isolate, &state);
    }

    Self(worker)
  }
}

impl Deref for WebWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for WebWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Future for WebWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.0.poll_unpin(cx)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use futures::executor::block_on;

  pub fn run_in_task<F>(f: F)
  where
    F: FnOnce() + Send + 'static,
  {
    let fut = futures::future::lazy(move |_cx| f());
    tokio_util::run(fut)
  }

  fn create_test_worker() -> WebWorker {
    let (int, ext) = ThreadSafeState::create_channels();
    let state = ThreadSafeState::mock(
      vec![String::from("./deno"), String::from("hello.js")],
      int,
    );
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      state,
      ext,
    );
    worker.execute("bootstrapWorkerRuntime(\"TEST\")").unwrap();
    worker.execute("runWorkerMessageLoop()").unwrap();
    worker
  }

  #[test]
  fn test_worker_messages() {
    run_in_task(|| {
      let mut worker = create_test_worker();
      let source = r#"
        onmessage = function(e) {
          console.log("msg from main script", e.data);
          if (e.data == "exit") {
            delete self.onmessage;
            return;
          } else {
            console.assert(e.data === "hi");
          }
          postMessage([1, 2, 3]);
          console.log("after postMessage");
        }
        "#;
      worker.execute(source).unwrap();

      let worker_ = worker.clone();

      let fut = async move {
        let r = worker.await;
        r.unwrap();
      };

      tokio::spawn(fut);

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

      let r = block_on(worker_.post_message(msg));
      assert!(r.is_ok());

      let maybe_msg = block_on(worker_.get_message());
      assert!(maybe_msg.is_some());
      // Check if message received is [1, 2, 3] in json
      assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = block_on(worker_.post_message(msg));
      assert!(r.is_ok());
    })
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    run_in_task(|| {
      let mut worker = create_test_worker();
      worker
        .execute("onmessage = () => { delete self.onmessage; }")
        .unwrap();

      let worker_ = worker.clone();
      let worker_future = async move {
        let result = worker_.await;
        println!("workers.rs after resource close");
        result.unwrap();
      }
      .shared();

      let worker_future_ = worker_future.clone();
      tokio::spawn(worker_future_);

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = block_on(worker.post_message(msg));
      assert!(r.is_ok());

      block_on(worker_future)
    })
  }
}
