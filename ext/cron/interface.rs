// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::CronError;
use async_trait::async_trait;

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
