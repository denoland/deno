// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(unused_variables)]
#![allow(dead_code)]

use crate::errors::DenoResult;
use crate::isolate_state::IsolateState;
use crate::ops;
use crate::permissions::DenoPermissions;
use deno_core::deno_buf;
use deno_core::deno_mod;
use deno_core::Behavior;
use deno_core::Op;
use deno_core::StartupData;
use std::sync::atomic::Ordering;
use std::sync::Arc;

// Buf represents a byte array returned from a "Op". The message might be empty
// (which will be translated into a null object on the javascript side) or it is
// a heap allocated opaque sequence of bytes.  Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

/// Implements deno_core::Behavior for the main Deno command-line.
pub struct Cli {
  startup_data: Option<StartupData>,
  pub state: Arc<IsolateState>,
  pub permissions: Arc<DenoPermissions>, // TODO(ry) move to IsolateState
}

impl Cli {
  pub fn new(
    startup_data: Option<StartupData>,
    state: Arc<IsolateState>,
    permissions: DenoPermissions,
  ) -> Self {
    Self {
      startup_data,
      state,
      permissions: Arc::new(permissions),
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
}

impl Behavior for Cli {
  fn startup_data(&mut self) -> Option<StartupData> {
    self.startup_data.take()
  }

  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod {
    self
      .state
      .metrics
      .resolve_count
      .fetch_add(1, Ordering::Relaxed);
    let mut modules = self.state.modules.lock().unwrap();
    modules.resolve_cb(&self.state.dir, specifier, referrer)
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch(self, control, zero_copy)
  }
}
