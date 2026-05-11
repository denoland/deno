// Copyright 2018-2026 the Deno authors. MIT license.

//! Persistent (OS-scheduled) cron backend.
//!
//! Persistent crons are registered with the host OS scheduler — crontab on
//! Linux, launchd on macOS, schtasks on Windows — so they survive process
//! exit and reboot. Per-platform backends will be added in follow-up PRs;
//! for now every platform reports unsupported.

use crate::CronError;
use crate::PersistentCronEntry;
use crate::PersistentCronHandler;
use crate::PersistentCronSpec;

/// Default handler used until per-platform backends land. Every operation
/// fails with [`CronError::Unsupported`].
pub struct UnimplementedPersistentCronHandler;

impl PersistentCronHandler for UnimplementedPersistentCronHandler {
  fn register(&self, _spec: PersistentCronSpec) -> Result<(), CronError> {
    Err(unsupported())
  }

  fn unregister(&self, _name: &str) -> Result<(), CronError> {
    Err(unsupported())
  }

  fn list(&self) -> Result<Vec<PersistentCronEntry>, CronError> {
    Err(unsupported())
  }
}

/// Top-level persistent-cron handler enum mirroring [`CronHandlerImpl`].
///
/// Currently only `Unimplemented` exists; per-OS variants will be added
/// alongside the launchd/crontab/schtasks backends.
pub enum PersistentCronHandlerImpl {
  Unimplemented(UnimplementedPersistentCronHandler),
}

impl PersistentCronHandlerImpl {
  pub fn create_from_env() -> Self {
    Self::Unimplemented(UnimplementedPersistentCronHandler)
  }
}

impl Default for PersistentCronHandlerImpl {
  fn default() -> Self {
    Self::create_from_env()
  }
}

impl PersistentCronHandler for PersistentCronHandlerImpl {
  fn register(&self, spec: PersistentCronSpec) -> Result<(), CronError> {
    match self {
      Self::Unimplemented(h) => h.register(spec),
    }
  }

  fn unregister(&self, name: &str) -> Result<(), CronError> {
    match self {
      Self::Unimplemented(h) => h.unregister(name),
    }
  }

  fn list(&self) -> Result<Vec<PersistentCronEntry>, CronError> {
    match self {
      Self::Unimplemented(h) => h.list(),
    }
  }
}

fn unsupported() -> CronError {
  CronError::Unsupported(
    "persistent cron is not yet supported on this platform".to_string(),
  )
}
