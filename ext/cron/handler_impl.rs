// Copyright 2018-2026 the Deno authors. MIT license.

use async_trait::async_trait;

use crate::CronError;
use crate::CronHandle;
use crate::CronHandler;
use crate::CronNextResult;
use crate::CronSpec;
use crate::local::CronExecutionHandle;
use crate::local::LocalCronHandler;
use crate::socket::SocketCronHandle;
use crate::socket::SocketCronHandler;

pub enum CronHandlerImpl {
  Local(LocalCronHandler),
  Socket(SocketCronHandler),
}

impl CronHandlerImpl {
  pub fn create_from_env() -> Self {
    match std::env::var("DENO_UNSTABLE_CRON_SOCK") {
      Ok(socket_addr) => Self::Socket(SocketCronHandler::new(socket_addr)),
      Err(_) => Self::Local(LocalCronHandler::new()),
    }
  }
}

impl CronHandler for CronHandlerImpl {
  type EH = CronHandleImpl;

  fn create(&self, spec: CronSpec) -> Result<Self::EH, CronError> {
    match self {
      Self::Local(h) => h.create(spec).map(CronHandleImpl::Local),
      Self::Socket(h) => h.create(spec).map(CronHandleImpl::Socket),
    }
  }
}

pub enum CronHandleImpl {
  Local(CronExecutionHandle),
  Socket(SocketCronHandle),
}

#[async_trait(?Send)]
impl CronHandle for CronHandleImpl {
  async fn next(
    &self,
    prev_success: bool,
  ) -> Result<CronNextResult, CronError> {
    match self {
      Self::Local(h) => h.next(prev_success).await,
      Self::Socket(h) => h.next(prev_success).await,
    }
  }

  fn close(&self) {
    match self {
      Self::Local(h) => h.close(),
      Self::Socket(h) => h.close(),
    }
  }
}
