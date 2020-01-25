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

/// This worker is used to host TypeScript and WASM compilers.
///
/// It provides minimal set of ops that are necessary to facilitate
/// compilation.
///
/// NOTE: This worker is considered priveleged, because it may
/// access file system without permission check.
///
/// At the moment this worker is meant to be single-use - after
/// performing single compilation/bundling it should be destroyed.
///
/// TODO(bartlomieju): add support to reuse the worker - or in other
/// words support stateful TS compiler
#[derive(Clone)]
pub struct CompilerWorker(Worker);

impl CompilerWorker {
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
      ops::compiler::init(&mut isolate, &state);
      ops::web_worker::init(&mut isolate, &state);
      // TODO(bartlomieju): CompilerWorker should not
      // depend on those ops
      ops::os::init(&mut isolate, &state);
      ops::files::init(&mut isolate, &state);
      ops::fs::init(&mut isolate, &state);
      ops::io::init(&mut isolate, &state);
    }

    Self(worker)
  }
}

impl Deref for CompilerWorker {
  type Target = Worker;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for CompilerWorker {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Future for CompilerWorker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    inner.0.poll_unpin(cx)
  }
}
