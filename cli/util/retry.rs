// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::future::Future;
use std::time::Duration;

pub fn retry<F: FnMut() -> Fut, Fut: Future<Output = Result<T, E>>, T, E>(
  mut f: F,
) -> impl Future<Output = Result<T, E>> {
  const ATTEMPTS: usize = 5;
  const MAX_WAIT: Duration = Duration::from_secs(1);
  const MIN_WAIT: Duration = Duration::from_micros(1);
  async move {
    let mut wait = Duration::from_millis(1);
    let mut attempt = 1;
    loop {
      let result = f().await;
      if result.is_ok() {
        return result;
      }
      if attempt >= ATTEMPTS {
        return result;
      }
      tokio::time::sleep(wait).await;
      attempt += 1;
      wait *= 10;
      wait = wait.clamp(MIN_WAIT, MAX_WAIT);
    }
  }
}
