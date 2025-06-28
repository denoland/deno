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

#[cfg(not(target_arch = "wasm32"))]
pub use deno_unsync::sync::MultiRuntimeAsyncValueCreator;

#[cfg(target_arch = "wasm32")]
mod wasm {
  use futures::future::LocalBoxFuture;

  type CreateFutureFn<TResult> =
    Box<dyn Fn() -> LocalBoxFuture<'static, TResult> + Send + Sync>;

  pub struct MultiRuntimeAsyncValueCreator<TResult: Send + Clone + 'static> {
    create_future: CreateFutureFn<TResult>,
  }

  impl<TResult: Send + Clone + 'static> std::fmt::Debug
    for MultiRuntimeAsyncValueCreator<TResult>
  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("MultiRuntimeAsyncValueCreator").finish()
    }
  }

  impl<TResult: Send + Clone + 'static> MultiRuntimeAsyncValueCreator<TResult> {
    pub fn new(create_future: CreateFutureFn<TResult>) -> Self {
      Self { create_future }
    }

    pub async fn get(&self) -> TResult {
      (self.create_future)().await
    }
  }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::MultiRuntimeAsyncValueCreator;
