// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Some code and comments under MIT license where adapted from Tokio code
// Copyright (c) 2023 Tokio Contributors

use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use futures::Future;
use tokio::task::AbortHandle;
use tokio::task::JoinError;

use crate::task::MaskFutureAsSend;
use crate::task::MaskResultAsSend;

/// Wraps the tokio [`JoinSet`] to make it !Send-friendly and to make it easier and safer for us to
/// poll while empty.
pub(crate) struct JoinSet<T> {
  joinset: tokio::task::JoinSet<MaskResultAsSend<T>>,
  /// If join_next returns Ready(None), we stash the waker
  waker: Option<Waker>,
}

impl<T> Default for JoinSet<T> {
  fn default() -> Self {
    Self {
      joinset: Default::default(),
      waker: None,
    }
  }
}

impl<T: 'static> JoinSet<T> {
  /// Spawn the provided task on the `JoinSet`, returning an [`AbortHandle`]
  /// that can be used to remotely cancel the task.
  ///
  /// The provided future will start running in the background immediately
  /// when this method is called, even if you don't await anything on this
  /// `JoinSet`.
  ///
  /// # Panics
  ///
  /// This method panics if called outside of a Tokio runtime.
  ///
  /// [`AbortHandle`]: tokio::task::AbortHandle
  #[track_caller]
  pub fn spawn<F>(&mut self, task: F) -> AbortHandle
  where
    F: Future<Output = T>,
    F: 'static,
    T: 'static,
  {
    // SAFETY: We only use this with the single-thread executor
    let handle = self.joinset.spawn(unsafe { MaskFutureAsSend::new(task) });

    // If someone had called poll_join_next while we were empty, ask them to poll again
    // so we can properly register the waker with the underlying JoinSet.
    if let Some(waker) = self.waker.take() {
      waker.wake();
    }
    handle
  }

  /// Returns the number of tasks currently in the `JoinSet`.
  pub fn len(&self) -> usize {
    self.joinset.len()
  }

  /// Waits until one of the tasks in the set completes and returns its output.
  ///
  /// # Cancel Safety
  ///
  /// This method is cancel safe. If `join_next` is used as the event in a `tokio::select!`
  /// statement and some other branch completes first, it is guaranteed that no tasks were
  /// removed from this `JoinSet`.
  pub fn poll_join_next(
    &mut self,
    cx: &mut Context,
  ) -> Poll<Result<T, JoinError>> {
    // TODO(mmastrac): Use poll_join_next from Tokio
    let next = std::pin::pin!(self.joinset.join_next());
    match next.poll(cx) {
      Poll::Ready(Some(res)) => Poll::Ready(res.map(|res| res.into_inner())),
      Poll::Ready(None) => {
        // Stash waker
        self.waker = Some(cx.waker().clone());
        Poll::Pending
      }
      Poll::Pending => Poll::Pending,
    }
  }
}
