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
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

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
    eprintln!("calling terminate");
    self.terminated.store(true, Ordering::Relaxed);
    self.isolate_handle.terminate_execution();
    let mut sender = self.terminate_tx.clone();
    sender
      .try_send(())
      .map_err(ErrBox::from)
      .expect("Failed to terminate");
    eprintln!("called terminate");
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

  pub terminated: Arc<AtomicBool>,
  terminate_rx: mpsc::Receiver<()>,
  terminate_tx: mpsc::Sender<()>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorker {
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_);
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::web_worker::init(isolate, &state, &worker.internal_channels.sender);
      ops::worker_host::init(isolate, &state);
      ops::io::init(isolate, &state);
      ops::resources::init(isolate, &state);
      ops::errors::init(isolate, &state);
      ops::timers::init(isolate, &state);
      ops::fetch::init(isolate, &state);
    }

    let terminated = Arc::new(AtomicBool::new(false));
    let isolate_handle = worker
      .isolate
      .v8_isolate
      .as_mut()
      .unwrap()
      .thread_safe_handle();
    let (terminate_tx, terminate_rx) = mpsc::channel::<()>(1);

    Self {
      worker,
      event_loop_idle: false,
      terminated,
      terminate_tx,
      terminate_rx,
      isolate_handle,
    }
  }
}

impl WebWorker {
  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WebWorkerHandle {
    WebWorkerHandle {
      worker_handle: self.worker.thread_safe_handle(),
      terminated: self.terminated.clone(),
      isolate_handle: self.isolate_handle.clone(),
      terminate_tx: self.terminate_tx.clone(),
    }
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

    let terminated = inner.terminated.load(Ordering::Relaxed);

    eprintln!("poll web worker {} {}", worker.name, terminated);

    if terminated {
      return Poll::Ready(Ok(()));
    }

    if !inner.event_loop_idle {
      match worker.poll_unpin(cx) {
        Poll::Ready(r) => {
          let terminated = inner.terminated.load(Ordering::Relaxed);
          eprintln!("poll web worker inner {} {}", terminated, worker.name);
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
        Poll::Pending => {
          let terminated = inner.terminated.load(Ordering::Relaxed);
          eprintln!(
            "poll web worker inner pending {} {}",
            terminated, worker.name
          );
        }
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
          debug!("received message from host: {}", msg);
          // TODO: just add second value and then bind using rusty_v8
          // to get structured clone/transfer working
          let script = format!("workerMessageRecvCallback({})", msg);
          let result = worker.execute(&script);

          eprintln!("execute result {:#?}", result);
          // Let worker be polled again
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
      let r = tokio_util::run_basic(worker);
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
      let r = tokio_util::run_basic(worker);
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
