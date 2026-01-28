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
