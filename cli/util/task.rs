// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use tokio_util::sync::CancellationToken;

pub async fn run_future_with_ctrl_c_cancellation<TOutput>(
  token: CancellationToken,
  future: impl std::future::Future<Output = TOutput>,
) -> TOutput {
  let drop_guard = token.drop_guard();
  tokio::pin!(future);
  tokio::select! {
    result = &mut future => {
      drop_guard.disarm();
      result
    }
    _ = tokio::signal::ctrl_c() => {
      drop(drop_guard); // cancel the token
      future.await
    }
  }
}
