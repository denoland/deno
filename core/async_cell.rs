// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use std::any::Any;
use std::borrow::Borrow;
use std::cell::Cell;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::ops::Deref;
use std::rc::Rc;

use self::internal as i;

pub type AsyncRef<T> = i::AsyncBorrowImpl<T, i::Shared>;
pub type AsyncMut<T> = i::AsyncBorrowImpl<T, i::Exclusive>;

pub type AsyncRefFuture<T> = i::AsyncBorrowFutureImpl<T, i::Shared>;
pub type AsyncMutFuture<T> = i::AsyncBorrowFutureImpl<T, i::Exclusive>;

pub struct AsyncRefCell<T> {
  value: UnsafeCell<T>,
  borrow_count: Cell<i::BorrowCount>,
  waiters: Cell<VecDeque<Option<i::Waiter>>>,
  turn: Cell<usize>,
}

impl<T: 'static> AsyncRefCell<T> {
  /// Create a new `AsyncRefCell` that encapsulates the specified value.
  /// Note that in order to borrow the inner value, the `AsyncRefCell`
  /// needs to be wrapped in an `Rc` or an `RcRef`. These can be created
  /// either manually, or by using the convenience method
  /// `AsyncRefCell::new_rc()`.
  pub fn new(value: T) -> Self {
    Self {
      value: UnsafeCell::new(value),
      borrow_count: Default::default(),
      waiters: Default::default(),
      turn: Default::default(),
    }
  }

  pub fn new_rc(value: T) -> Rc<Self> {
    Rc::new(Self::new(value))
  }

  pub fn as_ptr(&self) -> *mut T {
    self.value.get()
  }
}

impl<T: Default + 'static> Default for AsyncRefCell<T> {
  fn default() -> Self {
    Self::new(Default::default())
  }
}

impl<T: Default + 'static> AsyncRefCell<T> {
  pub fn default_rc() -> Rc<Self> {
    Rc::new(Default::default())
  }
}

impl<T: 'static> From<T> for AsyncRefCell<T> {
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

impl<T> AsyncRefCell<T> {
  pub fn borrow(self: &Rc<Self>) -> AsyncRefFuture<T> {
    AsyncRefFuture::new(self)
  }

  pub fn borrow_mut(self: &Rc<Self>) -> AsyncMutFuture<T> {
    AsyncMutFuture::new(self)
  }

  pub fn try_borrow(self: &Rc<Self>) -> Option<AsyncRef<T>> {
    Self::borrow_sync(self)
  }

  pub fn try_borrow_mut(self: &Rc<Self>) -> Option<AsyncMut<T>> {
    Self::borrow_sync(self)
  }
}

impl<T> RcRef<AsyncRefCell<T>> {
  pub fn borrow(&self) -> AsyncRefFuture<T> {
    AsyncRefFuture::new(self)
  }

  pub fn borrow_mut(&self) -> AsyncMutFuture<T> {
    AsyncMutFuture::new(self)
  }

  pub fn try_borrow(&self) -> Option<AsyncRef<T>> {
    AsyncRefCell::<T>::borrow_sync(self)
  }

  pub fn try_borrow_mut(&self) -> Option<AsyncMut<T>> {
    AsyncRefCell::<T>::borrow_sync(self)
  }
}

/// An `RcRef` encapsulates a reference counted pointer, just like a regular
/// `std::rc::Rc`. However, unlike a regular `Rc`, it can be remapped so that
/// it dereferences to any value that's reachable through the reference-counted
/// pointer. This is achieved through the associated method, `RcRef::map()`,
/// similar to how `std::cell::Ref::map()` works. Example:
///
/// ```rust
/// # use std::rc::Rc;
/// # use deno_core::async_cell::RcRef;
///
/// struct Stuff {
///   foo: u32,
///   bar: String,
/// }
///
/// let stuff_rc = Rc::new(Stuff {
///   foo: 42,
///   bar: "hello".to_owned(),
/// });
///
/// // `foo_rc` and `bar_rc` dereference to different types, however
/// // they share a reference count.
/// let foo_rc: RcRef<u32> = RcRef::map(stuff_rc.clone(), |v| &v.foo);
/// let bar_rc: RcRef<String> = RcRef::map(stuff_rc, |v| &v.bar);
/// ```
#[derive(Debug)]
pub struct RcRef<T> {
  rc: Rc<dyn Any>,
  value: *const T,
}

