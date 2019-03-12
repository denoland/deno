// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(dead_code)]
use crate::cli::Cli;
use crate::cli::Isolate as CoreIsolate;
use crate::errors::RustOrJsError;
use crate::isolate_state::IsolateState;
use crate::js_errors;
use deno_core::JSError;
use futures::Async;
use futures::Future;
use std::sync::Arc;

/// Wraps deno_core::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
pub struct Isolate {
  inner: CoreIsolate,
  state: Arc<IsolateState>,
}

impl Isolate {
  pub fn new(cli: Cli) -> Isolate {
    let state = cli.state.clone();
    Self {
      inner: CoreIsolate::new(cli),
      state,
    }
  }

  /// Same as execute2() but the filename defaults to "<anonymous>".
  pub fn execute(&self, js_source: &str) -> Result<(), JSError> {
    self.execute2("<anonymous>", js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), JSError> {
    self.inner.execute(js_filename, js_source)
  }

  /// Executes the provided JavaScript module.
  pub fn mod_execute(
    &mut self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    // TODO move isolate_state::mod_execute impl here.
    self
      .state
      .mod_execute(&self.inner, js_filename, is_prefetch)
      .map_err(|err| match err {
        RustOrJsError::Js(err) => RustOrJsError::Js(self.apply_source_map(err)),
        x => x,
      })
  }

  pub fn print_file_info(&self, module: &str) {
    let m = self.state.modules.lock().unwrap();
    m.print_file_info(&self.state.dir, module.to_string());
  }

  /// Applies source map to the error.
  fn apply_source_map(&self, err: JSError) -> JSError {
    js_errors::apply_source_map(&err, &self.state.dir)
  }
}

impl Future for Isolate {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Result<Async<()>, Self::Error> {
    self.inner.poll().map_err(|err| self.apply_source_map(err))
  }
}
