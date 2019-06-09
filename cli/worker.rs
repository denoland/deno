// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::DenoError;
use crate::errors::RustOrJsError;
use crate::js_errors;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno;
use deno::Config;
use deno::JSError;
use deno::StartupData;
use futures::Async;
use futures::Future;
use std::sync::Arc;
use std::sync::Mutex;
use url::Url;

/// Wraps deno::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
#[derive(Clone)]
pub struct Worker {
  inner: Arc<Mutex<deno::Isolate>>,
  pub modules: Arc<Mutex<deno::Modules>>,
  pub state: ThreadSafeState,
}

impl Worker {
  pub fn new(
    _name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
  ) -> Worker {
    let state_ = state.clone();
    let mut config = Config::default();
    config.dispatch(move |control_buf, zero_copy_buf| {
      state_.dispatch(control_buf, zero_copy_buf)
    });
    Self {
      inner: Arc::new(Mutex::new(deno::Isolate::new(startup_data, config))),
      modules: Arc::new(Mutex::new(deno::Modules::new())),
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
    let mut isolate = self.inner.lock().unwrap();
    isolate.execute(js_filename, js_source)
  }

  /// Consumes worker. Executes the provided JavaScript module.
  pub fn execute_mod_async(
    &mut self,
    js_url: &Url,
    is_prefetch: bool,
  ) -> impl Future<Item = (), Error = RustOrJsError> {
    let worker = self.clone();
    let worker_ = worker.clone();
    let loader = self.state.clone();
    let isolate = self.inner.clone();
    let modules = self.modules.clone();
    let recursive_load =
      deno::RecursiveLoad::new(js_url.as_str(), loader, isolate, modules);
    recursive_load
      .and_then(move |id| -> Result<(), deno::JSErrorOr<DenoError>> {
        worker.state.progress.done();
        if is_prefetch {
          Ok(())
        } else {
          let mut isolate = worker.inner.lock().unwrap();
          let result = isolate.mod_evaluate(id);
          if let Err(err) = result {
            Err(deno::JSErrorOr::JSError(err))
          } else {
            Ok(())
          }
        }
      }).map_err(move |err| {
        worker_.state.progress.done();
        // Convert to RustOrJsError AND apply_source_map.
        match err {
          deno::JSErrorOr::JSError(err) => {
            RustOrJsError::Js(worker_.apply_source_map(err))
          }
          deno::JSErrorOr::Other(err) => RustOrJsError::Rust(err),
        }
      })
  }

  /// Consumes worker. Executes the provided JavaScript module.
  pub fn execute_mod(
    &mut self,
    js_url: &Url,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    tokio_util::block_on(self.execute_mod_async(js_url, is_prefetch))
  }

  /// Applies source map to the error.
  fn apply_source_map(&self, err: JSError) -> JSError {
    js_errors::apply_source_map(&err, &self.state.dir)
  }
}

// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
// TODO(ry) Add tests.
// TODO(ry) Move this to core?
pub fn resolve_module_spec(
  specifier: &str,
  base: &str,
) -> Result<String, url::ParseError> {
  // 1. Apply the URL parser to specifier. If the result is not failure, return
  //    the result.
  // let specifier = parse_local_or_remote(specifier)?.to_string();
  if let Ok(specifier_url) = Url::parse(specifier) {
    return Ok(specifier_url.to_string());
  }

  // 2. If specifier does not start with the character U+002F SOLIDUS (/), the
  //    two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./), or the
  //    three-character sequence U+002E FULL STOP, U+002E FULL STOP, U+002F
  //    SOLIDUS (../), return failure.
  if !specifier.starts_with('/')
    && !specifier.starts_with("./")
    && !specifier.starts_with("../")
  {
    // TODO(ry) This is (probably) not the correct error to return here.
    return Err(url::ParseError::RelativeUrlWithCannotBeABaseBase);
  }

  // 3. Return the result of applying the URL parser to specifier with base URL
  //    as the base URL.
  let base_url = Url::parse(base)?;
  let u = base_url.join(&specifier)?;
  Ok(u.to_string())
}

/// Takes a string representing a path or URL to a module, but of the type
/// passed through the command-line interface for the main module. This is
/// slightly different than specifiers used in import statements: "foo.js" for
/// example is allowed here, whereas in import statements a leading "./" is
/// required ("./foo.js"). This function is aware of the current working
/// directory and returns an absolute URL.
pub fn root_specifier_to_url(
  root_specifier: &str,
) -> Result<Url, url::ParseError> {
  let maybe_url = Url::parse(root_specifier);
  if let Ok(url) = maybe_url {
    Ok(url)
  } else {
    let cwd = std::env::current_dir().unwrap();
    let base = Url::from_directory_path(cwd).unwrap();
    base.join(root_specifier)
  }
}

impl Future for Worker {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Result<Async<()>, Self::Error> {
    let mut isolate = self.inner.lock().unwrap();
    isolate.poll().map_err(|err| self.apply_source_map(err))
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
  use deno::js_check;
  use futures::future::lazy;
  use std::sync::atomic::Ordering;

  #[test]
  fn execute_mod_esm_imports_a() {
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/esm_imports_a.js");
    let js_url = Url::from_file_path(filename).unwrap();

    let argv = vec![String::from("./deno"), js_url.to_string()];
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
      let result = worker.execute_mod(&js_url, false);
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
    let filename = std::env::current_dir().unwrap().join("tests/circular1.js");
    let js_url = Url::from_file_path(filename).unwrap();

    let argv = vec![String::from("./deno"), js_url.to_string()];
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
      let result = worker.execute_mod(&js_url, false);
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
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/006_url_imports.ts");
    let js_url = Url::from_file_path(filename).unwrap();
    let argv = vec![String::from("deno"), js_url.to_string()];
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
      js_check(worker.execute("denoMain()"));
      let result = worker.execute_mod(&js_url, false);
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
            delete window.onmessage;
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
        worker.execute("onmessage = () => { delete window.onmessage; }"),
      );

      let resource = worker.state.resource.clone();
      let rid = resource.rid;

      let worker_future = worker
        .then(move |r| -> Result<(), ()> {
          resource.close();
          println!("workers.rs after resource close");
          js_check(r);
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
      // "foo" is not a vailid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let js_url = root_specifier_to_url("does-not-exist").unwrap();
      let result = worker.execute_mod_async(&js_url, false).wait();
      assert!(result.is_err());
    })
  }

  #[test]
  fn execute_mod_002_hello() {
    tokio_util::init(|| {
      // This assumes cwd is project root (an assumption made throughout the
      // tests).
      let mut worker = create_test_worker();
      let js_url = root_specifier_to_url("./tests/002_hello.ts").unwrap();
      let result = worker.execute_mod_async(&js_url, false).wait();
      assert!(result.is_ok());
    })
  }
}
