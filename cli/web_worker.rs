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
pub struct WebWorker(Worker);

impl WebWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
    external_channels: WorkerChannels,
  ) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_, external_channels);
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::web_worker::init(isolate, &state);
      ops::worker_host::init(isolate, &state);
      ops::errors::init(isolate, &state);
      ops::timers::init(isolate, &state);
      ops::fetch::init(isolate, &state);
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

    let handle = worker.thread_safe_handle();
    let _ = tokio_util::spawn_thread(move || tokio_util::run_basic(worker));

    tokio_util::run_basic(async move {
      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = handle.post_message(msg.clone()).await;
      assert!(r.is_ok());

      let maybe_msg = handle.get_message().await;
      assert!(maybe_msg.is_some());

      let r = handle.post_message(msg.clone()).await;
      assert!(r.is_ok());

      let maybe_msg = handle.get_message().await;
      assert!(maybe_msg.is_some());
      assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = handle.post_message(msg).await;
      assert!(r.is_ok());
    });
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let mut worker = create_test_worker();
    let handle = worker.thread_safe_handle();
    let worker_complete_fut = tokio_util::spawn_thread(move || {
      worker
        .execute("onmessage = () => { delete self.onmessage; }")
        .unwrap();
      tokio_util::run_basic(worker)
    });

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    tokio_util::run_basic(async move {
      let r = handle.post_message(msg).await;
      assert!(r.is_ok());
      let r = worker_complete_fut.await;
      assert!(r.is_ok());
    });
  }
}
