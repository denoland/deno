// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;
use std::ops::DerefMut;
use std::task::Context;
use std::task::Poll;

use futures::StreamExt;
use futures::stream::FuturesUnordered;

/// A `FuturesUnordered` paired with a `Cell<bool>` flag that tracks whether
/// the collection has pending items. The flag avoids borrowing the `RefCell`
/// just to check `is_empty()`.
pub(super) struct TrackedFutures<F> {
  futs: RefCell<FuturesUnordered<F>>,
  pending: Cell<bool>,
}

impl<F> Default for TrackedFutures<F> {
  fn default() -> Self {
    Self {
      futs: Default::default(),
      pending: Cell::new(false),
    }
  }
}

impl<F: Future + Unpin> TrackedFutures<F> {
  pub(super) fn is_pending(&self) -> bool {
    self.pending.get()
  }

  pub(super) fn push(&self, fut: F) {
    self.futs.borrow_mut().push(fut);
    self.pending.set(true);
  }

  /// Polls the inner `FuturesUnordered`. When the result is not
  /// `Ready(Some(_))` (i.e., no more ready items), the pending flag is
  /// synced from the collection's emptiness.
  pub(super) fn poll_next_unpin(
    &self,
    cx: &mut Context,
  ) -> Poll<Option<F::Output>> {
    let poll = self.futs.borrow_mut().poll_next_unpin(cx);
    if !matches!(poll, Poll::Ready(Some(_))) {
      self.pending.set(!self.futs.borrow().is_empty());
    }
    poll
  }

  pub(super) fn clear(&self) {
    self.futs.borrow_mut().clear();
    self.pending.set(false);
  }
}

/// A `Vec<T>` paired with a `Cell<bool>` flag that tracks whether the
/// collection has pending items.
pub(super) struct TrackedVec<T> {
  vec: RefCell<Vec<T>>,
  pending: Cell<bool>,
}

impl<T> Default for TrackedVec<T> {
  fn default() -> Self {
    Self {
      vec: RefCell::new(Vec::new()),
      pending: Cell::new(false),
    }
  }
}

impl<T> TrackedVec<T> {
  pub(super) fn is_pending(&self) -> bool {
    self.pending.get()
  }

  pub(super) fn push(&self, item: T) {
    self.vec.borrow_mut().push(item);
    self.pending.set(true);
  }

  pub(super) fn take(&self) -> Vec<T> {
    let v = std::mem::take(self.vec.borrow_mut().deref_mut());
    self.pending.set(false);
    v
  }

  pub(super) fn set(&self, items: Vec<T>) {
    self.pending.set(!items.is_empty());
    *self.vec.borrow_mut() = items;
  }

  pub(super) fn borrow(&self) -> std::cell::Ref<'_, Vec<T>> {
    self.vec.borrow()
  }

  pub(super) fn clear(&self) {
    self.vec.borrow_mut().clear();
    self.pending.set(false);
  }
}
