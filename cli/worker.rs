// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::DenoError;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno;
use deno::ModuleSpecifier;
use deno::StartupData;
use futures::Async;
use futures::Future;
use std::sync::Arc;
use std::sync::Mutex;

/// Wraps deno::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
#[derive(Clone)]
pub struct Worker {
  isolate: Arc<Mutex<deno::Isolate>>,
  pub state: ThreadSafeState,
}

impl Worker {
  pub fn new(
    _name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
  ) -> Worker {
    let state_ = state.clone();
    let isolate = Arc::new(Mutex::new(deno::Isolate::new(startup_data, false)));
    {
      let mut i = isolate.lock().unwrap();
      i.set_dispatch(move |control_buf, zero_copy_buf| {
        state_.dispatch(control_buf, zero_copy_buf)
      });
    }
    Self { isolate, state }
  }

  /// Same as execute2() but the filename defaults to "<anonymous>".
  pub fn execute(&mut self, js_source: &str) -> Result<(), DenoError> {
    self.execute2("<anonymous>", js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), DenoError> {
    let mut isolate = self.isolate.lock().unwrap();
    match isolate.execute(js_filename, js_source) {
      Ok(_) => Ok(()),
      Err(err) => Err(DenoError::from(err)),
    }
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod_async(
    &mut self,
    module_specifier: &ModuleSpecifier,
    is_prefetch: bool,
  ) -> impl Future<Item = (), Error = DenoError> {
    let worker = self.clone();
    let worker_ = worker.clone();
    let loader = self.state.clone();
    let isolate = self.isolate.clone();
    let modules = self.state.modules.clone();
    let recursive_load = deno::RecursiveLoad::new(
      &module_specifier.to_string(),
      loader,
      isolate,
      modules,
    );
    recursive_load
      .and_then(move |id| -> Result<(), deno::JSErrorOr<DenoError>> {
        worker.state.progress.done();
        if is_prefetch {
          Ok(())
        } else {
          let mut isolate = worker.isolate.lock().unwrap();
          let result = isolate.mod_evaluate(id);
          if let Err(err) = result {
            Err(deno::JSErrorOr::JSError(err))
          } else {
            Ok(())
          }
        }
      }).map_err(move |err| {
        worker_.state.progress.done();
        // Convert to DenoError AND apply_source_map.
        match err {
          deno::JSErrorOr::JSError(err) => {
            worker_.apply_source_map(DenoError::from(err))
          }
          deno::JSErrorOr::Other(err) => err,
        }
      })
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod(
    &mut self,
    module_specifier: &ModuleSpecifier,
    is_prefetch: bool,
  ) -> Result<(), DenoError> {
    tokio_util::block_on(self.execute_mod_async(module_specifier, is_prefetch))
  }

  /// Applies source map to the error.
  fn apply_source_map(&self, err: DenoError) -> DenoError {
    err.apply_source_map(&self.state.dir)
  }
}

impl Future for Worker {
  type Item = ();
  type Error = DenoError;

  fn poll(&mut self) -> Result<Async<()>, Self::Error> {
    let mut isolate = self.isolate.lock().unwrap();
    isolate
      .poll()
      .map_err(|err| self.apply_source_map(DenoError::from(err)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::deno_error::err_check;
  use crate::flags;
  use crate::ops::op_selector_std;
  use crate::progress::Progress;
  use crate::resources;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use futures::future::lazy;
  use std::sync::atomic::Ordering;

  #[test]
  fn execute_mod_esm_imports_a() {
    let module_specifier =
      ModuleSpecifier::resolve_root("tests/esm_imports_a.js").unwrap();
    let argv = vec![String::from("./deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      op_selector_std,
      Progress::new(),
    );
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_mod(&module_specifier, false);
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_mod_circular() {
    let module_specifier =
      ModuleSpecifier::resolve_root("tests/circular1.js").unwrap();
    let argv = vec![String::from("./deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      op_selector_std,
      Progress::new(),
    );
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_mod(&module_specifier, false);
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_006_url_imports() {
    let module_specifier =
      ModuleSpecifier::resolve_root("tests/006_url_imports.ts").unwrap();
    let argv = vec![String::from("deno"), module_specifier.to_string()];
    let mut flags = flags::DenoFlags::default();
    flags.reload = true;
    let state =
      ThreadSafeState::new(flags, argv, op_selector_std, Progress::new());
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker = Worker::new(
        "TEST".to_string(),
        startup_data::deno_isolate_init(),
        state,
      );
      err_check(worker.execute("denoMain()"));
      let result = worker.execute_mod(&module_specifier, false);
      if let Err(err) = result {
        eprintln!("execute_mod err {:?}", err);
      }
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 3);
    // Check that we've only invoked the compiler once.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 1);
  }

  fn create_test_worker() -> Worker {
    let state = ThreadSafeState::mock(vec![
      String::from("./deno"),
      String::from("hello.js"),
    ]);
    let mut worker =
      Worker::new("TEST".to_string(), startup_data::deno_isolate_init(), state);
    err_check(worker.execute("denoMain()"));
    err_check(worker.execute("workerMain()"));
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
            delete window.onmessage;
            return;
          } else {
            console.assert(e.data === "hi");
          }
          postMessage([1, 2, 3]);
          console.log("after postMessage");
        }
        "#;
      err_check(worker.execute(source));

      let resource = worker.state.resource.clone();
      let resource_ = resource.clone();

      tokio::spawn(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          err_check(r);
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
      err_check(
        worker.execute("onmessage = () => { delete window.onmessage; }"),
      );

      let resource = worker.state.resource.clone();
      let rid = resource.rid;

      let worker_future = worker
        .then(move |r| -> Result<(), ()> {
          resource.close();
          println!("workers.rs after resource close");
          err_check(r);
          Ok(())
        }).shared();

      let worker_future_ = worker_future.clone();
      tokio::spawn(lazy(move || worker_future_.then(|_| Ok(()))));

      assert_eq!(resources::get_type(rid), Some("worker".to_string()));

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = resources::post_message_to_worker(rid, msg).wait();
      assert!(r.is_ok());
      debug!("rid {:?}", rid);

      worker_future.wait().unwrap();
      assert_eq!(resources::get_type(rid), None);
    })
  }

  #[test]
  fn execute_mod_resolve_error() {
    tokio_util::init(|| {
      // "foo" is not a valid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_root("does-not-exist").unwrap();
      let result = worker.execute_mod_async(&module_specifier, false).wait();
      assert!(result.is_err());
    })
  }

  #[test]
  fn execute_mod_002_hello() {
    tokio_util::init(|| {
      // This assumes cwd is project root (an assumption made throughout the
      // tests).
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_root("./tests/002_hello.ts").unwrap();
      let result = worker.execute_mod_async(&module_specifier, false).wait();
      assert!(result.is_ok());
    })
  }
}
