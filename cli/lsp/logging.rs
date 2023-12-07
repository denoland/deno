// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::parking_lot::Mutex;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

static LSP_DEBUG_FLAG: AtomicBool = AtomicBool::new(false);
static LSP_LOG_LEVEL: AtomicUsize = AtomicUsize::new(log::Level::Info as usize);
static LSP_WARN_LEVEL: AtomicUsize =
  AtomicUsize::new(log::Level::Warn as usize);
static LOG_FILE: Mutex<Option<LogFile>> = Mutex::new(None);

pub struct LogFile {
  path: PathBuf,
  buffer: String,
}

impl LogFile {
  pub fn write_line(&mut self, s: &str) {
    self.buffer.push_str(s);
    self.buffer.push('\n');
  }

  fn commit(&mut self) {
    if !self.buffer.is_empty() {
      if let Ok(file) = fs::OpenOptions::new().append(true).open(&self.path) {
        if write!(&file, "{}", &self.buffer).is_ok() {
          self.buffer.clear();
        }
      }
    }
  }
}

pub fn init_log_file(path: PathBuf) {
  fs::write(&path, "").ok();
  *LOG_FILE.lock() = Some(LogFile {
    path,
    buffer: String::with_capacity(1024),
  });
  thread::spawn(|| loop {
    thread::sleep(std::time::Duration::from_secs(1));
    if let Some(log_file) = &mut *LOG_FILE.lock() {
      log_file.commit();
    }
  });
}

pub fn write_line_to_log_file(s: &str) {
  if let Some(log_file) = &mut *LOG_FILE.lock() {
    log_file.write_line(s);
  }
}

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
      let s = std::format!($($arg)+);
      $crate::lsp::logging::write_line_to_log_file(&s);
      log::log!(lsp_log_level, "{}", s)
    }
  )
}

/// Use this macro to do "warn" logs in the lsp code. This allows
/// for downgrading these logs to another log level in the REPL.
macro_rules! lsp_warn {
  ($($arg:tt)+) => (
    {
      let lsp_log_level = $crate::lsp::logging::lsp_warn_level();
      if lsp_log_level == log::Level::Debug {
        $crate::lsp::logging::lsp_debug!($($arg)+)
      } else {
        let s = std::format!($($arg)+);
        $crate::lsp::logging::write_line_to_log_file(&s);
        log::log!(lsp_log_level, "{}", s)
      }
    }
  )
}

macro_rules! lsp_debug {
  ($($arg:tt)+) => (
    {
      let s = std::format!($($arg)+);
      $crate::lsp::logging::write_line_to_log_file(&s);
      if $crate::lsp::logging::lsp_debug_enabled() {
        log::debug!("{}", s)
      }
    }
  )
}

pub(super) use lsp_debug;
pub(super) use lsp_log;
pub(super) use lsp_warn;
