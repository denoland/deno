// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ops;
use crate::state::State;
use crate::worker::Worker;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core::v8;
use deno_core::ErrBox;
use deno_core::StartupData;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use std::future::Future;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

/// Wrapper for `WorkerHandle` that adds functionality
/// for terminating workers.
///
/// This struct is used by host as well as worker itself.
///
/// Host uses it to communicate with worker and terminate it,
/// while worker uses it only to finish execution on `self.close()`.
#[derive(Clone)]
pub struct WebWorkerHandle {
  worker_handle: WorkerHandle,
  terminate_tx: mpsc::Sender<()>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl Deref for WebWorkerHandle {
  type Target = WorkerHandle;
  fn deref(&self) -> &Self::Target {
    &self.worker_handle
  }
}

impl DerefMut for WebWorkerHandle {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.worker_handle
  }
}

impl WebWorkerHandle {
  pub fn terminate(&self) {
    // This function can be called multiple times by whomever holds
    // the handle. However only a single "termination" should occur so
    // we need a guard here.
    let already_terminated = self.terminated.swap(true, Ordering::Relaxed);

    if !already_terminated {
      self.isolate_handle.terminate_execution();
      let mut sender = self.terminate_tx.clone();
      // This call should be infallible hence the `expect`.
      // This might change in the future.
      sender.try_send(()).expect("Failed to terminate");
    }
  }
}

/// This worker is implementation of `Worker` Web API
///
/// At the moment this type of worker supports only
/// communication with parent and creating new workers.
///
/// Each `WebWorker` is either a child of `MainWorker` or other
/// `WebWorker`.
pub struct WebWorker {
  worker: Worker,
  event_loop_idle: bool,
  terminate_rx: mpsc::Receiver<()>,
  handle: WebWorkerHandle,
  pub has_deno_namespace: bool,
}

impl WebWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: &Rc<State>,
    has_deno_namespace: bool,
  ) -> Self {
    let mut worker = Worker::new(name, startup_data, &state);

    let terminated = Arc::new(AtomicBool::new(false));
    let isolate_handle = worker.isolate.thread_safe_handle();
    let (terminate_tx, terminate_rx) = mpsc::channel::<()>(1);

    let handle = WebWorkerHandle {
      worker_handle: worker.thread_safe_handle(),
      terminated,
      isolate_handle,
      terminate_tx,
    };

    let mut web_worker = Self {
      worker,
      event_loop_idle: false,
      terminate_rx,
      handle,
      has_deno_namespace,
    };

    let handle = web_worker.thread_safe_handle();

    {
      let isolate = &mut web_worker.worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::web_worker::init(
        isolate,
        &state,
        &web_worker.worker.internal_channels.sender,
        handle,
      );
      ops::worker_host::init(isolate, &state);
      ops::idna::init(isolate, &state);
      ops::io::init(isolate, &state);
      ops::resources::init(isolate, &state);
      ops::errors::init(isolate, &state);
      ops::timers::init(isolate, &state);
      ops::fetch::init(isolate, &state);

      if has_deno_namespace {
        ops::runtime_compiler::init(isolate, &state);
        ops::fs::init(isolate, &state);
        ops::fs_events::init(isolate, &state);
        ops::plugin::init(isolate, &state);
        ops::net::init(isolate, &state);
        ops::tls::init(isolate, &state);
        ops::os::init(isolate, &state);
        ops::permissions::init(isolate, &state);
        ops::process::init(isolate, &state);
        ops::random::init(isolate, &state);
        ops::signal::init(isolate, &state);
        ops::tty::init(isolate, &state);
      }
    }

    web_worker
  }
}

impl WebWorker {
  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WebWorkerHandle {
    self.handle.clone()
  }
}

impl Deref for WebWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.worker
  }
}

impl DerefMut for WebWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.worker
  }
}

impl Future for WebWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let worker = &mut inner.worker;

    let terminated = inner.handle.terminated.load(Ordering::Relaxed);

    if terminated {
      return Poll::Ready(Ok(()));
    }

    if !inner.event_loop_idle {
      match worker.poll_unpin(cx) {
        Poll::Ready(r) => {
          let terminated = inner.handle.terminated.load(Ordering::Relaxed);
          if terminated {
            return Poll::Ready(Ok(()));
          }

          if let Err(e) = r {
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }
          inner.event_loop_idle = true;
        }
        Poll::Pending => {}
      }
    }

    if let Poll::Ready(r) = inner.terminate_rx.poll_next_unpin(cx) {
      // terminate_rx should never be closed
      assert!(r.is_some());
      return Poll::Ready(Ok(()));
    }

    if let Poll::Ready(r) =
      worker.internal_channels.receiver.poll_next_unpin(cx)
    {
      match r {
        Some(msg) => {
          let msg = String::from_utf8(msg.to_vec()).unwrap();
          let script = format!("workerMessageRecvCallback({})", msg);

          if let Err(e) = worker.execute(&script) {
            // If execution was terminated during message callback then
            // just ignore it
            if inner.handle.terminated.load(Ordering::Relaxed) {
              return Poll::Ready(Ok(()));
            }

            // Otherwise forward error to host
            let mut sender = worker.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }

          // Let event loop be polled again
          inner.event_loop_idle = false;
          worker.waker.wake();
        }
        None => unreachable!(),
      }
    }

    Poll::Pending
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::startup_data;
  use crate::state::State;
  use crate::tokio_util;
  use crate::worker::WorkerEvent;

  fn create_test_worker() -> WebWorker {
    let state = State::mock("./hello.js");
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      startup_data::deno_isolate_init(),
      &state,
      false,
    );
    worker
      .execute("bootstrap.workerRuntime(\"TEST\", false)")
      .unwrap();
    worker
  }
  #[test]
  fn test_worker_messages() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

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
      let r = tokio_util::run_basic(worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    tokio_util::run_basic(async move {
      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = handle.post_message(msg.clone());
      assert!(r.is_ok());

      let maybe_msg = handle.get_event().await.unwrap();
      assert!(maybe_msg.is_some());

      let r = handle.post_message(msg.clone());
      assert!(r.is_ok());

      let maybe_msg = handle.get_event().await.unwrap();
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
      let r = handle.post_message(msg);
      assert!(r.is_ok());
      let event = handle.get_event().await.unwrap();
      assert!(event.is_none());
      handle.sender.close_channel();
    });
    join_handle.join().expect("Failed to join worker thread");
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_worker();
      worker.execute("onmessage = () => { close(); }").unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let r = tokio_util::run_basic(worker);
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    tokio_util::run_basic(async move {
      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = handle.post_message(msg.clone());
      assert!(r.is_ok());
      let event = handle.get_event().await.unwrap();
      assert!(event.is_none());
      handle.sender.close_channel();
    });
    join_handle.join().expect("Failed to join worker thread");
  }
}
