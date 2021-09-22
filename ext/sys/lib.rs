// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub mod error;
pub mod fs;
pub mod fs_events;
pub mod io;
pub mod os;
pub mod process;
pub mod signal;
pub mod tty;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::Extension;
use deno_core::OpState;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub fn base_init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/sys",
      "01_build.js",
      "01_errors.js",
      "01_version.js",
      "01_web_util.js",
      "02_util.js",
    ))
    .build()
}

pub struct Unstable(pub bool);
impl Unstable {
  /// Quits the process if the --unstable flag was not provided.
  ///
  /// This is intentionally a non-recoverable check so that people cannot probe
  /// for unstable APIs from stable programs.
  // NOTE(bartlomieju): keep in sync with `cli/program_state.rs`
  pub fn check_unstable(&self, api_name: &str) {
    if !self.0 {
      eprintln!(
        "Unstable API '{}'. The --unstable flag must be provided.",
        api_name
      );
      std::process::exit(70);
    }
  }
}

/// Helper for checking unstable features. Used for sync ops.
fn check_unstable(state: &OpState, api_name: &str) {
  state.borrow::<Unstable>().check_unstable(api_name)
}

/// Helper for checking unstable features. Used for async ops.
fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  state.borrow::<Unstable>().check_unstable(api_name)
}

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {:?} is not valid UTF-8", s);
    custom_error("InvalidData", message)
  })
}

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, std::io::Error> {
  let mut canonicalized_path = path.canonicalize()?;
  if cfg!(windows) {
    canonicalized_path = PathBuf::from(
      canonicalized_path
        .display()
        .to_string()
        .trim_start_matches("\\\\?\\"),
    );
  }
  Ok(canonicalized_path)
}
