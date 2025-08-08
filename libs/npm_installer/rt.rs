// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(not(target_arch = "wasm32"))]
use deno_unsync::JoinResult;

#[cfg(target_arch = "wasm32")]
pub type JoinResult<T> = Result<T, std::convert::Infallible>;

pub async fn spawn_blocking<
  F: (FnOnce() -> R) + Send + 'static,
  R: Send + 'static,
>(
  f: F,
) -> JoinResult<R> {
  #[cfg(target_arch = "wasm32")]
  {
    let result = f();
    Ok(result)
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    deno_unsync::spawn_blocking_optional(f).await
  }
}
