// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::bad_resource;
use crate::fmt_errors::JSError;
use crate::ops;
use crate::resources;
use crate::resources::CoreResource;
use crate::resources::ResourceId;
use crate::state::ThreadSafeState;
use deno;
use deno::Buf;
use deno::ErrBox;
use deno::ModuleSpecifier;
use deno::RecursiveLoad;
use deno::StartupData;
use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Sink;
use futures::Stream;
use std::env;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use url::Url;

/// Wraps mpsc channels into a generic resource so they can be referenced
/// from ops and used to facilitate parent-child communication
/// for workers.
pub struct WorkerChannels {
  pub sender: mpsc::Sender<Buf>,
  pub receiver: mpsc::Receiver<Buf>,
}

impl CoreResource for WorkerChannels {
  fn inspect_repr(&self) -> &str {
    "worker"
  }
}

/// Wraps deno::Isolate to provide source maps, ops for the CLI, and
/// high-level module loading.
#[derive(Clone)]
pub struct Worker {
  pub name: String,
  isolate: Arc<Mutex<deno::Isolate>>,
  pub state: ThreadSafeState,
}

impl Worker {
  pub fn new(
    name: String,
    startup_data: StartupData,
    state: ThreadSafeState,
  ) -> Self {
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

      let global_state_ = state.global_state.clone();
      i.set_js_error_create(move |v8_exception| {
        JSError::from_v8_exception(v8_exception, &global_state_.ts_compiler)
      })
    }
    Self {
      name,
      isolate,
      state,
    }
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
      worker.state.global_state.progress.done();
      if is_prefetch {
        Ok(())
      } else {
        let mut isolate = worker.isolate.lock().unwrap();
        isolate.mod_evaluate(id)
      }
    })
  }

  /// Post message to worker as a host or privileged overlord
  pub fn post_message(self: &Self, buf: Buf) -> Result<Async<()>, ErrBox> {
    Worker::post_message_to_resource(self.state.rid, buf)
  }

  pub fn post_message_to_resource(
    rid: resources::ResourceId,
    buf: Buf,
  ) -> Result<Async<()>, ErrBox> {
    debug!("post message to resource {}", rid);
    let mut table = resources::lock_resource_table();
    let worker = table
      .get_mut::<WorkerChannels>(rid)
      .ok_or_else(bad_resource)?;
    let sender = &mut worker.sender;
    sender
      .send(buf)
      .poll()
      .map(|_| Async::Ready(()))
      .map_err(ErrBox::from)
  }

  pub fn get_message(self: &Self) -> WorkerReceiver {
    Worker::get_message_from_resource(self.state.rid)
  }

  pub fn get_message_from_resource(rid: ResourceId) -> WorkerReceiver {
    debug!("get message from resource {}", rid);
    WorkerReceiver { rid }
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

/// This structure wraps worker's resource id to implement future
/// that will return message received from worker or None
/// if worker's channel has been closed.
pub struct WorkerReceiver {
  rid: ResourceId,
}

impl Future for WorkerReceiver {
  type Item = Option<Buf>;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Option<Buf>, ErrBox> {
    let mut table = resources::lock_resource_table();
    let worker = table
      .get_mut::<WorkerChannels>(self.rid)
      .ok_or_else(bad_resource)?;
    let receiver = &mut worker.receiver;
    receiver.poll().map_err(ErrBox::from)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::flags;
  use crate::flags::DenoFlags;
  use crate::global_state::ThreadSafeGlobalState;
  use crate::progress::Progress;
  use crate::startup_data;
  use crate::state::ThreadSafeState;
  use crate::tokio_util;
  use futures::future::lazy;
  use futures::IntoFuture;
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
    let global_state = ThreadSafeGlobalState::new(
      flags::DenoFlags::default(),
      argv,
      Progress::new(),
    )
    .unwrap();
    let state =
      ThreadSafeState::new(global_state, Some(module_specifier.clone()), true)
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
    let global_state =
      ThreadSafeGlobalState::new(DenoFlags::default(), argv, Progress::new())
        .unwrap();
    let state =
      ThreadSafeState::new(global_state, Some(module_specifier.clone()), true)
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
    let global_state =
      ThreadSafeGlobalState::new(flags, argv, Progress::new()).unwrap();
    let state = ThreadSafeState::new(
      global_state.clone(),
      Some(module_specifier.clone()),
      true,
    )
    .unwrap();
    let global_state_ = global_state.clone();
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

    assert_eq!(state_.metrics.resolve_count.load(Ordering::SeqCst), 3);
    // Check that we've only invoked the compiler once.
    assert_eq!(
      global_state_.metrics.compiler_starts.load(Ordering::SeqCst),
      1
    );
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

      let worker_ = worker.clone();
      let rid = worker.state.rid;
      let resource_ = resources::Resource { rid };

      tokio::spawn(lazy(move || {
        worker.then(move |r| -> Result<(), ()> {
          resource_.close();
          r.unwrap();
          Ok(())
        })
      }));

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

      let r = worker_.post_message(msg).into_future().wait();
      assert!(r.is_ok());

      let maybe_msg = worker_.get_message().wait().unwrap();
      assert!(maybe_msg.is_some());
      // Check if message received is [1, 2, 3] in json
      assert_eq!(*maybe_msg.unwrap(), *b"[1,2,3]");

      let msg = json!("exit")
        .to_string()
        .into_boxed_str()
        .into_boxed_bytes();
      let r = worker_.post_message(msg).into_future().wait();
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

      let rid = worker.state.rid;
      let resource = resources::Resource { rid };
      let worker_ = worker.clone();

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

      let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
      let r = worker_.post_message(msg).into_future().wait();
      assert!(r.is_ok());
      debug!("rid {:?}", rid);

      worker_future.wait().unwrap();
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
