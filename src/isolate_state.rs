use crate::compiler::compile_sync;
use crate::compiler::ModuleMetaData;
use crate::deno_dir;
use crate::errors::DenoError;
use crate::errors::DenoResult;
use crate::errors::RustOrJsError;
use crate::flags;
use crate::isolate_init::IsolateInit;
use crate::js_errors::apply_source_map;
use crate::libdeno;
use crate::modules::Modules;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::tokio_util;

use futures::sync::mpsc as async_mpsc;
use futures::Future;
use libc::c_char;
use libc::c_void;
use std;
use std::cell::Cell;
use std::cell::RefCell;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::{Once, ONCE_INIT};
use std::time::Duration;
use std::time::Instant;
use tokio;

use deno_core::Behavior;
use deno_core::JSError;

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
  pub worker_channels: Option<Mutex<WorkerChannels>>,
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
      worker_channels: worker_channels.map(Mutex::new),
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

  #[cfg(test)]
  pub fn mock() -> Arc<IsolateState> {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    // For debugging: argv.push_back(String::from("-D"));
    let (flags, rest_argv, _) = flags::set_flags(argv).unwrap();
    Arc::new(IsolateState::new(flags, rest_argv, None))
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
