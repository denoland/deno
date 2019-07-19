// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::state::ThreadSafeState;
use deno;
use deno::ErrBox;
use deno::ModuleSpecifier;
use deno::StartupData;
use futures::executor::block_on;
use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

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
    let isolate = Arc::new(Mutex::new(deno::Isolate::new(startup_data, false)));
    {
      let mut i = isolate.lock().unwrap();
      let state_ = state.clone();
      i.set_dispatch(move |control_buf, zero_copy_buf| {
        state_.dispatch(control_buf, zero_copy_buf)
      });
      let state_ = state.clone();
      i.set_js_error_create(move |v8_exception| {
        JSError::from_v8_exception(v8_exception, &state_.ts_compiler)
      })
    }
    Self { isolate, state }
  }

  /// Same as execute2() but the filename defaults to "<anonymous>".
  pub fn execute(&mut self, js_source: &str) -> Result<(), ErrBox> {
    self.execute2("<anonymous>", js_source)
  }

  /// Executes the provided JavaScript source code. The js_filename argument is
  /// provided only for debugging purposes.
  pub fn execute2(
    &mut self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    let mut isolate = self.isolate.lock().unwrap();
    isolate.execute(js_filename, js_source)
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod_async(
    &mut self,
    module_specifier: &ModuleSpecifier,
    is_prefetch: bool,
  ) -> impl Future<Output = Result<(), ErrBox>> {
    let worker = self.clone();
    let loader = self.state.clone();
    let isolate = self.isolate.clone();
    let modules = self.state.modules.clone();
    let recursive_load = deno::RecursiveLoad::new(
      &module_specifier.to_string(),
      loader,
      isolate,
      modules,
    );
    recursive_load.and_then(move |id| {
      worker.state.progress.done();
      if is_prefetch {
        future::ok(())
      } else {
        let mut isolate = worker.isolate.lock().unwrap();
        future::ready(isolate.mod_evaluate(id))
      }
    })
  }

  /// Executes the provided JavaScript module.
  pub fn execute_mod(
    &mut self,
    module_specifier: &ModuleSpecifier,
    is_prefetch: bool,
  ) -> Result<(), ErrBox> {
    block_on(self.execute_mod_async(module_specifier, is_prefetch))
  }
}

impl Future for Worker {
  type Output = Result<(), ErrBox>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
    let mut isolate = self.isolate.lock().unwrap();
    isolate.poll_unpin(cx)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
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
      ModuleSpecifier::resolve_url_or_path("tests/esm_imports_a.js").unwrap();
    let argv = vec![String::from("./deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      op_selector_std,
      Progress::new(),
    );
    let state_ = state.clone();
    tokio_util::run(
      lazy(move |_cx| {
        let mut worker =
          Worker::new("TEST".to_string(), StartupData::None, state);
        let result = worker.execute_mod(&module_specifier, false);
        if let Err(err) = result {
          eprintln!("execute_mod err {:?}", err);
        }
        worker
      })
      .then(|worker| tokio_util::panic_on_error(worker).map(|r| Ok(r))),
    );

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_mod_circular() {
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path("tests/circular1.js").unwrap();
    let argv = vec![String::from("./deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      op_selector_std,
      Progress::new(),
    );
    let state_ = state.clone();
    tokio_util::run(
      lazy(move |_cx| {
        let mut worker =
          Worker::new("TEST".to_string(), StartupData::None, state);
        let result = worker.execute_mod(&module_specifier, false);
        if let Err(err) = result {
          eprintln!("execute_mod err {:?}", err);
        }
        worker
      })
      .then(|worker| tokio_util::panic_on_error(worker).map(|r| Ok(r))),
    );

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_006_url_imports() {
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path("tests/006_url_imports.ts").unwrap();
    let argv = vec![String::from("deno"), module_specifier.to_string()];
    let mut flags = flags::DenoFlags::default();
    flags.reload = true;
    let state =
      ThreadSafeState::new(flags, argv, op_selector_std, Progress::new());
    let state_ = state.clone();
    tokio_util::run(
      lazy(move |_cx| {
        let mut worker = Worker::new(
          "TEST".to_string(),
          startup_data::deno_isolate_init(),
          state,
        );
        worker.execute("denoMain()").unwrap();
        let result = worker.execute_mod(&module_specifier, false);
        if let Err(err) = result {
          eprintln!("execute_mod err {:?}", err);
        }
        worker
      })
      .then(|worker| tokio_util::panic_on_error(worker).map(|r| Ok(r))),
    );

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
    worker.execute("denoMain()").unwrap();
    worker.execute("workerMain()").unwrap();
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
      worker.execute(source).unwrap();

      let resource = worker.state.resource.clone();
      let resource_ = resource.clone();

      tokio::spawn(
        worker
          .then(move |r| {
            resource_.close();
            r.unwrap();
            future::ok(())
          })
          .boxed()
          .compat(),
      );

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

      let r = futures::executor::block_on(resources::post_message_to_worker(
        resource.rid,
        msg,
      ));
      assert!(r.is_ok());

      let maybe_msg_result =
        tokio_util::block_on(resources::get_message_from_worker(resource.rid));
      assert!(maybe_msg_result.is_ok());
      let maybe_msg = maybe_msg_result.unwrap();
      assert!(maybe_msg.is_some());
      // Check if message received is [1, 2, 3] in json
      assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = futures::executor::block_on(resources::post_message_to_worker(
        resource.rid,
        msg,
      ));
      assert!(r.is_ok());
    })
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    tokio_util::init(|| {
      let mut worker = create_test_worker();
      worker
        .execute("onmessage = () => { delete window.onmessage; }")
        .unwrap();

      let resource = worker.state.resource.clone();
      let rid = resource.rid;

      let (sender, receiver) = futures::channel::oneshot::channel();

      let worker_future = worker.then(move |r| {
        resource.close();
        println!("workers.rs after resource close");
        r.unwrap();
        sender.send(()).unwrap();
        future::ok(())
      });

      tokio::spawn(worker_future.boxed().compat());

      assert_eq!(resources::get_type(rid), Some("worker".to_string()));

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = futures::executor::block_on(resources::post_message_to_worker(
        rid, msg,
      ));
      assert!(r.is_ok());
      debug!("rid {:?}", rid);

      futures::executor::block_on(receiver).unwrap();
      assert_eq!(resources::get_type(rid), None);
    })
  }

  #[test]
  fn execute_mod_resolve_error() {
    tokio_util::init(|| {
      // "foo" is not a valid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
      let result = futures::executor::block_on(
        worker.execute_mod_async(&module_specifier, false),
      );
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
        ModuleSpecifier::resolve_url_or_path("./tests/002_hello.ts").unwrap();
      let result = futures::executor::block_on(
        worker.execute_mod_async(&module_specifier, false),
      );
      assert!(result.is_ok());
    })
  }
}
