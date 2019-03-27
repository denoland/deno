// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compiler::compile_sync;
use crate::compiler::ModuleMetaData;
use crate::errors::DenoError;
use crate::errors::RustOrJsError;
use crate::isolate_state::IsolateState;
use crate::isolate_state::IsolateStateContainer;
use crate::js_errors;
use crate::msg;
use crate::tokio_util;
use deno_core;
use deno_core::deno_mod;
use deno_core::Behavior;
use deno_core::JSError;
use futures::Async;
use futures::Future;
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

impl<B: DenoBehavior> Isolate<B> {
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

  // TODO(ry) make this return a future.
  fn mod_load_deps(&self, id: deno_mod) -> Result<(), RustOrJsError> {
    // basically iterate over the imports, start loading them.

    let referrer_name = {
      let g = self.state.modules.lock().unwrap();
      g.get_name(id).unwrap().clone()
    };

    for specifier in self.inner.mod_get_imports(id) {
      let (name, _local_filename) = self
        .state
        .dir
        .resolve_module(&specifier, &referrer_name)
        .map_err(DenoError::from)
        .map_err(RustOrJsError::from)?;

      debug!("mod_load_deps {}", name);

      if !self.state.modules.lock().unwrap().is_registered(&name) {
        let out = fetch_module_meta_data_and_maybe_compile(
          &self.state,
          &specifier,
          &referrer_name,
        )?;
        let child_id = self.mod_new_and_register(
          false,
          &out.module_name.clone(),
          &out.js_source(),
        )?;

        self.mod_load_deps(child_id)?;
      }
    }

    Ok(())
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod(
    &mut self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    // TODO move isolate_state::execute_mod impl here.
    self
      .execute_mod_inner(js_filename, is_prefetch)
      .map_err(|err| match err {
        RustOrJsError::Js(err) => RustOrJsError::Js(self.apply_source_map(err)),
        x => x,
      })
  }

  /// High-level way to execute modules.
  /// This will issue HTTP requests and file system calls.
  /// Blocks. TODO(ry) Don't block.
  fn execute_mod_inner(
    &mut self,
    url: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    let out = fetch_module_meta_data_and_maybe_compile(&self.state, url, ".")
      .map_err(RustOrJsError::from)?;

    let id = self
      .mod_new_and_register(true, &out.module_name.clone(), &out.js_source())
      .map_err(RustOrJsError::from)?;

    self.mod_load_deps(id)?;

    let state = self.state.clone();

    let mut resolve = move |specifier: &str, referrer: deno_mod| -> deno_mod {
      state.metrics.resolve_count.fetch_add(1, Ordering::Relaxed);
      let mut modules = state.modules.lock().unwrap();
      modules.resolve_cb(&state.dir, specifier, referrer)
    };

    self
      .inner
      .mod_instantiate(id, &mut resolve)
      .map_err(RustOrJsError::from)?;
    if !is_prefetch {
      self.inner.mod_evaluate(id).map_err(RustOrJsError::from)?;
    }
    Ok(())
  }

  /// Wraps Isolate::mod_new but registers with modules.
  fn mod_new_and_register(
    &self,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<deno_mod, JSError> {
    let id = self.inner.mod_new(main, name, source)?;
    self.state.modules.lock().unwrap().register(id, &name);
    Ok(id)
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

impl<B: DenoBehavior> Future for Isolate<B> {
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
  let state_ = state.clone();
  let specifier = specifier.to_string();
  let referrer = referrer.to_string();
  state
    .dir
    .fetch_module_meta_data_async(&specifier, &referrer)
    .and_then(move |mut out| {
      if (out.media_type == msg::MediaType::TypeScript
        && out.maybe_output_code.is_none())
        || state_.flags.recompile
      {
        debug!(">>>>> compile_sync START");
        out = compile_sync(state_.clone(), &specifier, &referrer, &out);
        debug!(">>>>> compile_sync END");
        state_.dir.code_cache(&out)?;
      }
      Ok(out)
    })
}

fn fetch_module_meta_data_and_maybe_compile(
  state: &Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
) -> Result<ModuleMetaData, DenoError> {
  tokio_util::block_on(fetch_module_meta_data_and_maybe_compile_async(
    state, specifier, referrer,
  ))
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
      let mut isolate = Isolate::new(cli);
      if let Err(err) = isolate.execute_mod(&filename, false) {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(isolate)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 1);
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
      let mut isolate = Isolate::new(cli);
      if let Err(err) = isolate.execute_mod(&filename, false) {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(isolate)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }
}
