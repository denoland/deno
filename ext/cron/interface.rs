// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use async_trait::async_trait;
use deno_core::error::AnyError;

pub trait CronHandler {
  type EH: CronHandle + 'static;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, AnyError>;
}

#[async_trait(?Send)]
pub trait CronHandle {
  async fn next(&self, prev_success: bool) -> Result<bool, AnyError>;
  fn close(&self);
}

#[derive(Clone)]
pub struct CronSpec {
  pub name: String,
  pub cron_schedule: String,
  pub backoff_schedule: Option<Vec<u32>>,
}
