// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compiler::compile_async;
use crate::compiler::ModuleMetaData;
use crate::errors::DenoError;
use crate::errors::RustOrJsError;
use crate::js_errors;
use crate::js_errors::JSErrorColor;
use crate::msg;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno;
use deno::deno_mod;
use deno::JSError;
use deno::StartupData;
use futures::future::Either;
use futures::Async;
use futures::Future;
use std::sync::atomic::Ordering;

/// Wraps deno::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
pub struct Worker {
  inner: deno::Isolate<ThreadSafeState>,
  state: ThreadSafeState,
}

impl Worker {
  pub fn new(
    _name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
  ) -> Worker {
    let state_ = state.clone();
    Self {
      inner: deno::Isolate::new(startup_data, state_),
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

        // The resolved module is an alias to another module (due to redirects).
        // Save such alias to the module map.
        if out.module_redirect_source_name.is_some() {
          self.mod_alias(
            &out.module_redirect_source_name.clone().unwrap(),
            &out.module_name,
          );
        }

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
    // TODO move state::execute_mod impl here.
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

    // Be careful.
    // url might not match the actual out.module_name
    // due to the mechanism of redirection.

    let id = self
      .mod_new_and_register(true, &out.module_name.clone(), &out.js_source())
      .map_err(RustOrJsError::from)?;

    // The resolved module is an alias to another module (due to redirects).
    // Save such alias to the module map.
    if out.module_redirect_source_name.is_some() {
      self.mod_alias(
        &out.module_redirect_source_name.clone().unwrap(),
        &out.module_name,
      );
    }

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

  /// Create an alias for another module.
  /// The alias could later be used to grab the module
  /// which `target` points to.
  fn mod_alias(&self, name: &str, target: &str) {
    self.state.modules.lock().unwrap().alias(name, target);
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

impl Future for Worker {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Result<Async<()>, Self::Error> {
    self.inner.poll().map_err(|err| self.apply_source_map(err))
  }
}

fn fetch_module_meta_data_and_maybe_compile_async(
  state: &ThreadSafeState,
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
    .and_then(move |out| {
      if out.media_type == msg::MediaType::TypeScript
        && !out.has_output_code_and_source_map()
      {
        debug!(">>>>> compile_sync START");
        Either::A(
          compile_async(state_.clone(), &specifier, &referrer, &out)
            .map_err(|e| {
              debug!("compiler error exiting!");
              eprintln!("{}", JSErrorColor(&e).to_string());
              std::process::exit(1);
            }).and_then(move |out| {
              debug!(">>>>> compile_sync END");
              state_.dir.code_cache(&out)?;
              Ok(out)
            }),
        )
      } else {
        Either::B(futures::future::ok(out))
      }
    })
}

fn fetch_module_meta_data_and_maybe_compile(
  state: &ThreadSafeState,
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
  use crate::flags;
  use crate::ops::op_selector_std;
  use crate::resources;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use deno::js_check;
  use futures::future::lazy;
  use std::sync::atomic::Ordering;
  use std::thread;

  #[test]
  fn execute_mod() {
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/esm_imports_a.js");
    let filename = filename.to_str().unwrap().to_string();

    let argv = vec![String::from("./deno"), filename.clone()];
    let (flags, rest_argv) = flags::set_flags(argv).unwrap();

    let state = ThreadSafeState::new(flags, rest_argv, op_selector_std);
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      if let Err(err) = worker.execute_mod(&filename, false) {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn execute_mod_circular() {
    let filename = std::env::current_dir().unwrap().join("tests/circular1.js");
    let filename = filename.to_str().unwrap().to_string();

    let argv = vec![String::from("./deno"), filename.clone()];
    let (flags, rest_argv) = flags::set_flags(argv).unwrap();

    let state = ThreadSafeState::new(flags, rest_argv, op_selector_std);
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      if let Err(err) = worker.execute_mod(&filename, false) {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }

  fn create_test_worker() -> Worker {
    let state = ThreadSafeState::mock();
    let mut worker =
      Worker::new("TEST".to_string(), startup_data::deno_isolate_init(), state);
    js_check(worker.execute("denoMain()"));
    js_check(worker.execute("workerMain()"));
    worker
  }

  #[test]
  fn test_worker_messages() {
    tokio_util::init(|| {
      let mut worker = create_test_worker();
      let source = r#"
        onmessage = function(e) {
          console.log("msg from main script", e.data);
          if (e.data == "exit") {
            close();
            return;
          } else {
            console.assert(e.data === "hi");
          }
          postMessage([1, 2, 3]);
          console.log("after postMessage");
        }
        "#;
      js_check(worker.execute(source));

      let resource = worker.state.resource.clone();
      let resource_ = resource.clone();

      tokio::spawn(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          js_check(r);
          Ok(())
        })
      }));

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

      let r = resources::post_message_to_worker(resource.rid, msg).wait();
      assert!(r.is_ok());

      let maybe_msg = resources::get_message_from_worker(resource.rid)
        .wait()
        .unwrap();
      assert!(maybe_msg.is_some());
      // Check if message received is [1, 2, 3] in json
      assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = resources::post_message_to_worker(resource.rid, msg).wait();
      assert!(r.is_ok());
    })
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    tokio_util::init(|| {
      let mut worker = create_test_worker();
      js_check(
        worker.execute("onmessage = () => { delete window['onmessage']; }"),
      );

      let resource = worker.state.resource.clone();
      let rid = resource.rid;

      tokio::spawn(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource.close();
          println!("workers.rs after resource close");
          js_check(r);
          Ok(())
        })
      }));

      assert_eq!(resources::get_type(rid), Some("worker".to_string()));

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = resources::post_message_to_worker(rid, msg).wait();
      assert!(r.is_ok());
      debug!("rid {:?}", rid);

      // TODO Need a way to get a future for when a resource closes.
      // For now, just sleep for a bit.
      thread::sleep(std::time::Duration::from_millis(1000));
      assert_eq!(resources::get_type(rid), None);
    })
  }
}
