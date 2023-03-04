// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

pub struct SingleConcurrencyEnforcerPermit<'a>(SemaphorePermit<'a>);

/// Enforces only one request can enter the section of code
/// that holds a permit.
#[derive(Debug)]
pub struct SingleConcurrencyEnforcer(Semaphore);

impl Default for SingleConcurrencyEnforcer {
  fn default() -> Self {
    Self(Semaphore::new(1))
  }
}

impl SingleConcurrencyEnforcer {
  /// Acquire a permit to force other calls to `acquire` to wait until the
  /// permit is dropped. Permits are acquired in order (first-in, first-out).
  pub async fn acquire(&self) -> SingleConcurrencyEnforcerPermit {
    // it is ok to unwrap here because we don't ever call `self.0.close()`
    let permit = self.0.acquire().await.unwrap();
    SingleConcurrencyEnforcerPermit(permit)
  }
}
