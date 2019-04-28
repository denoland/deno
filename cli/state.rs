// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir;
use crate::errors::DenoResult;
use crate::flags;
use crate::global_timer::GlobalTimer;
use crate::ops;
use crate::permissions::DenoPermissions;
use crate::resources;
use crate::resources::ResourceId;
use crate::worker::Worker;
use deno::Buf;
use deno::Op;
use deno::PinnedBuf;
use futures::future::Shared;
use std;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::mpsc as async_mpsc;

pub type WorkerSender = async_mpsc::Sender<Buf>;
pub type WorkerReceiver = async_mpsc::Receiver<Buf>;
pub type WorkerChannels = (WorkerSender, WorkerReceiver);
pub type UserWorkerTable = HashMap<ResourceId, Shared<Worker>>;

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

// Wrap State so that it can implement Dispatch.
pub struct ThreadSafeState(Arc<State>);

// Isolate cannot be passed between threads but ThreadSafeState can.
// ThreadSafeState satisfies Send and Sync.
// So any state that needs to be accessed outside the main V8 thread should be
// inside ThreadSafeState.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct State {
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub permissions: DenoPermissions,
  pub flags: flags::DenoFlags,
  /// When flags contains a `.config_path` option, the content of the
  /// configuration file will be resolved and set.
  pub config: Option<Vec<u8>>,
  /// When flags contains a `.config_path` option, the fully qualified path
  /// name of the passed path will be resolved and set.
  pub config_path: Option<String>,
  pub metrics: Metrics,
  pub worker_channels: Mutex<WorkerChannels>,
  pub global_timer: Mutex<GlobalTimer>,
  pub workers: Mutex<UserWorkerTable>,
  pub start_time: Instant,
  pub resource: resources::Resource,
  pub dispatch_selector: ops::OpSelector,
}

impl Clone for ThreadSafeState {
  fn clone(&self) -> Self {
    ThreadSafeState(self.0.clone())
  }
}

impl Deref for ThreadSafeState {
  type Target = Arc<State>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ThreadSafeState {
  pub fn dispatch(
    &self,
    control: &[u8],
    zero_copy: Option<PinnedBuf>,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, self.dispatch_selector)
  }
}

impl ThreadSafeState {
  pub fn new(
    flags: flags::DenoFlags,
    argv_rest: Vec<String>,
    dispatch_selector: ops::OpSelector,
  ) -> Self {
    let custom_root = env::var("DENO_DIR").map(String::into).ok();

    let (worker_in_tx, worker_in_rx) = async_mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = async_mpsc::channel::<Buf>(1);
    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);
    let resource = resources::add_worker(external_channels);

    // take the passed flag and resolve the file name relative to the cwd
    let config_file = match &flags.config_path {
      Some(config_file_name) => {
        debug!("Compiler config file: {}", config_file_name);
        let cwd = std::env::current_dir().unwrap();
        Some(cwd.join(config_file_name))
      }
      _ => None,
    };

    // Convert the PathBuf to a canonicalized string.  This is needed by the
    // compiler to properly deal with the configuration.
    let config_path = match &config_file {
      Some(config_file) => Some(
        config_file
          .canonicalize()
          .unwrap()
          .to_str()
          .unwrap()
          .to_owned(),
      ),
      _ => None,
    };

    // Load the contents of the configuration file
    let config = match &config_file {
      Some(config_file) => {
        debug!("Attempt to load config: {}", config_file.to_str().unwrap());
        match fs::read(&config_file) {
          Ok(config_data) => Some(config_data.to_owned()),
          _ => panic!(
            "Error retrieving compiler config file at \"{}\"",
            config_file.to_str().unwrap()
          ),
        }
      }
      _ => None,
    };

    ThreadSafeState(Arc::new(State {
      dir: deno_dir::DenoDir::new(custom_root, &config).unwrap(),
      argv: argv_rest,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      config,
      config_path,
      metrics: Metrics::default(),
      worker_channels: Mutex::new(internal_channels),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(UserWorkerTable::new()),
      start_time: Instant::now(),
      resource,
      dispatch_selector,
    }))
  }

  /// Read main module from argv
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

  #[inline]
  pub fn check_read(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_read(filename)
  }

  #[inline]
  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_write(filename)
  }

  #[inline]
  pub fn check_env(&self) -> DenoResult<()> {
    self.permissions.check_env()
  }

  #[inline]
  pub fn check_net(&self, filename: &str) -> DenoResult<()> {
    self.permissions.check_net(filename)
  }

  #[inline]
  pub fn check_run(&self) -> DenoResult<()> {
    self.permissions.check_run()
  }

  #[cfg(test)]
  pub fn mock() -> ThreadSafeState {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    ThreadSafeState::new(
      flags::DenoFlags::default(),
      argv,
      ops::op_selector_std,
    )
  }

  pub fn metrics_op_dispatched(
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

  pub fn metrics_op_completed(&self, bytes_received: usize) {
    self.metrics.ops_completed.fetch_add(1, Ordering::SeqCst);
    self
      .metrics
      .bytes_received
      .fetch_add(bytes_received, Ordering::SeqCst);
  }
}

#[test]
fn thread_safe() {
  fn f<S: Send + Sync>(_: S) {}
  f(ThreadSafeState::mock());
}
