// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::fmt_errors::PrettyJsError;
use crate::inspector::DenoInspector;
use crate::js;
use crate::metrics::Metrics;
use crate::module_loader::CliModuleLoader;
use crate::ops;
use crate::permissions::Permissions;
use crate::program_state::ProgramState;
use crate::source_maps::apply_source_map;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::poll_fn;
use deno_core::futures::future::FutureExt;
use deno_core::futures::stream::StreamExt;
use deno_core::futures::task::AtomicWaker;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::RuntimeOptions;
use std::env;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;

/// Events that are sent to host from child
/// worker.
pub enum WorkerEvent {
  Message(Box<[u8]>),
  Error(AnyError),
  TerminalError(AnyError),
}

pub struct WorkerChannelsInternal {
  pub sender: mpsc::Sender<WorkerEvent>,
  pub receiver: mpsc::Receiver<Box<[u8]>>,
}

/// Wrapper for `WorkerHandle` that adds functionality
/// for terminating workers.
///
/// This struct is used by host as well as worker itself.
///
/// Host uses it to communicate with worker and terminate it,
/// while worker uses it only to finish execution on `self.close()`.
#[derive(Clone)]
pub struct WebWorkerHandle {
  pub sender: mpsc::Sender<Box<[u8]>>,
  pub receiver: Arc<AsyncMutex<mpsc::Receiver<WorkerEvent>>>,
  terminate_tx: mpsc::Sender<()>,
  terminated: Arc<AtomicBool>,
  isolate_handle: v8::IsolateHandle,
}

impl WebWorkerHandle {
  /// Post message to worker as a host.
  pub fn post_message(&self, buf: Box<[u8]>) -> Result<(), AnyError> {
    let mut sender = self.sender.clone();
    sender.try_send(buf)?;
    Ok(())
  }

  /// Get the event with lock.
  /// Return error if more than one listener tries to get event
  pub async fn get_event(&self) -> Result<Option<WorkerEvent>, AnyError> {
    let mut receiver = self.receiver.try_lock()?;
    Ok(receiver.next().await)
  }

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

fn create_channels(
  isolate_handle: v8::IsolateHandle,
  terminate_tx: mpsc::Sender<()>,
) -> (WorkerChannelsInternal, WebWorkerHandle) {
  let (in_tx, in_rx) = mpsc::channel::<Box<[u8]>>(1);
  let (out_tx, out_rx) = mpsc::channel::<WorkerEvent>(1);
  let internal_channels = WorkerChannelsInternal {
    sender: out_tx,
    receiver: in_rx,
  };
  let external_channels = WebWorkerHandle {
    sender: in_tx,
    receiver: Arc::new(AsyncMutex::new(out_rx)),
    terminated: Arc::new(AtomicBool::new(false)),
    terminate_tx,
    isolate_handle,
  };
  (internal_channels, external_channels)
}

/// This struct is an implementation of `Worker` Web API
///
/// Each `WebWorker` is either a child of `MainWorker` or other
/// `WebWorker`.
pub struct WebWorker {
  inspector: Option<Box<DenoInspector>>,
  // Following fields are pub because they are accessed
  // when creating a new WebWorker instance.
  pub(crate) internal_channels: WorkerChannelsInternal,
  pub(crate) js_runtime: JsRuntime,
  pub(crate) name: String,
  waker: AtomicWaker,
  event_loop_idle: bool,
  terminate_rx: mpsc::Receiver<()>,
  handle: WebWorkerHandle,
  pub has_deno_namespace: bool,
}

impl WebWorker {
  pub fn new(
    name: String,
    permissions: Permissions,
    main_module: ModuleSpecifier,
    program_state: Arc<ProgramState>,
    has_deno_namespace: bool,
  ) -> Self {
    let module_loader =
      CliModuleLoader::new_for_worker(program_state.flags.unstable);
    let global_state_ = program_state.clone();

    let js_error_create_fn = Box::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, global_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(module_loader),
      startup_snapshot: Some(js::deno_isolate_init()),
      js_error_create_fn: Some(js_error_create_fn),
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      ..Default::default()
    });

    let inspector =
      if let Some(inspector_server) = &program_state.maybe_inspector_server {
        Some(DenoInspector::new(
          &mut js_runtime,
          Some(inspector_server.clone()),
        ))
      } else if program_state.flags.coverage || program_state.flags.repl {
        Some(DenoInspector::new(&mut js_runtime, None))
      } else {
        None
      };

    let (terminate_tx, terminate_rx) = mpsc::channel::<()>(1);
    let isolate_handle = js_runtime.v8_isolate().thread_safe_handle();
    let (internal_channels, handle) =
      create_channels(isolate_handle, terminate_tx);

    let mut worker = Self {
      inspector,
      internal_channels,
      js_runtime,
      name,
      waker: AtomicWaker::new(),
      event_loop_idle: false,
      terminate_rx,
      handle,
      has_deno_namespace,
    };

