// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compiler::compile_sync;
use crate::compiler::ModuleMetaData;
use crate::errors::DenoError;
use crate::errors::RustOrJsError;
use crate::isolate_state::IsolateState;
use crate::isolate_state::IsolateStateContainer;
use crate::js_check;
use crate::js_errors;
use crate::modules::print_file_info;
use crate::msg;
use crate::tokio_util;
use deno_core;
use deno_core::Behavior;
use deno_core::JSError;
use deno_core::Loader;
use deno_core::RecursiveLoad;
use deno_core::SourceCodeFuture;
use futures::Async;
use futures::Future;
use std::ops::DerefMut;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub trait DenoBehavior: Behavior + IsolateStateContainer + Send {}
impl<T> DenoBehavior for T where T: Behavior + IsolateStateContainer + Send {}

type CoreIsolate<B> = deno_core::Isolate<B>;

/// Wraps deno_core::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
pub struct Isolate<B: Behavior> {
  inner: CoreIsolate<B>,
  state: Arc<IsolateState>,
}

impl<B: 'static + DenoBehavior> Isolate<B> {
  pub fn new(behavior: B) -> Isolate<B> {
    let state = behavior.state().clone();
    Self {
      inner: CoreIsolate::new(behavior),
      state,
    }
  }

  /// Same as execute2() but the filename defaults to "<anonymous>".
  pub fn execute(&mut self, js_source: &str) -> Result<(), JSError> {
    self.execute2("<anonymous>", js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), JSError> {
    self.inner.execute(js_filename, js_source)
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod_async(
    self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> impl Future<Item = Self, Error = (deno_core::Either<DenoError>, Self)>
  {
    let recursive_load = RecursiveLoad::new(js_filename, self);
    recursive_load.and_then(
      move |(id, mut self_)| -> Result<Self, (deno_core::Either<DenoError>, Self)> {
        if !is_prefetch {
          js_check(self_.inner.mod_evaluate(id));
        }

        Ok(self_)
      },
    )
  }

  pub fn execute_mod(
    self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<Self, (RustOrJsError, Self)> {
    tokio_util::block_on(
      self
        .execute_mod_async(js_filename, is_prefetch)
        .map_err(|(err, isolate)| (RustOrJsError::from(err), isolate)),
    )
  }

  pub fn print_file_info(&self, module: &str) {
    let m = self.state.core_modules.lock().unwrap();
    print_file_info(&m, &self.state.dir, module.to_string());
  }

  /// Applies source map to the error.
  fn apply_source_map(&self, err: JSError) -> JSError {
    js_errors::apply_source_map(&err, &self.state.dir)
  }
}

impl<B: DenoBehavior> Loader<DenoError, B> for Isolate<B> {
  /// Returns an absolute URL.
  /// This should be exactly the algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(&mut self, specifier: &str, referrer: &str) -> String {
    let (name, _local_filename) =
      self.state.dir.resolve_module(specifier, referrer).unwrap();
    name
  }

  /// Given an absolute url, load its source code.
  fn load(&mut self, url: &str) -> Box<SourceCodeFuture<DenoError>> {
    self
      .state
      .metrics
      .resolve_count
      .fetch_add(1, Ordering::Relaxed);
    Box::new(
      fetch_module_meta_data_and_maybe_compile_async(&self.state, url, ".")
        .map_err(|err| {
          eprintln!("{}", err);
          err
        }).map(|module_meta_data| module_meta_data.js_source()),
    )
  }

  fn use_isolate<R, F: FnMut(&mut CoreIsolate<B>) -> R>(
    &mut self,
    mut cb: F,
  ) -> R {
    cb(&mut self.inner)
  }

  fn use_modules<R, F: FnMut(&mut deno_core::Modules) -> R>(
    &mut self,
    mut cb: F,
  ) -> R {
    let mut g = self.state.core_modules.lock().unwrap();
    cb(g.deref_mut())
  }
}

impl<B: 'static + DenoBehavior> Future for Isolate<B> {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Result<Async<()>, Self::Error> {
    self.inner.poll().map_err(|err| self.apply_source_map(err))
  }
}

fn fetch_module_meta_data_and_maybe_compile_async(
  state: &Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
) -> impl Future<Item = ModuleMetaData, Error = DenoError> {
  let use_cache = !state.flags.reload;
  let state_ = state.clone();
  let specifier = specifier.to_string();
  let referrer = referrer.to_string();
  state
    .dir
    .fetch_module_meta_data_async(&specifier, &referrer, use_cache)
    .and_then(move |mut out| {
      if out.media_type == msg::MediaType::TypeScript
        && !out.has_output_code_and_source_map()
      {
        debug!(">>>>> compile_sync START");
        eprintln!("Compile {}", out.module_name);
        out = compile_sync(state_.clone(), &specifier, &referrer, &out);
        debug!(">>>>> compile_sync END");
        state_.dir.code_cache(&out)?;
      }
      Ok(out)
    })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cli_behavior::CliBehavior;
  use crate::flags;
  use futures::future::lazy;
  use std::sync::atomic::Ordering;

  #[test]
  fn execute_mod() {
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/esm_imports_a.js");
    let filename = filename.to_str().unwrap().to_string();

    let argv = vec![String::from("./deno"), filename.clone()];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();

    let state = Arc::new(IsolateState::new(flags, rest_argv, None));
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let cli = CliBehavior::new(None, state.clone());
      let isolate = Isolate::new(cli);
      let r = isolate.execute_mod(&filename, false);
      let isolate = match r {
        Err((err, isolate)) => {
          eprintln!("execute_mod err {:?}", err);
          isolate
        }
        Ok(isolate) => isolate,
      };
      tokio_util::panic_on_error(isolate)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }

  #[test]
  fn execute_mod_circular() {
    let filename = std::env::current_dir().unwrap().join("tests/circular1.js");
    let filename = filename.to_str().unwrap().to_string();

    let argv = vec![String::from("./deno"), filename.clone()];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();

    let state = Arc::new(IsolateState::new(flags, rest_argv, None));
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let cli = CliBehavior::new(None, state.clone());
      let isolate = Isolate::new(cli);
      let r = isolate.execute_mod(&filename, false);
      let isolate = match r {
        Err((err, isolate)) => {
          eprintln!("unhandled error {:?}", err);
          isolate
        }
        Ok(isolate) => isolate,
      };
      tokio_util::panic_on_error(isolate)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }
}
