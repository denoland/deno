// Copyright 2018-2025 the Deno authors. MIT license.

use async_trait::async_trait;

use crate::CronError;

pub trait CronHandler {
  type EH: CronHandle + 'static;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError>;
}

#[async_trait(?Send)]
pub trait CronHandle {
  async fn next(&self, prev_success: bool) -> Result<bool, CronError>;
  fn close(&self);
}

#[derive(Clone)]
pub struct CronSpec {
  pub name: String,
  pub cron_schedule: String,
  pub backoff_schedule: Option<Vec<u32>>,
}