impl<T: 'static> RcRef<T> {
  pub fn new(value: T) -> Self {
    Self::from(Rc::new(value))
  }

  pub fn map<S: 'static, R: RcLike<S>, F: FnOnce(&S) -> &T>(
    source: R,
    map_fn: F,
  ) -> RcRef<T> {
    let RcRef::<S> { rc, value } = source.into();
    let value = map_fn(unsafe { &*value });
    RcRef { rc, value }
  }

  pub(crate) fn split(rc_ref: &Self) -> (&T, &Rc<dyn Any>) {
    let &Self { ref rc, value } = rc_ref;
    (unsafe { &*value }, rc)
  }
}

impl<T: Default + 'static> Default for RcRef<T> {
  fn default() -> Self {
    Self::new(Default::default())
  }
}

impl<T> Clone for RcRef<T> {
  fn clone(&self) -> Self {
    Self {
      rc: self.rc.clone(),
      value: self.value,
    }
  }
}

impl<T: 'static> From<&RcRef<T>> for RcRef<T> {
  fn from(rc_ref: &RcRef<T>) -> Self {
    rc_ref.clone()
  }
}

impl<T: 'static> From<Rc<T>> for RcRef<T> {
  fn from(rc: Rc<T>) -> Self {
    Self {
      value: &*rc,
      rc: rc as Rc<_>,
    }
  }
}

impl<T: 'static> From<&Rc<T>> for RcRef<T> {
  fn from(rc: &Rc<T>) -> Self {
    rc.clone().into()
  }
}

impl<T> Deref for RcRef<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    unsafe { &*self.value }
  }
}

impl<T> Borrow<T> for RcRef<T> {
  fn borrow(&self) -> &T {
    &**self
  }
}

impl<T> AsRef<T> for RcRef<T> {
  fn as_ref(&self) -> &T {
    &**self
  }
}

/// The `RcLike` trait provides an abstraction over `std::rc::Rc` and `RcRef`,
/// so that applicable methods can operate on either type.
pub trait RcLike<T>: AsRef<T> + Into<RcRef<T>> {}

impl<T: 'static> RcLike<T> for Rc<T> {}
impl<T: 'static> RcLike<T> for RcRef<T> {}
impl<T: 'static> RcLike<T> for &Rc<T> {}
impl<T: 'static> RcLike<T> for &RcRef<T> {}

mod internal {
  use super::AsyncRefCell;
  use super::RcLike;
  use super::RcRef;
  use futures::future::Future;
  use futures::ready;
  use futures::task::Context;
  use futures::task::Poll;
  use futures::task::Waker;
  use std::borrow::Borrow;
  use std::borrow::BorrowMut;
  use std::fmt::Debug;
  use std::marker::PhantomData;
  use std::ops::Deref;
  use std::ops::DerefMut;
  use std::pin::Pin;

  impl<T> AsyncRefCell<T> {
    /// Borrow the cell's contents synchronouslym without creating an
    /// intermediate future. If the cell has already been borrowed and either
    /// the existing or the requested borrow is exclusive, this function returns
    /// `None`.
    pub fn borrow_sync<M: BorrowModeTrait, R: RcLike<AsyncRefCell<T>>>(
      cell: R,
    ) -> Option<AsyncBorrowImpl<T, M>> {
      let cell_ref = cell.as_ref();
      // Don't allow synchronous borrows to cut in line; if there are any
      // enqueued waiters, return `None`, even if the current borrow is a shared
      // one and the requested borrow is too.
      let waiters = unsafe { &mut *cell_ref.waiters.as_ptr() };
      if waiters.is_empty() {
        // There are no enqueued waiters, but it is still possible that the cell
        // is currently borrowed. If there are no current borrows, or both the
        // existing and requested ones are shared, `try_add()` returns the
        // adjusted borrow count.
        let new_borrow_count =
          cell_ref.borrow_count.get().try_add(M::borrow_mode())?;
        cell_ref.borrow_count.set(new_borrow_count);
        Some(AsyncBorrowImpl::<T, M>::new(cell.into()))
      } else {
        None
      }
    }

