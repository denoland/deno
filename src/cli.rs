// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(unused_variables)]

use crate::errors::DenoResult;
use crate::isolate_init::IsolateInit;
use crate::isolate_state::IsolateState;
use crate::modules::Modules;
use crate::permissions::DenoPermissions;
use deno_core::deno_buf;
use deno_core::deno_mod;
use deno_core::Behavior;
use deno_core::Op;
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

// Buf represents a byte array returned from a "Op".
// The message might be empty (which will be translated into a null object on
// the javascript side) or it is a heap allocated opaque sequence of bytes.
// Usually a flatbuffer message.
pub type Buf = Box<[u8]>;

pub type Isolate = deno_core::Isolate<Buf, Cli>;

pub type CliOp = Op<Buf>;

/// Implements deno_core::Behavior for the main Deno command-line.
pub struct Cli {
  shared: Vec<u8>, // Pin<Vec<u8>> ?
  init: IsolateInit,
  timeout_due: Cell<Option<Instant>>,
  pub modules: RefCell<Modules>,
  pub state: Arc<IsolateState>,
  pub permissions: Arc<DenoPermissions>,
}

impl Cli {
  pub fn new(
    init: IsolateInit,
    state: Arc<IsolateState>,
    permissions: DenoPermissions,
  ) -> Self {
    let mut shared = Vec::new();
    shared.resize(1024 * 1024, 0);
    Self {
      init,
      shared,
      timeout_due: Cell::new(None),
      modules: RefCell::new(Modules::new()),
      state,
      permissions: Arc::new(permissions),
    }
  }

  #[inline]
  pub fn get_timeout_due(&self) -> Option<Instant> {
    self.timeout_due.clone().into_inner()
  }

  #[inline]
  pub fn set_timeout_due(&self, inst: Option<Instant>) {
    self.timeout_due.set(inst);
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

impl Behavior<Buf> for Cli {
  fn startup_snapshot(&mut self) -> Option<deno_buf> {
    self.init.snapshot.take()
  }

  fn startup_shared(&mut self) -> Option<deno_buf> {
    let ptr = self.shared.as_ptr() as *const u8;
    let len = self.shared.len();
    Some(unsafe { deno_buf::from_raw_parts(ptr, len) })
  }

  fn resolve(&mut self, specifier: &str, referrer: deno_mod) -> deno_mod {
    unimplemented!()
  }

  fn recv(
    &mut self,
    record: Buf,
    zero_copy_buf: deno_buf,
  ) -> (bool, Box<CliOp>) {
    unimplemented!()
  }

  fn records_reset(&mut self) {
    unimplemented!()
  }

  fn records_push(&mut self, record: Buf) -> bool {
    unimplemented!()
  }

  fn records_pop(&mut self) -> Option<Buf> {
    unimplemented!()
  }
}
