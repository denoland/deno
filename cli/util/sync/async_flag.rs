// Copyright 2018-2025 the Deno authors. MIT license.

use tokio_util::sync::CancellationToken;

#[derive(Default, Clone)]
#[cfg_attr(any(test, debug_assertions), derive(Debug))]
pub struct AsyncFlag(CancellationToken);

impl AsyncFlag {
  pub fn raise(&self) {
    self.0.cancel();
  }

  pub fn is_raised(&self) -> bool {
    self.0.is_cancelled()
  }

  pub fn wait_raised(&self) -> impl std::future::Future<Output = ()> + '_ {
    self.0.cancelled()
  }
}
