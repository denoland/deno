// Copyright 2018-2026 the Deno authors. MIT license.

use async_trait::async_trait;

use crate::CronError;

pub type Traceparent = Option<String>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CronNextResult {
  pub active: bool,
  pub traceparent: Traceparent,
}

pub trait CronHandler {
  type EH: CronHandle + 'static;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError>;
}

#[async_trait(?Send)]
pub trait CronHandle {
  async fn next(&self, prev_success: bool)
  -> Result<CronNextResult, CronError>;
  fn close(&self);
}

#[derive(Clone)]
pub struct CronSpec {
  pub name: String,
  pub cron_schedule: String,
  pub backoff_schedule: Option<Vec<u32>>,
}

/// Registration spec for a persistent (OS-scheduled) cron.
///
/// Unlike `CronSpec`, the runtime that registers a persistent cron does not
/// stay alive — the host OS scheduler is responsible for invoking
/// `deno cron exec <script>` at each scheduled time.
#[derive(Clone, Debug)]
pub struct PersistentCronSpec {
  pub name: String,
  pub cron_schedule: String,
  pub script: String,
  pub cwd: Option<String>,
  pub permissions: Vec<String>,
  pub env: Vec<(String, String)>,
}

/// One row returned by `PersistentCronHandler::list()`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PersistentCronEntry {
  pub name: String,
  pub schedule: String,
  pub script: String,
}

/// Backend for OS-scheduled (persistent) crons.
///
/// All methods are synchronous and are expected to complete quickly —
/// registration writes to user-scoped scheduler state (crontab/launchd/
/// schtasks) rather than waiting for cron deadlines.
pub trait PersistentCronHandler {
  fn register(&self, spec: PersistentCronSpec) -> Result<(), CronError>;
  fn unregister(&self, name: &str) -> Result<(), CronError>;
  fn list(&self) -> Result<Vec<PersistentCronEntry>, CronError>;
}
