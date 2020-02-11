// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::futures::SinkExt;
use crate::global_state::GlobalState;
use crate::ops;
use crate::permissions::DenoPermissions;
use crate::startup_data;
use crate::state::State;
use crate::tokio_util;
use crate::worker::Worker;
use crate::worker::WorkerEvent;
use crate::worker::WorkerHandle;
use deno_core;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use futures::future::FutureExt;
use std::ops::Deref;
use std::ops::DerefMut;
use std::thread::JoinHandle;

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

fn create_web_worker(
  name: String,
  global_state: GlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
) -> Result<WebWorker, ErrBox> {
  let state =
    State::new_for_worker(global_state, Some(permissions), specifier)?;

  let mut worker =
    WebWorker::new(name.to_string(), startup_data::deno_isolate_init(), state);
  let script = format!("bootstrapWorkerRuntime(\"{}\")", name);
  worker.execute(&script)?;

  Ok(worker)
}

// TODO(bartlomieju): check if order of actions is aligned to Worker spec
pub fn run_in_thread(
  name: String,
  global_state: GlobalState,
  permissions: DenoPermissions,
  specifier: ModuleSpecifier,
  has_source_code: bool,
  source_code: String,
) -> Result<(JoinHandle<()>, WorkerHandle), ErrBox> {
  let (handle_sender, handle_receiver) =
    std::sync::mpsc::sync_channel::<Result<WorkerHandle, ErrBox>>(1);

  let builder =
    std::thread::Builder::new().name(format!("deno-worker-{}", name));
  let join_handle = builder.spawn(move || {
    // Any error inside this block is terminal:
    // - JS worker is useless - meaning it throws an exception and can't do anything else,
    //  all action done upon it should be noops
    // - newly spawned thread exits
    let result =
      create_web_worker(name, global_state, permissions, specifier.clone());

    if let Err(err) = result {
      handle_sender.send(Err(err)).unwrap();
      return;
    }

    let mut worker = result.unwrap();
    // Send thread safe handle to newly created worker to host thread
    handle_sender.send(Ok(worker.thread_safe_handle())).unwrap();
    drop(handle_sender);

    // At this point the only method of communication with host
    // is using `worker.internal_channels`.
    //
    // Host can already push messages and interact with worker.
    //
    // Next steps:
    // - create tokio runtime
    // - load provided module or code
    // - start driving worker's event loop

    let mut rt = tokio_util::create_basic_runtime();

    // TODO: run using select with terminate

    // Execute provided source code immediately
    let result = if has_source_code {
      worker.execute(&source_code)
    } else {
      // TODO(bartlomieju): add "type": "classic", ie. ability to load
      // script instead of module
      let load_future = worker
        .execute_mod_async(&specifier, None, false)
        .boxed_local();

      rt.block_on(load_future)
    };

    if let Err(e) = result {
      let mut sender = worker.internal_channels.sender.clone();
      futures::executor::block_on(sender.send(WorkerEvent::Error(e)))
        .expect("Failed to post message to host");

      // Failure to execute script is a terminal error, bye, bye.
      return;
    }

    rt.block_on(worker.run_event_loop())
      .expect("Unexpected error in worker loop");
  })?;

  let worker_handle = handle_receiver.recv().unwrap()?;

  Ok((join_handle, worker_handle))
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
      let mut rt = tokio_util::create_basic_runtime();
      let r = rt.block_on(worker.run_event_loop());
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
      let r = rt.block_on(worker.run_event_loop());
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
