// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct AsyncFlag(Arc<Semaphore>);

impl Default for AsyncFlag {
  fn default() -> Self {
    Self(Arc::new(Semaphore::new(0)))
  }
}

impl AsyncFlag {
  pub fn raise(&self) {
    self.0.close();
  }

  pub fn is_raised(&self) -> bool {
    self.0.is_closed()
  }

  pub async fn wait_raised(&self) {
    self.0.acquire().await.unwrap_err();
  }
}
