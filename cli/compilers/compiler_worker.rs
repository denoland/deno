// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ops;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno_core;
use deno_core::StartupData;
use std::ops::Deref;
use std::ops::DerefMut;

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
pub struct CompilerWorker(Worker);

impl CompilerWorker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
  ) -> Self {
    let state_ = state.clone();
    let mut worker = Worker::new(name, startup_data, state_);
    {
      let isolate = &mut worker.isolate;
      ops::runtime::init(isolate, &state);
      ops::compiler::init(isolate, &state);
      ops::web_worker::init(isolate, &state);
      ops::errors::init(isolate, &state);
      // for compatibility with Worker scope, though unused at
      // the moment
      ops::timers::init(isolate, &state);
      ops::fetch::init(isolate, &state);
      // TODO(bartlomieju): CompilerWorker should not
      // depend on those ops
      ops::os::init(isolate, &state);
      ops::files::init(isolate, &state);
      ops::fs::init(isolate, &state);
      ops::io::init(isolate, &state);
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
