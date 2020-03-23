// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ops;
use crate::state::State;
use crate::web_worker::WebWorker;
use core::task::Context;
use deno_core;
use deno_core::ErrBox;
use deno_core::StartupData;
use futures::future::Future;
use futures::future::FutureExt;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
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
pub struct CompilerWorker(WebWorker);

impl CompilerWorker {
  pub fn new(name: String, startup_data: StartupData, state: State) -> Self {
    let state_ = state.clone();
    let mut worker = WebWorker::new(name, startup_data, state_);
    {
      let isolate = &mut worker.isolate;
      ops::compiler::init(isolate, &state);
      // TODO(bartlomieju): CompilerWorker should not
      // depend on those ops
      ops::os::init(isolate, &state);
      ops::fs::init(isolate, &state);
    }
    Self(worker)
  }
}

impl Deref for CompilerWorker {
  type Target = WebWorker;
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
