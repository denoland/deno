// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use chrono::DateTime;
use chrono::Utc;
use deno_core::parking_lot::Mutex;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::SystemTime;

static LSP_DEBUG_FLAG: AtomicBool = AtomicBool::new(false);
static LSP_LOG_LEVEL: AtomicUsize = AtomicUsize::new(log::Level::Info as usize);
static LSP_WARN_LEVEL: AtomicUsize =
  AtomicUsize::new(log::Level::Warn as usize);
static LOG_FILE: LogFile = LogFile {
  enabled: AtomicBool::new(true),
  buffer: Mutex::new(String::new()),
};

pub struct LogFile {
  enabled: AtomicBool,
  buffer: Mutex<String>,
}

impl LogFile {
  pub fn write_line(&self, s: &str) {
    if LOG_FILE.enabled.load(Ordering::Relaxed) {
      let mut buffer = self.buffer.lock();
      buffer.push_str(s);
      buffer.push('\n');
    }
  }

  fn commit(&self, path: &Path) {
    let unbuffered = {
      let mut buffer = self.buffer.lock();
      if buffer.is_empty() {
        return;
      }
      // We clone here rather than take so the buffer can retain its capacity.
      let unbuffered = buffer.clone();
      buffer.clear();
      unbuffered
    };
    if let Ok(file) = fs::OpenOptions::new().append(true).open(path) {
      write!(&file, "{}", unbuffered).ok();
    }
  }
}

pub fn init_log_file(enabled: bool) {
  let prepare_path = || {
    if !enabled {
      return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let now = SystemTime::now();
    let now: DateTime<Utc> = now.into();
    let now = now.to_rfc3339().replace(':', "_");
    let path = cwd.join(format!(".deno_lsp/log_{}.txt", now));
    fs::create_dir_all(path.parent()?).ok()?;
    fs::write(&path, "").ok()?;
    Some(path)
  };
  let Some(path) = prepare_path() else {
    LOG_FILE.enabled.store(false, Ordering::Relaxed);
    LOG_FILE.buffer.lock().clear();
    return;
  };
  thread::spawn(move || loop {
    LOG_FILE.commit(&path);
    thread::sleep(std::time::Duration::from_secs(1));
  });
}

pub fn write_line_to_log_file(s: &str) {
  LOG_FILE.write_line(s);
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