    fn drop_borrow<M: BorrowModeTrait>(&self) {
      let new_borrow_count = self.borrow_count.get().remove(M::borrow_mode());
      self.borrow_count.set(new_borrow_count);

      if new_borrow_count.is_empty() {
        self.wake_waiters()
      }
    }

    fn create_waiter<M: BorrowModeTrait>(&self) -> usize {
      let waiter = Waiter::new(M::borrow_mode());
      let turn = self.turn.get();
      let index = {
        let waiters = unsafe { &mut *self.waiters.as_ptr() };
        waiters.push_back(Some(waiter));
        waiters.len() - 1
      };
      if index == 0 {
        // SAFETY: the `waiters` reference used above *must* be dropped here.
        self.wake_waiters()
      }
      // Return the new waiter's id.
      turn + index
    }

    fn poll_waiter<M: BorrowModeTrait>(
      &self,
      id: usize,
      cx: &mut Context,
    ) -> Poll<()> {
      let borrow_count = self.borrow_count.get();
      let turn = self.turn.get();
      if id < turn {
        // This waiter made it to the front of the line; we reserved a borrow
        // for it, woke its Waker, and removed the waiter from the queue.
        // Assertion: BorrowCount::remove() will panic if `mode` is incorrect.
        let _ = borrow_count.remove(M::borrow_mode());
        Poll::Ready(())
      } else {
        // This waiter is still in line and has not yet been woken.
        let waiters = unsafe { &mut *self.waiters.as_ptr() };
        // Sanity check: id cannot be higher than the last queue element.
        assert!(id < turn + waiters.len());
        // Sanity check: since we always call wake_waiters() when the queue head
        // is updated, it should be impossible to add it to the current borrow.
        assert!(id > turn || borrow_count.try_add(M::borrow_mode()).is_none());
        // Save or update the waiter's Waker.
        // TODO(piscisaureus): Use will_wake() to make this more efficient.
        let waiter_mut = waiters[id - turn].as_mut().unwrap();
        waiter_mut.set_waker(cx.waker().clone());
        Poll::Pending
      }
    }

    fn wake_waiters(&self) {
      let mut borrow_count = self.borrow_count.get();
      let waiters = unsafe { &mut *self.waiters.as_ptr() };
      let mut turn = self.turn.get();

      loop {
        let waiter_entry = match waiters.front().map(Option::as_ref) {
          None => break, // Queue empty.
          Some(w) => w,
        };
        let borrow_mode = match waiter_entry {
          None => {
            // Queue contains a hole. This happens when a Waiter is dropped
            // before it makes it to the front of the queue.
            waiters.pop_front();
            turn += 1;
            continue;
          }
          Some(waiter) => waiter.borrow_mode(),
        };
        // See if the waiter at the front of the queue can borrow the cell's
        // value now. If it does, `try_add()` returns the new borrow count,
        // effectively "reserving" the borrow until the associated
        // AsyncBorrowFutureImpl future gets polled and produces the actual
        // borrow.
        borrow_count = match borrow_count.try_add(borrow_mode) {
          None => break, // Can't borrow yet.
          Some(b) => b,
        };
        // Drop from queue.
        let mut waiter = waiters.pop_front().unwrap().unwrap();
        turn += 1;
        // Wake this waiter, so the AsyncBorrowFutureImpl future gets polled.
        if let Some(waker) = waiter.take_waker() {
          waker.wake()
        }
      }
      // Save updated counters.
      self.borrow_count.set(borrow_count);
      self.turn.set(turn);
    }

