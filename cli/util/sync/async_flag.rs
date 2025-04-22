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
    self.0.add_permits(1);
  }

  pub async fn wait_raised(&self) {
    drop(self.0.acquire().await);
  }
}
