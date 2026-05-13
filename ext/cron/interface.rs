// Copyright 2018-2026 the Deno authors. MIT license.

use std::rc::Rc;

use async_trait::async_trait;

use crate::CronError;

pub type Traceparent = Option<String>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CronNextResult {
  pub active: bool,
  pub traceparent: Traceparent,
}

pub trait CronHandler {
  fn create(&self, spec: CronSpec) -> Result<Rc<dyn CronHandle>, CronError>;

  /// Check if the handler should be replaced based on current environment.
  /// Returns a fresh handler when a reload is needed, `None` otherwise.
  /// Called when a `MainWorker` hydrates an unconfigured runtime, since the
  /// snapshot captured a handler built from the environment at snapshot
  /// time rather than at run time.
  fn maybe_reload(&self) -> Option<Box<dyn CronHandler>> {
    None
  }
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
