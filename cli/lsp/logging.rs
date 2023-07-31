// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

static LSP_DEBUG_FLAG: AtomicBool = AtomicBool::new(false);
static LSP_LOG_LEVEL: AtomicUsize = AtomicUsize::new(log::Level::Info as usize);
static LSP_WARN_LEVEL: AtomicUsize =
  AtomicUsize::new(log::Level::Warn as usize);

pub fn set_lsp_debug_flag(value: bool) {
  LSP_DEBUG_FLAG.store(value, Ordering::SeqCst)
}

pub fn lsp_debug_enabled() -> bool {
  LSP_DEBUG_FLAG.load(Ordering::SeqCst)
}

/// Change the lsp to log at the provided level.
pub fn set_lsp_log_level(level: log::Level) {
  LSP_LOG_LEVEL.store(level as usize, Ordering::SeqCst)
}

pub fn lsp_log_level() -> log::Level {
  let level = LSP_LOG_LEVEL.load(Ordering::SeqCst);
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::mem::transmute(level)
  }
}

/// Change the lsp to warn at the provided level.
pub fn set_lsp_warn_level(level: log::Level) {
  LSP_WARN_LEVEL.store(level as usize, Ordering::SeqCst)
}

pub fn lsp_warn_level() -> log::Level {
  let level = LSP_LOG_LEVEL.load(Ordering::SeqCst);
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::mem::transmute(level)
  }
}

/// Use this macro to do "info" logs in the lsp code. This allows
/// for downgrading these logs to another log level in the REPL.
macro_rules! lsp_log {
  ($($arg:tt)+) => (
    let lsp_log_level = $crate::lsp::logging::lsp_log_level();
    if lsp_log_level == log::Level::Debug {
      $crate::lsp::logging::lsp_debug!($($arg)+)
    } else {
      log::log!(lsp_log_level, $($arg)+)
    }
  )
}

/// Use this macro to do "warn" logs in the lsp code. This allows
/// for downgrading these logs to another log level in the REPL.
macro_rules! lsp_warn {
  ($($arg:tt)+) => (
    let lsp_log_level = $crate::lsp::logging::lsp_warn_level();
    if lsp_log_level == log::Level::Debug {
      $crate::lsp::logging::lsp_debug!($($arg)+)
    } else {
      log::log!(lsp_log_level, $($arg)+)
    }
  )
}

macro_rules! lsp_debug {
  ($($arg:tt)+) => (
    if crate::lsp::logging::lsp_debug_enabled() {
      log::debug!($($arg)+)
    }
  )
}

pub(super) use lsp_debug;
pub(super) use lsp_log;
pub(super) use lsp_warn;
