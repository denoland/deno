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
use deno::JSError;
use deno::Loader;
use deno::StartupData;
use futures::future::Either;
use futures::Async;
use futures::Future;
use std::sync::atomic::Ordering;
use url::Url;

/// Wraps deno::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading
pub struct Worker {
  inner: deno::Isolate<ThreadSafeState>,
  pub modules: deno::Modules,
  pub state: ThreadSafeState,
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
      modules: deno::Modules::new(),
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

  /// Consumes worker. Executes the provided JavaScript module.
  pub fn execute_mod_async(
    self,
    js_url: &Url,
    is_prefetch: bool,
  ) -> impl Future<Item = Self, Error = (RustOrJsError, Self)> {
    let recursive_load = deno::RecursiveLoad::new(js_url.as_str(), self);
    recursive_load.and_then(
      move |(id, mut self_)| -> Result<Self, (deno::JSErrorOr<DenoError>, Self)> {
        if is_prefetch {
          Ok(self_)
        } else {
          let result = self_.inner.mod_evaluate(id);
          if let Err(err) = result {
            Err((deno::JSErrorOr::JSError(err), self_))
          } else {
            Ok(self_)
          }
        }
      },
    )
    .map_err(|(err, self_)| {
      // Convert to RustOrJsError AND apply_source_map.
      let err = match err {
        deno::JSErrorOr::JSError(err) => RustOrJsError::Js(self_.apply_source_map(err)),
        deno::JSErrorOr::Other(err) => RustOrJsError::Rust(err),
      };
      (err, self_)
    })
  }

  /// Consumes worker. Executes the provided JavaScript module.
  pub fn execute_mod(
    self,
    js_url: &Url,
    is_prefetch: bool,
  ) -> Result<Self, (RustOrJsError, Self)> {
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

impl Loader for Worker {
  type Dispatch = ThreadSafeState;
  type Error = DenoError;

  fn resolve(specifier: &str, referrer: &str) -> Result<String, Self::Error> {
    resolve_module_spec(specifier, referrer).map_err(DenoError::from)
  }

  /// Given an absolute url, load its source code.
  fn load(
    &mut self,
    url: &str,
  ) -> Box<deno::SourceCodeInfoFuture<Self::Error>> {
    self
      .state
      .metrics
      .resolve_count
      .fetch_add(1, Ordering::SeqCst);
    Box::new(
      fetch_module_meta_data_and_maybe_compile_async(&self.state, url, ".")
        .map_err(|err| {
          eprintln!("{}", err);
          err
        }).map(|module_meta_data| deno::SourceCodeInfo {
          // Real module name, might be different from initial URL
          // due to redirections.
          code: module_meta_data.js_source(),
          module_name: module_meta_data.module_name,
        }),
    )
  }

  fn isolate_and_modules<'a: 'b + 'c, 'b, 'c>(
    &'a mut self,
  ) -> (&'b mut deno::Isolate<Self::Dispatch>, &'c mut deno::Modules) {
    (&mut self.inner, &mut self.modules)
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

pub fn fetch_module_meta_data_and_maybe_compile(
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

  #[test]
  fn execute_mod_esm_imports_a() {
    let filename = std::env::current_dir()
      .unwrap()
      .join("tests/esm_imports_a.js");
    let js_url = Url::from_file_path(filename).unwrap();

    let argv = vec![String::from("./deno"), js_url.to_string()];
    let state =
      ThreadSafeState::new(flags::DenoFlags::default(), argv, op_selector_std);
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let worker = Worker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_mod(&js_url, false);
      let worker = match result {
        Err((err, worker)) => {
          eprintln!("execute_mod err {:?}", err);
          worker
        }
        Ok(worker) => worker,
      };
      tokio_util::panic_on_error(worker)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }

  #[test]
  fn execute_mod_circular() {
    let filename = std::env::current_dir().unwrap().join("tests/circular1.js");
    let js_url = Url::from_file_path(filename).unwrap();

    let argv = vec![String::from("./deno"), js_url.to_string()];
    let state =
      ThreadSafeState::new(flags::DenoFlags::default(), argv, op_selector_std);
    let state_ = state.clone();
    tokio_util::run(lazy(move || {
      let worker = Worker::new("TEST".to_string(), StartupData::None, state);
      let result = worker.execute_mod(&js_url, false);
      let worker = match result {
        Err((err, worker)) => {
          eprintln!("execute_mod err {:?}", err);
          worker
        }
        Ok(worker) => worker,
      };
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
    // "foo" is not a vailid module specifier so this should return an error.
    let worker = create_test_worker();
    let js_url = root_specifier_to_url("does-not-exist").unwrap();
    let result = worker.execute_mod_async(&js_url, false).wait();
    assert!(result.is_err());
  }

  #[test]
  fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let worker = create_test_worker();
    let js_url = root_specifier_to_url("./tests/002_hello.ts").unwrap();
    let result = worker.execute_mod_async(&js_url, false).wait();
    assert!(result.is_ok());
  }
}
