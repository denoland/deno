// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub mod io;
pub mod ops;
pub mod ops_tls;
#[cfg(unix)]
pub mod ops_unix;
pub mod resolve_addr;

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::Extension;
use deno_core::OpState;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub trait NetPermissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
  ) -> Result<(), AnyError>;
  fn check_read(&mut self, _p: &Path) -> Result<(), AnyError>;
  fn check_write(&mut self, _p: &Path) -> Result<(), AnyError>;
}

/// For use with this crate when the user does not want permission checks.
pub struct NoNetPermissions;

impl NetPermissions for NoNetPermissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
  ) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_read(&mut self, _p: &Path) -> Result<(), AnyError> {
    Ok(())
  }

  fn check_write(&mut self, _p: &Path) -> Result<(), AnyError> {
    Ok(())
  }
}

/// `UnstableChecker` is a struct so it can be placed inside `GothamState`;
/// using type alias for a bool could work, but there's a high chance
/// that there might be another type alias pointing to a bool, which
/// would override previously used alias.
pub struct UnstableChecker {
  pub unstable: bool,
}

impl UnstableChecker {
  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.unstable {
      eprintln!(
        "Unstable API '{}'. The --unstable flag must be provided.",
        api_name
      );
      std::process::exit(70);
    }
  }
}
/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<UnstableChecker>().check_unstable(api_name)
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_net.d.ts")
}

pub fn get_unstable_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_net.unstable.d.ts")
}

pub fn init<P: NetPermissions + 'static>(unstable: bool) -> Extension {
  let mut ops_to_register = vec![];
  ops_to_register.extend(io::init());
  ops_to_register.extend(ops::init::<P>());
  ops_to_register.extend(ops_tls::init::<P>());

  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/net",
      "01_net.js",
      "02_tls.js",
      "04_net_unstable.js",
    ))
    .ops(ops_to_register)
    .state(move |state| {
      state.put(UnstableChecker { unstable });
      Ok(())
    })
    .build()
}
