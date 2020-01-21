// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::ops;
use crate::state::ThreadSafeState;
use crate::worker::WorkerChannels;
use crate::worker::WorkerReceiver;
use deno_core;
use deno_core::Buf;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::StartupData;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::sink::SinkExt;
use futures::task::AtomicWaker;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

#[derive(Clone)]
pub struct WebWorker {
  pub name: String,
  pub isolate: Arc<AsyncMutex<Box<deno_core::EsIsolate>>>,
  pub state: ThreadSafeState,
  external_channels: Arc<Mutex<WorkerChannels>>,
}

impl WebWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
    external_channels: WorkerChannels,
  ) -> Self {
    let mut isolate =
      deno_core::EsIsolate::new(Box::new(state.clone()), startup_data, false);

    ops::web_worker::init(&mut isolate, &state);
    ops::worker_host::init(&mut isolate, &state);

    let global_state_ = state.global_state.clone();
    isolate.set_js_error_create(move |v8_exception| {
      JSError::from_v8_exception(v8_exception, &global_state_.ts_compiler)
    });

    Self {
      name,
      isolate: Arc::new(AsyncMutex::new(isolate)),
      state,
      external_channels: Arc::new(Mutex::new(external_channels)),
    }
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), ErrBox> {
    let path = env::current_dir().unwrap().join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.execute2(url.as_str(), js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  fn execute2(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    let mut isolate = self.isolate.try_lock().unwrap();
    isolate.execute(js_filename, js_source)
  }

  /// Executes the provided JavaScript module.
  ///
  /// Takes ownership of the isolate behind mutex.
  pub async fn execute_mod_async(
    &mut self,
    module_specifier: &ModuleSpecifier,
    maybe_code: Option<String>,
    is_prefetch: bool,
  ) -> Result<(), ErrBox> {
    let specifier = module_specifier.to_string();
    let worker = self.clone();

    let mut isolate = self.isolate.lock().await;
    let id = isolate.load_module(&specifier, maybe_code).await?;
    worker.state.global_state.progress.done();

    if !is_prefetch {
      return isolate.mod_evaluate(id);
    }

    Ok(())
  }

  /// Post message to worker as a host.
  ///
  /// This method blocks current thread.
  pub fn post_message(
    &self,
    buf: Buf,
  ) -> impl Future<Output = Result<(), ErrBox>> {
    let channels = self.external_channels.lock().unwrap();
    let mut sender = channels.sender.clone();
    async move {
      let result = sender.send(buf).map_err(ErrBox::from).await;
      drop(sender);
      result
    }
  }

  /// Get message from worker as a host.
  pub fn get_message(&self) -> WorkerReceiver {
    WorkerReceiver {
      channels: self.external_channels.clone(),
    }
  }

  pub fn clear_exception(&mut self) {
    let mut isolate = self.isolate.try_lock().unwrap();
    isolate.clear_exception();
  }
}

impl Future for WebWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let waker = AtomicWaker::new();
    waker.register(cx.waker());
    match inner.isolate.try_lock() {
      Ok(mut isolate) => isolate.poll_unpin(cx),
      Err(_) => {
        waker.wake();
        Poll::Pending
      }
    }
  }
}