    {
      let handle = worker.thread_safe_handle();
      let sender = worker.internal_channels.sender.clone();
      let js_runtime = &mut worker.js_runtime;
      // All ops registered in this function depend on these
      {
        let op_state = js_runtime.op_state();
        let mut op_state = op_state.borrow_mut();
        op_state.put::<Metrics>(Default::default());
        op_state.put::<Arc<ProgramState>>(program_state.clone());
        op_state.put::<Permissions>(permissions);
      }

      ops::web_worker::init(js_runtime, sender.clone(), handle);
      ops::runtime::init(js_runtime, main_module, true);
      ops::fetch::init(js_runtime, program_state.flags.ca_file.as_deref());
      ops::timers::init(js_runtime);
      ops::worker_host::init(js_runtime, Some(sender));
      ops::reg_json_sync(js_runtime, "op_close", deno_core::op_close);
      ops::reg_json_sync(js_runtime, "op_resources", deno_core::op_resources);
      ops::reg_json_sync(
        js_runtime,
        "op_domain_to_ascii",
        deno_web::op_domain_to_ascii,
      );
      ops::errors::init(js_runtime);
      ops::io::init(js_runtime);
      ops::websocket::init(js_runtime);

      if has_deno_namespace {
        ops::fs_events::init(js_runtime);
        ops::fs::init(js_runtime);
        ops::net::init(js_runtime);
        ops::os::init(js_runtime);
        ops::permissions::init(js_runtime);
        ops::plugin::init(js_runtime);
        ops::process::init(js_runtime);
        ops::crypto::init(js_runtime, program_state.flags.seed);
        ops::runtime_compiler::init(js_runtime);
        ops::signal::init(js_runtime);
        ops::tls::init(js_runtime);
        ops::tty::init(js_runtime);
      }
    }

    worker
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), AnyError> {
    let path = env::current_dir().unwrap().join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.js_runtime.execute(url.as_str(), js_source)
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let id = self.js_runtime.load_module(module_specifier, None).await?;
    self.js_runtime.mod_evaluate(id).await
  }

  /// Returns a way to communicate with the Worker from other threads.
  pub fn thread_safe_handle(&self) -> WebWorkerHandle {
    self.handle.clone()
  }

  pub fn poll_event_loop(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<(), AnyError>> {
    let terminated = self.handle.terminated.load(Ordering::Relaxed);

    if terminated {
      return Poll::Ready(Ok(()));
    }

    if !self.event_loop_idle {
      let poll_result = {
        // We always poll the inspector if it exists.
        let _ = self.inspector.as_mut().map(|i| i.poll_unpin(cx));
        self.waker.register(cx.waker());
        self.js_runtime.poll_event_loop(cx)
      };
      match poll_result {
        Poll::Ready(r) => {
          let terminated = self.handle.terminated.load(Ordering::Relaxed);
          if terminated {
            return Poll::Ready(Ok(()));
          }

          if let Err(e) = r {
            eprintln!(
              "{}: Uncaught (in worker \"{}\") {}",
              colors::red_bold("error"),
              self.name.to_string(),
              e.to_string().trim_start_matches("Uncaught "),
            );
            let mut sender = self.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }
          self.event_loop_idle = true;
        }
        Poll::Pending => {}
      }
    }

    if let Poll::Ready(r) = self.terminate_rx.poll_next_unpin(cx) {
      // terminate_rx should never be closed
      assert!(r.is_some());
      return Poll::Ready(Ok(()));
    }

    if let Poll::Ready(r) = self.internal_channels.receiver.poll_next_unpin(cx)
    {
      match r {
        Some(msg) => {
          let msg = String::from_utf8(msg.to_vec()).unwrap();
          let script = format!("workerMessageRecvCallback({})", msg);

          if let Err(e) = self.execute(&script) {
            // If execution was terminated during message callback then
            // just ignore it
            if self.handle.terminated.load(Ordering::Relaxed) {
              return Poll::Ready(Ok(()));
            }

            // Otherwise forward error to host
            let mut sender = self.internal_channels.sender.clone();
            sender
              .try_send(WorkerEvent::Error(e))
              .expect("Failed to post message to host");
          }

          // Let event loop be polled again
          self.event_loop_idle = false;
          self.waker.wake();
        }
        None => unreachable!(),
      }
    }

    Poll::Pending
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    poll_fn(|cx| self.poll_event_loop(cx)).await
  }
}

impl Drop for WebWorker {
  fn drop(&mut self) {
    // The Isolate object must outlive the Inspector object, but this is
    // currently not enforced by the type system.
    self.inspector.take();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::program_state::ProgramState;
  use crate::tokio_util;
  use deno_core::serde_json::json;

  fn create_test_web_worker() -> WebWorker {
    let main_module =
      ModuleSpecifier::resolve_url_or_path("./hello.js").unwrap();
    let program_state = ProgramState::mock(vec!["deno".to_string()], None);
    let mut worker = WebWorker::new(
      "TEST".to_string(),
      Permissions::allow_all(),
      main_module,
      program_state,
      false,
    );
    worker
      .execute("bootstrap.workerRuntime(\"TEST\", false)")
      .unwrap();
    worker
  }

  #[tokio::test]
  async fn test_worker_messages() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_web_worker();
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
      let r = tokio_util::run_basic(worker.run_event_loop());
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

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
    join_handle.join().expect("Failed to join worker thread");
  }

  #[tokio::test]
  async fn removed_from_resource_table_on_close() {
    let (handle_sender, handle_receiver) =
      std::sync::mpsc::sync_channel::<WebWorkerHandle>(1);

    let join_handle = std::thread::spawn(move || {
      let mut worker = create_test_web_worker();
      worker.execute("onmessage = () => { close(); }").unwrap();
      let handle = worker.thread_safe_handle();
      handle_sender.send(handle).unwrap();
      let r = tokio_util::run_basic(worker.run_event_loop());
      assert!(r.is_ok())
    });

    let mut handle = handle_receiver.recv().unwrap();

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    let r = handle.post_message(msg.clone());
    assert!(r.is_ok());
    let event = handle.get_event().await.unwrap();
    assert!(event.is_none());
    handle.sender.close_channel();

    join_handle.join().expect("Failed to join worker thread");
  }
}
