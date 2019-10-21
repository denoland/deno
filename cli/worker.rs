// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::ops;
use crate::state::ThreadSafeState;
use deno;
use deno::ErrBox;
use deno::ModuleSpecifier;
use deno::RecursiveLoad;
use deno::StartupData;
use futures::Async;
use futures::Future;
use std::env;
use std::sync::Arc;
use std::sync::Mutex;
use url::Url;

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

      ops::compiler::init(&mut i, &state);
      ops::errors::init(&mut i, &state);
      ops::fetch::init(&mut i, &state);
      ops::files::init(&mut i, &state);
      ops::fs::init(&mut i, &state);
      ops::io::init(&mut i, &state);
      ops::net::init(&mut i, &state);
      ops::tls::init(&mut i, &state);
      ops::os::init(&mut i, &state);
      ops::permissions::init(&mut i, &state);
      ops::process::init(&mut i, &state);
      ops::random::init(&mut i, &state);
      ops::repl::init(&mut i, &state);
      ops::resources::init(&mut i, &state);
      ops::timers::init(&mut i, &state);
      ops::workers::init(&mut i, &state);

      let state_ = state.clone();
      i.set_dyn_import(move |id, specifier, referrer| {
        let load_stream = RecursiveLoad::dynamic_import(
          id,
          specifier,
          referrer,
          state_.clone(),
          state_.modules.clone(),
        );
        Box::new(load_stream)
      });

      let state_ = state.clone();
      i.set_js_error_create(move |v8_exception| {
        JSError::from_v8_exception(v8_exception, &state_.ts_compiler)
      })
    }
    Self { isolate, state }
  }

  /// Same as execute2() but the filename defaults to "$CWD/__anonymous__".
  pub fn execute(&mut self, js_source: &str) -> Result<(), ErrBox> {
    let path = env::current_dir().unwrap().join("__anonymous__");
    let url = Url::from_file_path(path).unwrap();
    self.execute2(url.as_str(), js_source)
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
    maybe_code: Option<String>,
    is_prefetch: bool,
  ) -> impl Future<Item = (), Error = ErrBox> {
    let worker = self.clone();
    let loader = self.state.clone();
    let isolate = self.isolate.clone();
    let modules = self.state.modules.clone();
    let recursive_load = RecursiveLoad::main(
      &module_specifier.to_string(),
      maybe_code,
      loader,
      modules,
    )
    .get_future(isolate);
    recursive_load.and_then(move |id| -> Result<(), ErrBox> {
      worker.state.progress.done();
      if is_prefetch {
        Ok(())
      } else {
        let mut isolate = worker.isolate.lock().unwrap();
        isolate.mod_evaluate(id)
      }
    })
  }
}

impl Future for Worker {
  type Item = ();
  type Error = ErrBox;

  fn poll(&mut self) -> Result<Async<()>, ErrBox> {
    let mut isolate = self.isolate.lock().unwrap();
    isolate.poll()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags;
  use crate::progress::Progress;
  use crate::resources;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use futures::future::lazy;
  use std::sync::atomic::Ordering;

  #[test]
  fn execute_mod_esm_imports_a() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/esm_imports_a.js")
      .to_owned();
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let argv = vec![String::from("./deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      Progress::new(),
      true,
    )
    .unwrap();
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      worker
        .execute_mod_async(&module_specifier, None, false)
        .then(|result| {
          if let Err(err) = result {
            eprintln!("execute_mod err {:?}", err);
          }
          tokio_util::panic_on_error(worker)
        })
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.ts")
      .to_owned();
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let argv = vec![String::from("deno"), module_specifier.to_string()];
    let state = ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      Progress::new(),
      true,
    )
    .unwrap();
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker =
        Worker::new("TEST".to_string(), StartupData::None, state);
      worker
        .execute_mod_async(&module_specifier, None, false)
        .then(|result| {
          if let Err(err) = result {
            eprintln!("execute_mod err {:?}", err);
          }
          tokio_util::panic_on_error(worker)
        })
    }));

    let metrics = &state_.metrics;
    // TODO  assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
    // Check that we didn't start the compiler.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 0);
  }

  #[test]
  fn execute_006_url_imports() {
    let http_server_guard = crate::test_util::http_server();

    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("cli/tests/006_url_imports.ts")
      .to_owned();
    let module_specifier =
      ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
    let argv = vec![String::from("deno"), module_specifier.to_string()];
    let mut flags = flags::DenoFlags::default();
    flags.reload = true;
    let state =
      ThreadSafeState::new(flags, argv, Progress::new(), true).unwrap();
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let mut worker = Worker::new(
        "TEST".to_string(),
        startup_data::deno_isolate_init(),
        state,
      );
      worker.execute("denoMain()").unwrap();
      worker
        .execute_mod_async(&module_specifier, None, false)
        .then(|result| {
          if let Err(err) = result {
            eprintln!("execute_mod err {:?}", err);
          }
          tokio_util::panic_on_error(worker)
        })
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 3);
    // Check that we've only invoked the compiler once.
    assert_eq!(metrics.compiler_starts.load(Ordering::SeqCst), 1);
    drop(http_server_guard);
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
    tokio_util::run_in_task(|| {
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

      tokio::spawn(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          r.unwrap();
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
    tokio_util::run_in_task(|| {
      let mut worker = create_test_worker();
      worker
        .execute("onmessage = () => { delete window.onmessage; }")
        .unwrap();

      let resource = worker.state.resource.clone();
      let rid = resource.rid;

      let worker_future = worker
        .then(move |r| -> Result<(), ()> {
          resource.close();
          println!("workers.rs after resource close");
          r.unwrap();
          Ok(())
        })
        .shared();

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
    tokio_util::run_in_task(|| {
      // "foo" is not a valid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
      let result = worker
        .execute_mod_async(&module_specifier, None, false)
        .wait();
      assert!(result.is_err());
    })
  }

  #[test]
  fn execute_mod_002_hello() {
    tokio_util::run_in_task(|| {
      // This assumes cwd is project root (an assumption made throughout the
      // tests).
      let mut worker = create_test_worker();
      let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/002_hello.ts")
        .to_owned();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      let result = worker
        .execute_mod_async(&module_specifier, None, false)
        .wait();
      assert!(result.is_ok());
    })
  }
}
