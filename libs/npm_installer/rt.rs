// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(not(target_arch = "wasm32"))]
use deno_unsync::JoinHandle;
#[cfg(target_arch = "wasm32")]
pub type JoinHandle<T> =
  std::future::Ready<Result<T, std::convert::Infallible>>;

pub fn spawn_blocking<
  F: (FnOnce() -> R) + Send + 'static,
  R: Send + 'static,
>(
  f: F,
) -> JoinHandle<R> {
  #[cfg(target_arch = "wasm32")]
  {
    let result = f();
    std::future::ready(Ok(result))
  }
  #[cfg(not(target_arch = "wasm32"))]
  {
    deno_unsync::spawn_blocking(f)
  }
}
