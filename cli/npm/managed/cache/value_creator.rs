// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use tokio::task::JoinError;

// todo(dsherret): unit test this

type FutureResult<TResult> = Result<TResult, Arc<AnyError>>;
type JoinResult<TResult> = Result<FutureResult<TResult>, Arc<JoinError>>;

#[derive(Debug)]
struct State<TResult> {
  retry_index: usize,
  future: Shared<BoxFuture<'static, JoinResult<TResult>>>,
}

/// Attempts to create a shared value asynchronously on one tokio runtime while
/// many runtimes are requesting the value.
///
/// This is only useful when the value needs to get created once across
/// many runtimes.
///
/// This handles the case where one tokio runtime goes down while another
/// one is still running.
#[derive(Debug)]
pub struct MultiRuntimeAsyncValueCreator<TResult: Send + Clone + 'static> {
  state: Mutex<State<TResult>>,
}

impl<TResult: Send + Clone + 'static> MultiRuntimeAsyncValueCreator<TResult> {
  pub fn new(
    future: LocalBoxFuture<'static, Result<TResult, AnyError>>,
  ) -> Self {
    Self {
      state: Mutex::new(State {
        retry_index: 0,
        future: Self::create_shared_future(future),
      }),
    }
  }

  pub async fn get(
    &self,
    recreate_future: impl Fn() -> LocalBoxFuture<'static, Result<TResult, AnyError>>,
  ) -> Result<TResult, Arc<AnyError>> {
    let (mut future, mut retry_index) = {
      let state = self.state.lock();
      (state.future.clone(), state.retry_index)
    };

    loop {
      let result = future.await;

      match result {
        Ok(result) => return result,
        Err(join_error) => {
          if join_error.is_cancelled() {
            let mut state = self.state.lock();

            if state.retry_index == retry_index {
              // we were the first one to retry, so create a new future
              // that we'll run from the current runtime
              state.retry_index += 1;
              state.future = Self::create_shared_future(recreate_future());
            }

            retry_index = state.retry_index;
            future = state.future.clone();

            // just in case we're stuck in a loop
            if retry_index > 1000 {
              panic!("Something went wrong.") // should never happen
            }
          } else {
            panic!("{}", join_error);
          }
        }
      }
    }
  }

  fn create_shared_future(
    future: LocalBoxFuture<'static, Result<TResult, AnyError>>,
  ) -> Shared<BoxFuture<'static, JoinResult<TResult>>> {
    deno_core::unsync::spawn(future)
      .map(|result| match result {
        Ok(Ok(value)) => Ok(Ok(value)),
        Ok(Err(err)) => Ok(Err(Arc::new(err))),
        Err(err) => Err(Arc::new(err)),
      })
      .boxed()
      .shared()
  }
}
