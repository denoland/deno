// Copyright 2018-2025 the Deno authors. MIT license.

use super::Output;
use super::Snapshot;
use super::create_runtime_from_snapshot;
use super::run_async;
use anyhow::anyhow;
use deno_core::GarbageCollected;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::v8::IsolateHandle;
use deno_error::JsErrorBox;
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::task::Poll;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::watch;

/// Our cppgc object.
#[derive(Debug)]
pub struct WorkerControl {
  worker_channel: WorkerChannel,
  close_watcher: WorkerCloseWatcher,
  handle: Option<IsolateHandle>,
  shutdown_flag: Option<UnboundedSender<()>>,
}

unsafe impl GarbageCollected for WorkerControl {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"WorkerControl"
  }
}

#[derive(Debug)]
pub struct WorkerChannel {
  tx: UnboundedSender<String>,
  rx: Mutex<UnboundedReceiver<String>>,
}

#[derive(Debug, Clone)]
pub struct WorkerCloseWatcher {
  watcher: Arc<Mutex<watch::Receiver<bool>>>,
}

#[derive(Debug)]
pub struct Worker {
  _close_send: watch::Sender<bool>,
  pub(crate) close_watcher: WorkerCloseWatcher,
  parent_channel: std::sync::Mutex<Option<WorkerChannel>>,
  parent_close_watcher: std::sync::Mutex<Option<WorkerCloseWatcher>>,
}

pub struct WorkerHostSide {
  worker_channel: WorkerChannel,
  close_watcher: WorkerCloseWatcher,
}

pub fn worker_create(
  parent: Option<WorkerCloseWatcher>,
) -> (Worker, WorkerHostSide) {
  let (tx1, rx1) = unbounded_channel();
  let (tx2, rx2) = unbounded_channel();
  let (_close_send, close_recv) = watch::channel(false);
  let close_watcher = WorkerCloseWatcher {
    watcher: Arc::new(Mutex::new(close_recv)),
  };
  let worker = Worker {
    _close_send,
    close_watcher: close_watcher.clone(),
    parent_channel: parent
      .as_ref()
      .map(move |_| WorkerChannel {
        tx: tx1,
        rx: rx2.into(),
      })
      .into(),
    parent_close_watcher: parent.into(),
  };
  let worker_host_side = WorkerHostSide {
    close_watcher,
    worker_channel: WorkerChannel {
      tx: tx2,
      rx: rx1.into(),
    },
  };
  (worker, worker_host_side)
}

#[op2]
#[cppgc]
pub fn op_worker_spawn(
  state: &OpState,
  #[string] base_url: String,
  #[string] main_script: String,
) -> Result<WorkerControl, std::sync::mpsc::RecvError> {
  let this_worker = state.borrow::<Worker>();
  let output = state.borrow::<Output>().clone();
  let snapshot = state.borrow::<Snapshot>();
  let snapshot = snapshot.0;
  let close_watcher = this_worker.close_watcher.clone();
  let (init_send, init_recv) = channel();
  let (shutdown_tx, shutdown_rx) = unbounded_channel();
  std::thread::spawn(move || {
    let (mut runtime, worker_host_side) = create_runtime_from_snapshot(
      snapshot,
      false,
      Some(close_watcher),
      vec![],
    );
    runtime.op_state().borrow_mut().put(output.clone());
    init_send
      .send(WorkerControl {
        worker_channel: worker_host_side.worker_channel,
        close_watcher: worker_host_side.close_watcher,
        handle: Some(runtime.v8_isolate().thread_safe_handle()),
        shutdown_flag: Some(shutdown_tx),
      })
      .map_err(|_| unreachable!())
      .unwrap();
    run_async(run_worker_task(runtime, base_url, main_script, shutdown_rx));
  });

  // This is technically a blocking call
  let worker = init_recv.recv()?;
  Ok(worker)
}

async fn run_worker_task(
  mut runtime: JsRuntime,
  base_url: String,
  main_script: String,
  mut shutdown_rx: UnboundedReceiver<()>,
) -> Result<(), anyhow::Error> {
  let url = Url::try_from(base_url.as_str())?.join(&main_script)?;
  let module = runtime.load_main_es_module(&url).await?;
  let f = runtime.mod_evaluate(module);
  // We need this structure for the shutdown code to ensure that the output is
  // consistent whether the v8 termination signal is sent, or the shutdown_rx is
  // triggered.
  if let Err(e) = poll_fn(|cx| {
    if shutdown_rx.poll_recv(cx).is_ready() {
      // This matches the v8 error. We'll hit both, depending on timing.
      return Poll::Ready(Err(anyhow!("Uncaught Error: execution terminated")));
    }
    runtime
      .poll_event_loop(cx, PollEventLoopOptions::default())
      .map_err(|e| e.into())
  })
  .await
  {
    let state = runtime.op_state().clone();
    let state = state.borrow();
    let output: &Output = state.borrow();
    for line in e.to_string().split('\n') {
      println!("[ERR] {line}");
      output.line(format!("[ERR] {line}"));
    }
    return Ok(());
  } else if let Err(e) = f.await {
    let state = runtime.op_state().clone();
    let state = state.borrow();
    let output: &Output = state.borrow();
    for line in e.to_string().split('\n') {
      println!("[ERR] {line}");
      output.line(format!("[ERR] {line}"));
    }
    return Ok(());
  }

  Ok(())
}

#[op2(fast)]
pub fn op_worker_send(
  #[cppgc] worker: &WorkerControl,
  #[string] message: String,
) -> Result<(), tokio::sync::mpsc::error::SendError<String>> {
  worker.worker_channel.tx.send(message)?;
  Ok(())
}

#[op2]
#[string]
pub async fn op_worker_recv(#[cppgc] worker: &WorkerControl) -> Option<String> {
  worker.worker_channel.rx.lock().await.recv().await
}

#[op2]
#[cppgc]
pub fn op_worker_parent(
  state: Rc<RefCell<OpState>>,
) -> Result<WorkerControl, JsErrorBox> {
  let state = state.borrow_mut();
  let worker: &Worker = state.borrow();
  let (Some(worker_channel), Some(close_watcher)) = (
    worker.parent_channel.lock().unwrap().take(),
    worker.parent_close_watcher.lock().unwrap().take(),
  ) else {
    return Err(JsErrorBox::generic("No parent worker is available"));
  };
  Ok(WorkerControl {
    worker_channel,
    close_watcher,
    handle: None,
    shutdown_flag: None,
  })
}

#[op2]
pub async fn op_worker_await_close(#[cppgc] worker: &WorkerControl) {
  loop {
    if worker
      .close_watcher
      .watcher
      .lock()
      .await
      .changed()
      .await
      .is_err()
    {
      break;
    }
  }
}

#[op2(fast)]
pub fn op_worker_terminate(
  #[cppgc] worker: &WorkerControl,
  state: Rc<RefCell<OpState>>,
) {
  worker.handle.as_ref().unwrap().terminate_execution();
  _ = worker.shutdown_flag.as_ref().unwrap().send(());
  state.borrow_mut().waker.wake();
}
