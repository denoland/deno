// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ops;
use crate::state::State;
use crate::worker::Worker;
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
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_);
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::web_worker::init(isolate, &state, &worker.internal_channels.sender);
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
  use crate::ops::worker_host::run_worker_loop;
  use crate::startup_data;
  use crate::state::State;
  use crate::tokio_util;
  use crate::worker::WorkerEvent;
  use crate::worker::WorkerHandle;

  fn create_test_worker() -> WebWorker {
    let state = State::mock("./hello.js");
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      state,
    );
    worker.execute("bootstrapWorkerRuntime(\"TEST\")").unwrap();
    worker
  }
  #[test]
  fn test_worker_messages() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_worker();
      let source = r#"
          onmessage = function(e) {
            console.log("msg from main script", e.data);
            if (e.data == "exit") {
              return close();
            } else {
              console.assert(e.data === "hi");
            }
            postMessage([1, 2, 3]);
            console.log("after postMessage");
          }
          "#;
      worker.execute(source).unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let mut rt = tokio_util::create_basic_runtime();
      let r = run_worker_loop(&mut rt, &mut worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    tokio_util::run_basic(async move {
      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = handle.post_message(msg.clone()).await;
      assert!(r.is_ok());

      let maybe_msg = handle.get_event().await;
      assert!(maybe_msg.is_some());

      let r = handle.post_message(msg.clone()).await;
      assert!(r.is_ok());

      let maybe_msg = handle.get_event().await;
      assert!(maybe_msg.is_some());
      match maybe_msg {
        Some(WorkerEvent::Message(buf)) => {
          assert_eq!(*buf, *b"[1,2,3]");
        }
        _ => unreachable!(),
      }

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = handle.post_message(msg).await;
      assert!(r.is_ok());
      let event = handle.get_event().await;
      assert!(event.is_none());
      handle.sender.close_channel();
    });
    join_handle.join().expect("Failed to join worker thread");
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_worker();
      worker.execute("onmessage = () => { close(); }").unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let mut rt = tokio_util::create_basic_runtime();
      let r = run_worker_loop(&mut rt, &mut worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    tokio_util::run_basic(async move {
      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = handle.post_message(msg.clone()).await;
      assert!(r.is_ok());
      let event = handle.get_event().await;
      assert!(event.is_none());
      handle.sender.close_channel();
    });
    join_handle.join().expect("Failed to join worker thread");
  }
}
