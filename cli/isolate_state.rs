// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_dir;
use crate::errors::DenoResult;
use crate::flags;
use crate::global_timer::GlobalTimer;
use crate::modules::Modules;
use crate::permissions::DenoPermissions;
use crate::resources;
use crate::resources::ResourceId;
use crate::worker::Worker;
use deno::Buf;
use futures::future::Shared;
use std;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
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

// Isolate cannot be passed between threads but IsolateState can.
// IsolateState satisfies Send and Sync.
// So any state that needs to be accessed outside the main V8 thread should be
// inside IsolateState.
#[cfg_attr(feature = "cargo-clippy", allow(stutter))]
pub struct IsolateState {
  pub dir: deno_dir::DenoDir,
  pub argv: Vec<String>,
  pub permissions: DenoPermissions,
  pub flags: flags::DenoFlags,
  pub metrics: Metrics,
  pub modules: Mutex<Modules>,
  pub worker_channels: Mutex<WorkerChannels>,
  pub global_timer: Mutex<GlobalTimer>,
  pub workers: Mutex<UserWorkerTable>,
  pub start_time: Instant,
  pub resource: resources::Resource,
}

impl IsolateState {
  pub fn new(flags: flags::DenoFlags, argv_rest: Vec<String>) -> Self {
    let custom_root = env::var("DENO_DIR").map(|s| s.into()).ok();

    let (worker_in_tx, worker_in_rx) = async_mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = async_mpsc::channel::<Buf>(1);
    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);
    let resource = resources::add_worker(external_channels);

    Self {
      dir: deno_dir::DenoDir::new(custom_root).unwrap(),
      argv: argv_rest,
      permissions: DenoPermissions::from_flags(&flags),
      flags,
      metrics: Metrics::default(),
      modules: Mutex::new(Modules::new()),
      worker_channels: Mutex::new(internal_channels),
      global_timer: Mutex::new(GlobalTimer::new()),
      workers: Mutex::new(UserWorkerTable::new()),
      start_time: Instant::now(),
      resource,
    }
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
  pub fn mock() -> IsolateState {
    let argv = vec![String::from("./deno"), String::from("hello.js")];
    // For debugging: argv.push_back(String::from("-D"));
    let (flags, rest_argv) = flags::set_flags(argv).unwrap();
    IsolateState::new(flags, rest_argv)
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