    fn drop_waiter<M: BorrowModeTrait>(&self, id: usize) {
      let turn = self.turn.get();
      if id < turn {
        // We already made a borrow count reservation for this waiter but the
        // borrow will never be picked up and removesequently, never dropped.
        // Therefore, call the borrow drop handler here.
        self.drop_borrow::<M>();
      } else {
        // This waiter is still in the queue, take it out and leave a "hole".
        let waiters = unsafe { &mut *self.waiters.as_ptr() };
        waiters[id - turn].take().unwrap();
      }

      if id == turn {
        // Since the first entry in the waiter queue was touched we have to
        // reprocess the waiter queue.
        self.wake_waiters()
      }
    }
  }

  pub struct AsyncBorrowFutureImpl<T: 'static, M: BorrowModeTrait> {
    cell: Option<RcRef<AsyncRefCell<T>>>,
    id: usize,
    _phantom: PhantomData<M>,
  }

  impl<T, M: BorrowModeTrait> AsyncBorrowFutureImpl<T, M> {
    pub fn new<R: RcLike<AsyncRefCell<T>>>(cell: R) -> Self {
      Self {
        id: cell.as_ref().create_waiter::<M>(),
        cell: Some(cell.into()),
        _phantom: PhantomData,
      }
    }
  }

  impl<T: 'static, M: BorrowModeTrait> Future for AsyncBorrowFutureImpl<T, M> {
    type Output = AsyncBorrowImpl<T, M>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      ready!(self.cell.as_ref().unwrap().poll_waiter::<M>(self.id, cx));
      let self_mut = unsafe { Pin::get_unchecked_mut(self) };
      let cell = self_mut.cell.take().unwrap();
      Poll::Ready(AsyncBorrowImpl::<T, M>::new(cell))
    }
  }

  impl<T, M: BorrowModeTrait> Drop for AsyncBorrowFutureImpl<T, M> {
    fn drop(&mut self) {
      // The expected mode of operation is that this future gets polled until it
      // is ready and yields a value of type `AsyncBorrowImpl`, which has a drop
      // handler that adjusts the `AsyncRefCell` borrow counter. However if the
      // `cell` field still holds a value at this point, it means that the
      // future was never polled to completion and no `AsyncBorrowImpl` was ever
      // created, so we have to adjust the borrow count here.
      if let Some(cell) = self.cell.take() {
        cell.drop_waiter::<M>(self.id)
      }
    }
  }

  pub struct AsyncBorrowImpl<T: 'static, M: BorrowModeTrait> {
    cell: RcRef<AsyncRefCell<T>>,
    _phantom: PhantomData<M>,
  }

  impl<T, M: BorrowModeTrait> AsyncBorrowImpl<T, M> {
    fn new(cell: RcRef<AsyncRefCell<T>>) -> Self {
      Self {
        cell,
        _phantom: PhantomData,
      }
    }
  }

  impl<T, M: BorrowModeTrait> Deref for AsyncBorrowImpl<T, M> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
      unsafe { &*self.cell.as_ptr() }
    }
  }

  impl<T, M: BorrowModeTrait> Borrow<T> for AsyncBorrowImpl<T, M> {
    fn borrow(&self) -> &T {
      &**self
    }
  }

  impl<T, M: BorrowModeTrait> AsRef<T> for AsyncBorrowImpl<T, M> {
    fn as_ref(&self) -> &T {
      &**self
    }
  }

  impl<T> DerefMut for AsyncBorrowImpl<T, Exclusive> {
    fn deref_mut(&mut self) -> &mut Self::Target {
      unsafe { &mut *self.cell.as_ptr() }
    }
  }

  impl<T> BorrowMut<T> for AsyncBorrowImpl<T, Exclusive> {
    fn borrow_mut(&mut self) -> &mut T {
      &mut **self
    }
  }

  impl<T> AsMut<T> for AsyncBorrowImpl<T, Exclusive> {
    fn as_mut(&mut self) -> &mut T {
      &mut **self
    }
  }

  impl<T, M: BorrowModeTrait> Drop for AsyncBorrowImpl<T, M> {
    fn drop(&mut self) {
      self.cell.drop_borrow::<M>()
    }
  }

  #[derive(Copy, Clone, Debug, Eq, PartialEq)]
  pub enum BorrowMode {
    Shared,
    Exclusive,
  }

  pub trait BorrowModeTrait: Copy {
    fn borrow_mode() -> BorrowMode;
  }

  #[derive(Copy, Clone, Debug)]
  pub struct Shared;

  impl BorrowModeTrait for Shared {
    fn borrow_mode() -> BorrowMode {
      BorrowMode::Shared
    }
  }

  #[derive(Copy, Clone, Debug)]
  pub struct Exclusive;

  impl BorrowModeTrait for Exclusive {
    fn borrow_mode() -> BorrowMode {
      BorrowMode::Exclusive
    }
  }

  #[derive(Copy, Clone, Debug, Eq, PartialEq)]
  pub enum BorrowCount {
    Shared(usize),
    Exclusive,
  }

  impl Default for BorrowCount {
    fn default() -> Self {
      Self::Shared(0)
    }
  }

  impl BorrowCount {
    pub fn is_empty(self) -> bool {
      matches!(self, BorrowCount::Shared(0))
    }

    pub fn try_add(self, mode: BorrowMode) -> Option<BorrowCount> {
      match (self, mode) {
        (BorrowCount::Shared(refs), BorrowMode::Shared) => {
          Some(BorrowCount::Shared(refs + 1))
        }
        (BorrowCount::Shared(0), BorrowMode::Exclusive) => {
          Some(BorrowCount::Exclusive)
        }
        _ => None,
      }
    }

    #[allow(dead_code)]
    pub fn add(self, mode: BorrowMode) -> BorrowCount {
      match self.try_add(mode) {
        Some(value) => value,
        None => panic!("Can't add {:?} to {:?}", mode, self),
      }
    }

    pub fn try_remove(self, mode: BorrowMode) -> Option<BorrowCount> {
      match (self, mode) {
        (BorrowCount::Shared(refs), BorrowMode::Shared) if refs > 0 => {
          Some(BorrowCount::Shared(refs - 1))
        }
        (BorrowCount::Exclusive, BorrowMode::Exclusive) => {
          Some(BorrowCount::Shared(0))
        }
        _ => None,
      }
    }

    pub fn remove(self, mode: BorrowMode) -> BorrowCount {
      match self.try_remove(mode) {
        Some(value) => value,
        None => panic!("Can't remove {:?} from {:?}", mode, self),
      }
    }
  }

  /// The `waiters` queue that is associated with an individual `AsyncRefCell`
  /// contains elements of the `Waiter` type.
  pub struct Waiter {
    borrow_mode: BorrowMode,
    waker: Option<Waker>,
  }

  impl Waiter {
    pub fn new(borrow_mode: BorrowMode) -> Self {
      Self {
        borrow_mode,
        waker: None,
      }
    }

    pub fn borrow_mode(&self) -> BorrowMode {
      self.borrow_mode
    }

    pub fn set_waker(&mut self, waker: Waker) {
      self.waker.replace(waker);
    }

    pub fn take_waker(&mut self) -> Option<Waker> {
      self.waker.take()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Default)]
  struct Thing {
    touch_count: usize,
    _private: (),
  }

  impl Thing {
    pub fn look(&self) -> usize {
      self.touch_count
    }

    pub fn touch(&mut self) -> usize {
      self.touch_count += 1;
      self.touch_count
    }
  }

  #[tokio::test]
  async fn async_ref_cell_borrow() {
    let cell = AsyncRefCell::<Thing>::default_rc();

    let fut1 = cell.borrow();
    let fut2 = cell.borrow_mut();
    let fut3 = cell.borrow();
    let fut4 = cell.borrow();
    let fut5 = cell.borrow();
    let fut6 = cell.borrow();
    let fut7 = cell.borrow_mut();
    let fut8 = cell.borrow();

    // The `try_borrow` and `try_borrow_mut` methods should always return `None`
    // if there's a queue of async borrowers.
    assert!(cell.try_borrow().is_none());
    assert!(cell.try_borrow_mut().is_none());

    assert_eq!(fut1.await.look(), 0);

    assert_eq!(fut2.await.touch(), 1);

    {
      let ref5 = fut5.await;
      let ref4 = fut4.await;
      let ref3 = fut3.await;
      let ref6 = fut6.await;
      assert_eq!(ref3.look(), 1);
      assert_eq!(ref4.look(), 1);
      assert_eq!(ref5.look(), 1);
      assert_eq!(ref6.look(), 1);
    }

    {
      let mut ref7 = fut7.await;
      assert_eq!(ref7.look(), 1);
      assert_eq!(ref7.touch(), 2);
    }

    {
      let ref8 = fut8.await;
      assert_eq!(ref8.look(), 2);
    }
  }

  #[test]
  fn async_ref_cell_try_borrow() {
    let cell = AsyncRefCell::<Thing>::default_rc();

    {
      let ref1 = cell.try_borrow().unwrap();
      assert_eq!(ref1.look(), 0);
      assert!(cell.try_borrow_mut().is_none());
    }

    {
      let mut ref2 = cell.try_borrow_mut().unwrap();
      assert_eq!(ref2.touch(), 1);
      assert!(cell.try_borrow().is_none());
      assert!(cell.try_borrow_mut().is_none());
    }

    {
      let ref3 = cell.try_borrow().unwrap();
      let ref4 = cell.try_borrow().unwrap();
      let ref5 = cell.try_borrow().unwrap();
      let ref6 = cell.try_borrow().unwrap();
      assert_eq!(ref3.look(), 1);
      assert_eq!(ref4.look(), 1);
      assert_eq!(ref5.look(), 1);
      assert_eq!(ref6.look(), 1);
      assert!(cell.try_borrow_mut().is_none());
    }

    {
      let mut ref7 = cell.try_borrow_mut().unwrap();
      assert_eq!(ref7.look(), 1);
      assert_eq!(ref7.touch(), 2);
      assert!(cell.try_borrow().is_none());
      assert!(cell.try_borrow_mut().is_none());
    }

    {
      let ref8 = cell.try_borrow().unwrap();
      assert_eq!(ref8.look(), 2);
      assert!(cell.try_borrow_mut().is_none());
      assert!(cell.try_borrow().is_some());
    }
  }

  #[derive(Default)]
  struct ThreeThings {
    pub thing1: AsyncRefCell<Thing>,
    pub thing2: AsyncRefCell<Thing>,
    pub thing3: AsyncRefCell<Thing>,
  }

  #[tokio::test]
  async fn rc_ref_map() {
    let three_cells = Rc::new(ThreeThings::default());

    let rc1 = RcRef::map(three_cells.clone(), |things| &things.thing1);
    let rc2 = RcRef::map(three_cells.clone(), |things| &things.thing2);
    let rc3 = RcRef::map(three_cells, |things| &things.thing3);

    let mut ref1 = rc1.borrow_mut().await;
    let ref2 = rc2.borrow().await;
    let mut ref3 = rc3.borrow_mut().await;

    assert_eq!(ref1.look(), 0);
    assert_eq!(ref3.touch(), 1);
    assert_eq!(ref1.touch(), 1);
    assert_eq!(ref2.look(), 0);
    assert_eq!(ref3.touch(), 2);
    assert_eq!(ref1.look(), 1);
    assert_eq!(ref1.touch(), 2);
    assert_eq!(ref3.touch(), 3);
    assert_eq!(ref1.touch(), 3);
  }
}
