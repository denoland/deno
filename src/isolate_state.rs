// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

#![allow(dead_code)]

use crate::cli::Buf;
use crate::cli::Isolate;
use crate::compiler::compile_sync;
use crate::compiler::ModuleMetaData;
use crate::deno_dir;
use crate::errors::DenoError;
use crate::errors::RustOrJsError;
use crate::flags;
use crate::global_timer::GlobalTimer;
use crate::modules::Modules;
use crate::msg;
use deno_core::deno_mod;
use deno_core::JSError;
use futures::sync::mpsc as async_mpsc;
use std;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub type WorkerSender = async_mpsc::Sender<Buf>;
pub type WorkerReceiver = async_mpsc::Receiver<Buf>;
pub type WorkerChannels = (WorkerSender, WorkerReceiver);

// AtomicU64 is currently unstable
#[derive(Default)]
pub struct Metrics {
  pub ops_dispatched: AtomicUsize,
  pub ops_completed: AtomicUsize,
  pub bytes_sent_control: AtomicUsize,
  pub bytes_sent_data: AtomicUsize,
  pub bytes_received: AtomicUsize,
  pub resolve_count: AtomicUsize,
}

// Isolate cannot be passed between threads but IsolateState can.
// IsolateState satisfies Send and Sync.
// So any state that needs to be accessed outside the main V8 thread should be
// inside IsolateState.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct IsolateState {
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub flags: flags::DenoFlags,
  pub metrics: Metrics,
  pub modules: Mutex<Modules>,
  pub worker_channels: Option<Mutex<WorkerChannels>>,
  pub global_timer: Mutex<GlobalTimer>,
}

impl IsolateState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    worker_channels: Option<WorkerChannels>,
  ) -> Self {
    let custom_root = env::var("DENO_DIR").map(|s| s.into()).ok();

    Self {
      dir: deno_dir::DenoDir::new(flags.reload, flags.recompile, custom_root)
        .unwrap(),
      argv: argv_rest,
      flags,
      metrics: Metrics::default(),
      modules: Mutex::new(Modules::new()),
      worker_channels: worker_channels.map(Mutex::new),
      global_timer: Mutex::new(GlobalTimer::new()),
    }
  }

  pub fn main_module(&self) -> Option<String> {
    if self.argv.len() <= 1 {
      None
    } else {
      let specifier = self.argv[1].clone();
      let referrer = ".";
      match self.dir.resolve_module_url(&specifier, referrer) {
        Ok(url) => Some(url.to_string()),
        Err(e) => {
          debug!("Potentially swallowed error {}", e);
          None
        }
      }
    }
  }

  fn fetch_module_meta_data_and_maybe_compile(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<ModuleMetaData, DenoError> {
    let mut out = self.dir.fetch_module_meta_data(specifier, referrer)?;
    if (out.media_type == msg::MediaType::TypeScript
      && out.maybe_output_code.is_none())
      || self.flags.recompile
    {
      debug!(">>>>> compile_sync START");
      out = compile_sync(self, specifier, &referrer, &out);
      debug!(">>>>> compile_sync END");
      self.dir.code_cache(&out)?;
    }
    Ok(out)
  }

  // TODO(ry) make this return a future.
  fn mod_load_deps(
    &self,
    isolate: &Isolate,
    id: deno_mod,
  ) -> Result<(), RustOrJsError> {
    // basically iterate over the imports, start loading them.

    let referrer_name = {
      let g = self.modules.lock().unwrap();
      g.get_name(id).unwrap().clone()
    };

    for specifier in isolate.mod_get_imports(id) {
      let (name, _local_filename) = self
        .dir
        .resolve_module(&specifier, &referrer_name)
        .map_err(DenoError::from)
        .map_err(RustOrJsError::from)?;

      debug!("mod_load_deps {}", name);

      if !self.modules.lock().unwrap().is_registered(&name) {
        let out = self.fetch_module_meta_data_and_maybe_compile(
          &specifier,
          &referrer_name,
        )?;
        let child_id = self.mod_new_and_regsiter(
          isolate,
          false,
          &out.module_name.clone(),
          &out.js_source(),
        )?;

        self.mod_load_deps(isolate, child_id)?;
      }
    }

    Ok(())
  }

  /// High-level way to execute modules.
  /// This will issue HTTP requests and file system calls.
  /// Blocks. TODO(ry) Don't block.
  pub fn mod_execute(
    &self,
    isolate: &Isolate,
    url: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    let out = self
      .fetch_module_meta_data_and_maybe_compile(url, ".")
      .map_err(RustOrJsError::from)?;

    let id = self
      .mod_new_and_regsiter(
        isolate,
        true,
        &out.module_name.clone(),
        &out.js_source(),
      ).map_err(RustOrJsError::from)?;

    self.mod_load_deps(isolate, id)?;

    isolate.mod_instantiate(id).map_err(RustOrJsError::from)?;
    if !is_prefetch {
      isolate.mod_evaluate(id).map_err(RustOrJsError::from)?;
    }
    Ok(())
  }

  /// Wraps Isolate::mod_new but registers with modules.
  fn mod_new_and_regsiter(
    &self,
    isolate: &Isolate,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<deno_mod, JSError> {
    let id = isolate.mod_new(main, name, source)?;
    self.modules.lock().unwrap().register(id, &name);
    Ok(id)
  }

  #[cfg(test)]
  pub fn mock() -> IsolateState {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    // For debugging: argv.push_back(String::from("-D"));
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();
    IsolateState::new(flags, rest_argv, None)
  }

  fn metrics_op_dispatched(
    &self,
    bytes_sent_control: usize,
    bytes_sent_data: usize,
  ) {
    self.metrics.ops_dispatched.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_control
      .fetch_add(bytes_sent_control, Ordering::SeqCst);
    self
      .metrics
      .bytes_sent_data
      .fetch_add(bytes_sent_data, Ordering::SeqCst);
  }

  fn metrics_op_completed(&self, bytes_received: usize) {
    self.metrics.ops_completed.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_received
      .fetch_add(bytes_received, Ordering::SeqCst);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cli::Cli;
  use crate::isolate_init::IsolateInit;
  use crate::permissions::DenoPermissions;
  use crate::tokio_util::panic_on_error;
  use futures::future::lazy;
  use std::sync::Arc;

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
    let init = IsolateInit {
      snapshot: None,
      init_script: None,
    };
    let cli = Cli::new(init, state.clone(), DenoPermissions::default());
    let isolate = Isolate::new(cli);
    tokio::runtime::current_thread::run(lazy(move || {
      state.mod_execute(&isolate, &filename, false).ok();
      panic_on_error(isolate)
    }));

    let metrics = &state_.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 1);
  }

  /*

	TODO(ry) uncomment this before landing

  #[test]
  fn execute_mod_circular() {
    let filename = std::env::current_dir().unwrap().join("tests/circular1.js");
    let filename = filename.to_str().unwrap();

    let argv = vec![String::from("./deno"), String::from(filename)];
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();

    let state = Arc::new(IsolateState::new(flags, rest_argv, None));
    let init = IsolateInit {
      snapshot: None,
      init_script: None,
    };
    let mut isolate =
      Isolate::new(init, state, dispatch_sync, DenoPermissions::default());
    tokio_util::init(|| {
      isolate
        .execute_mod(filename, false)
        .expect("execute_mod error");
      isolate.event_loop().ok();
    });

    let metrics = &isolate.state.metrics;
    assert_eq!(metrics.resolve_count.load(Ordering::SeqCst), 2);
  }
	*/
}
