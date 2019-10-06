// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::fmt_errors::JSError;
use crate::ops::json_op;
use crate::ops::minimal_op;
use crate::ops::*;
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
      let state_ = state.clone();

      i.register_op("read", state_.cli_op(minimal_op(io::op_read)));
      i.register_op("write", state_.cli_op(minimal_op(io::op_write)));

      i.register_op(
        "exit",
        state_.cli_op(json_op(state_.stateful_op(os::op_exit))),
      );
      i.register_op(
        "is_tty",
        state_.cli_op(json_op(state_.stateful_op(os::op_is_tty))),
      );
      i.register_op(
        "env",
        state_.cli_op(json_op(state_.stateful_op(os::op_env))),
      );
      i.register_op(
        "exec_path",
        state_.cli_op(json_op(state_.stateful_op(os::op_exec_path))),
      );
      i.register_op(
        "utime",
        state_.cli_op(json_op(state_.stateful_op(fs::op_utime))),
      );
      i.register_op(
        "set_env",
        state_.cli_op(json_op(state_.stateful_op(os::op_set_env))),
      );
      i.register_op(
        "get_env",
        state_.cli_op(json_op(state_.stateful_op(os::op_get_env))),
      );
      i.register_op(
        "home_dir",
        state_.cli_op(json_op(state_.stateful_op(os::op_home_dir))),
      );
      i.register_op(
        "start",
        state_.cli_op(json_op(state_.stateful_op(os::op_start))),
      );
      i.register_op(
        "apply_source_map",
        state_.cli_op(json_op(state_.stateful_op(errors::op_apply_source_map))),
      );
      i.register_op(
        "format_error",
        state_.cli_op(json_op(state_.stateful_op(errors::op_format_error))),
      );
      i.register_op(
        "cache",
        state_.cli_op(json_op(state_.stateful_op(compiler::op_cache))),
      );
      i.register_op(
        "fetch_source_files",
        state_
          .cli_op(json_op(state_.stateful_op(compiler::op_fetch_source_files))),
      );
      i.register_op(
        "open",
        state_.cli_op(json_op(state_.stateful_op(files::op_open))),
      );
      i.register_op(
        "close",
        state_.cli_op(json_op(state_.stateful_op(files::op_close))),
      );
      i.register_op(
        "seek",
        state_.cli_op(json_op(state_.stateful_op(files::op_seek))),
      );
      i.register_op(
        "fetch",
        state_.cli_op(json_op(state_.stateful_op(fetch::op_fetch))),
      );
      i.register_op(
        "metrics",
        state_.cli_op(json_op(state_.stateful_op(metrics::op_metrics))),
      );
      i.register_op(
        "repl_start",
        state_.cli_op(json_op(state_.stateful_op(repl::op_repl_start))),
      );
      i.register_op(
        "repl_readline",
        state_.cli_op(json_op(state_.stateful_op(repl::op_repl_readline))),
      );
      i.register_op(
        "accept",
        state_.cli_op(json_op(state_.stateful_op(net::op_accept))),
      );
      i.register_op(
        "dial",
        state_.cli_op(json_op(state_.stateful_op(net::op_dial))),
      );
      i.register_op(
        "dial_tls",
        state_.cli_op(json_op(state_.stateful_op(net::op_dial))),
      );
      i.register_op(
        "shutdown",
        state_.cli_op(json_op(state_.stateful_op(net::op_shutdown))),
      );
      i.register_op(
        "listen",
        state_.cli_op(json_op(state_.stateful_op(net::op_listen))),
      );
      i.register_op(
        "resources",
        state_.cli_op(json_op(state_.stateful_op(resources::op_resources))),
      );
      i.register_op(
        "get_random_values",
        state_
          .cli_op(json_op(state_.stateful_op(random::op_get_random_values))),
      );
      i.register_op(
        "global_timer_stop",
        state_
          .cli_op(json_op(state_.stateful_op(timers::op_global_timer_stop))),
      );
      i.register_op(
        "global_timer",
        state_.cli_op(json_op(state_.stateful_op(timers::op_global_timer))),
      );
      i.register_op(
        "now",
        state_.cli_op(json_op(state_.stateful_op(performance::op_now))),
      );
      i.register_op(
        "permissions",
        state_.cli_op(json_op(state_.stateful_op(permissions::op_permissions))),
      );
      i.register_op(
        "revoke_permission",
        state_.cli_op(json_op(
          state_.stateful_op(permissions::op_revoke_permission),
        )),
      );
      i.register_op(
        "create_worker",
        state_.cli_op(json_op(state_.stateful_op(workers::op_create_worker))),
      );
      i.register_op(
        "host_get_worker_closed",
        state_.cli_op(json_op(
          state_.stateful_op(workers::op_host_get_worker_closed),
        )),
      );
      i.register_op(
        "host_post_message",
        state_
          .cli_op(json_op(state_.stateful_op(workers::op_host_post_message))),
      );
      i.register_op(
        "host_get_message",
        state_
          .cli_op(json_op(state_.stateful_op(workers::op_host_get_message))),
      );
      // TODO: make sure these two ops are only accessible to appropriate Worker
      i.register_op(
        "worker_post_message",
        state_
          .cli_op(json_op(state_.stateful_op(workers::op_worker_post_message))),
      );
      i.register_op(
        "worker_get_message",
        state_
          .cli_op(json_op(state_.stateful_op(workers::op_worker_get_message))),
      );
      i.register_op(
        "run",
        state_.cli_op(json_op(state_.stateful_op(process::op_run))),
      );
      i.register_op(
        "run_status",
        state_.cli_op(json_op(state_.stateful_op(process::op_run_status))),
      );
      i.register_op(
        "kill",
        state_.cli_op(json_op(state_.stateful_op(process::op_kill))),
      );
      i.register_op(
        "chdir",
        state_.cli_op(json_op(state_.stateful_op(fs::op_chdir))),
      );
      i.register_op(
        "mkdir",
        state_.cli_op(json_op(state_.stateful_op(fs::op_mkdir))),
      );
      i.register_op(
        "chmod",
        state_.cli_op(json_op(state_.stateful_op(fs::op_chmod))),
      );
      i.register_op(
        "chown",
        state_.cli_op(json_op(state_.stateful_op(fs::op_chown))),
      );
      i.register_op(
        "remove",
        state_.cli_op(json_op(state_.stateful_op(fs::op_remove))),
      );
      i.register_op(
        "copy_file",
        state_.cli_op(json_op(state_.stateful_op(fs::op_copy_file))),
      );
      i.register_op(
        "stat",
        state_.cli_op(json_op(state_.stateful_op(fs::op_stat))),
      );
      i.register_op(
        "read_dir",
        state_.cli_op(json_op(state_.stateful_op(fs::op_read_dir))),
      );
      i.register_op(
        "rename",
        state_.cli_op(json_op(state_.stateful_op(fs::op_rename))),
      );
      i.register_op(
        "link",
        state_.cli_op(json_op(state_.stateful_op(fs::op_link))),
      );
      i.register_op(
        "symlink",
        state_.cli_op(json_op(state_.stateful_op(fs::op_symlink))),
      );
      i.register_op(
        "read_link",
        state_.cli_op(json_op(state_.stateful_op(fs::op_read_link))),
      );
      i.register_op(
        "truncate",
        state_.cli_op(json_op(state_.stateful_op(fs::op_truncate))),
      );
      i.register_op(
        "make_temp_dir",
        state_.cli_op(json_op(state_.stateful_op(fs::op_make_temp_dir))),
      );
      i.register_op(
        "cwd",
        state_.cli_op(json_op(state_.stateful_op(fs::op_cwd))),
      );
      i.register_op(
        "fetch_asset",
        state_.cli_op(json_op(state_.stateful_op(compiler::op_fetch_asset))),
      );
      i.register_op(
        "hostname",
        state_.cli_op(json_op(state_.stateful_op(os::op_hostname))),
      );

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
    is_prefetch: bool,
  ) -> impl Future<Item = (), Error = ErrBox> {
    let worker = self.clone();
    let loader = self.state.clone();
    let isolate = self.isolate.clone();
    let modules = self.state.modules.clone();
    let recursive_load =
      RecursiveLoad::main(&module_specifier.to_string(), loader, modules)
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
        .execute_mod_async(&module_specifier, false)
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
        .execute_mod_async(&module_specifier, false)
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
        .execute_mod_async(&module_specifier, false)
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
    tokio_util::init(|| {
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
    tokio_util::init(|| {
      // "foo" is not a valid module specifier so this should return an error.
      let mut worker = create_test_worker();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path("does-not-exist").unwrap();
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
      let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/002_hello.ts")
        .to_owned();
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&p.to_string_lossy()).unwrap();
      let result = worker.execute_mod_async(&module_specifier, false).wait();
      assert!(result.is_ok());
    })
  }
}
