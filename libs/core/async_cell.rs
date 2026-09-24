// Copyright 2018-2026 the Deno authors. MIT license.

use std::any::Any;
use std::any::type_name;
use std::borrow::Borrow;
use std::cell::Cell;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::ops::Deref;
use std::rc::Rc;

use self::internal as i;

pub type AsyncRef<T> = i::AsyncBorrowImpl<T, i::Shared>;
pub type AsyncMut<T> = i::AsyncBorrowImpl<T, i::Exclusive>;

pub type AsyncRefFuture<T> = i::FastAsyncBorrowFuture<T, i::Shared>;
pub type AsyncMutFuture<T> = i::FastAsyncBorrowFuture<T, i::Exclusive>;

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

  pub fn into_inner(self) -> T {
    assert!(self.borrow_count.get().is_empty());
    self.value.into_inner()
  }
}

impl<T> Debug for AsyncRefCell<T> {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "AsyncRefCell<{}>", type_name::<T>())
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
/// # use deno_core::RcRef;
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
    // TODO(piscisaureus): safety comment
    #[allow(
      clippy::undocumented_unsafe_blocks,
      reason = "safety comment on the containing block"
    )]
    let value = map_fn(unsafe { &*value });
    RcRef { rc, value }
  }

  pub(crate) fn split(rc_ref: &Self) -> (&T, &Rc<dyn Any>) {
    let &Self { ref rc, value } = rc_ref;
    // TODO(piscisaureus): safety comment
    #[allow(
      clippy::undocumented_unsafe_blocks,
      reason = "safety comment on the containing block"
    )]
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
    // TODO(piscisaureus): safety comment
    #[allow(
      clippy::undocumented_unsafe_blocks,
      reason = "safety comment on the containing block"
    )]
    unsafe {
      &*self.value
    }
  }
}

impl<T> Borrow<T> for RcRef<T> {
  fn borrow(&self) -> &T {
    self
  }
}

impl<T> AsRef<T> for RcRef<T> {
  fn as_ref(&self) -> &T {
    self
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
  use std::borrow::Borrow;
  use std::borrow::BorrowMut;
  use std::fmt::Debug;
  use std::future::Future;
  use std::marker::PhantomData;
  use std::ops::Deref;
  use std::ops::DerefMut;
  use std::pin::Pin;
  use std::task::Context;
  use std::task::Poll;
  use std::task::Waker;
  use std::task::ready;

  use super::AsyncRefCell;
  use super::RcLike;
  use super::RcRef;

  impl<T> AsyncRefCell<T> {
    /// Borrow the cell's contents synchronously without creating an
    /// intermediate future. If the cell has already been borrowed and either
    /// the existing or the requested borrow is exclusive, this function returns
    /// `None`.
    pub fn borrow_sync<M: BorrowModeTrait, R: RcLike<AsyncRefCell<T>>>(
      cell: R,
    ) -> Option<AsyncBorrowImpl<T, M>> {
      if cell.as_ref().try_take_borrow::<M>() {
        Some(AsyncBorrowImpl::<T, M>::new(cell.into()))
      } else {
        None
      }
    }

    /// The admission test for taking a borrow *without* going through the
    /// waiter queue. Returns `true` — having already charged the borrow to
    /// `borrow_count` — if and only if the borrow can be granted right now.
    ///
    /// This is the single definition of "uncontended". Both [`Self::borrow_sync`]
    /// and the fast path in [`FastAsyncBorrowFuture::new`] go through it, so
    /// they cannot drift apart. It is also exactly the condition under which
    /// `create_waiter()` + `wake_waiters()` would have reserved the borrow
    /// immediately anyway (queue empty => the new waiter lands at index 0 =>
    /// `wake_waiters()` runs and admits it iff `try_add()` succeeds).
    ///
    /// A `true` return transfers ownership of one borrow to the caller, which
    /// *must* hand it to an `AsyncBorrowImpl` (whose `Drop` releases it).
    // `#[inline]` alone is not enough here: LLVM declines to inline this into
    // `borrow_sync()` and the exclusive `try_borrow_mut()` measurably
    // regresses (8.8 -> 10.7 ns) versus having the body written inline.
    #[inline(always)]
    fn try_take_borrow<M: BorrowModeTrait>(&self) -> bool {
      // Don't allow borrows to cut in line; if there are any enqueued waiters,
      // fail, even if the current borrow is a shared one and the requested
      // borrow is too.
      // TODO(piscisaureus): safety comment
      #[allow(
        clippy::undocumented_unsafe_blocks,
        reason = "safety comment on the containing block"
      )]
      let waiters = unsafe { &mut *self.waiters.as_ptr() };
      if !waiters.is_empty() {
        return false;
      }
      // There are no enqueued waiters, but it is still possible that the cell
      // is currently borrowed. If there are no current borrows, or both the
      // existing and requested ones are shared, `try_add()` returns the
      // adjusted borrow count.
      match self.borrow_count.get().try_add(M::borrow_mode()) {
        Some(new_borrow_count) => {
          self.borrow_count.set(new_borrow_count);
          true
        }
        None => false,
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
        // TODO(piscisaureus): safety comment
        #[allow(
          clippy::undocumented_unsafe_blocks,
          reason = "safety comment on the containing block"
        )]
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
        // TODO(piscisaureus): safety comment
        #[allow(
          clippy::undocumented_unsafe_blocks,
          reason = "safety comment on the containing block"
        )]
        let waiters = unsafe { &mut *self.waiters.as_ptr() };
        // Sanity check: id cannot be higher than the last queue element.
        assert!(id < turn + waiters.len());
        // Sanity check: since we always call wake_waiters() when the queue head
        // is updated, it should be impossible to add it to the current borrow.
        assert!(id > turn || borrow_count.try_add(M::borrow_mode()).is_none());
        // Save or update the waiter's Waker.
        let waiter_mut = waiters[id - turn].as_mut().unwrap();
        waiter_mut.set_waker(cx.waker());
        Poll::Pending
      }
    }

    fn wake_waiters(&self) {
      let mut borrow_count = self.borrow_count.get();
      // TODO(piscisaureus): safety comment
      #[allow(
        clippy::undocumented_unsafe_blocks,
        reason = "safety comment on the containing block"
      )]
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
        // borrow will never be picked up and consequently, never dropped.
        // Therefore, call the borrow drop handler here.
        self.drop_borrow::<M>();
      } else {
        // This waiter is still in the queue, take it out and leave a "hole".
        // TODO(piscisaureus): safety comment
        #[allow(
          clippy::undocumented_unsafe_blocks,
          reason = "safety comment on the containing block"
        )]
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

  /// The future returned by `AsyncRefCell::borrow{,_mut}()`.
  ///
  /// Splits the uncontended case out of the waiter queue. When the cell can be
  /// borrowed at the moment the future is constructed, the borrow is taken
  /// right there and the future is trivially `Ready`; otherwise this is just a
  /// thin wrapper around [`AsyncBorrowFutureImpl`], unchanged.
  ///
  /// # Why taking the borrow at construction is not a semantic change
  ///
  /// It looks like one — the borrow moves from "granted at first poll" to
  /// "granted at construction" — but the queueing path already behaved that
  /// way. `AsyncBorrowFutureImpl::new()` calls `create_waiter()`, which pushes
  /// the waiter and, *if it landed at index 0*, immediately runs
  /// `wake_waiters()`. `wake_waiters()` charges the borrow to `borrow_count`
  /// ("reserving" it) before any poll happens. So a borrow future built when
  /// the queue was empty and the count admitted the mode has always held the
  /// borrow from construction; `poll()` only converted the reservation into an
  /// `AsyncBorrowImpl`.
  ///
  /// [`AsyncRefCell::try_take_borrow`] is exactly the condition under which
  /// that reservation would have happened, and it is the same condition the
  /// long-public `try_borrow{,_mut}()` uses. When it fails we fall through to
  /// the queue, so a contended borrow is enqueued in FIFO order exactly as
  /// before. This matters for code that reserves a place in line without
  /// awaiting (`ext/websocket`'s `reserve_lock()`): a second reservation on an
  /// already-borrowed cell still queues rather than being granted.
  pub enum FastAsyncBorrowFuture<T: 'static, M: BorrowModeTrait> {
    /// The borrow was uncontended and has already been taken. The `Option` is
    /// emptied by `poll()`; if the future is dropped before it is ever polled,
    /// the `AsyncBorrowImpl` drops with it and releases the borrow.
    Ready(Option<AsyncBorrowImpl<T, M>>),
    /// The borrow was contended; wait in line.
    Queued(AsyncBorrowFutureImpl<T, M>),
  }

  impl<T, M: BorrowModeTrait> FastAsyncBorrowFuture<T, M> {
    #[inline]
    pub fn new<R: RcLike<AsyncRefCell<T>>>(cell: R) -> Self {
      if cell.as_ref().try_take_borrow::<M>() {
        // `try_take_borrow()` charged the borrow to the cell; handing it to an
        // `AsyncBorrowImpl` makes its `Drop` responsible for releasing it.
        Self::Ready(Some(AsyncBorrowImpl::<T, M>::new(cell.into())))
      } else {
        Self::Queued(AsyncBorrowFutureImpl::new(cell))
      }
    }
  }

  impl<T: 'static, M: BorrowModeTrait> Future for FastAsyncBorrowFuture<T, M> {
    type Output = AsyncBorrowImpl<T, M>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      // Both variants are `Unpin` (every field is a pointer, an index or a
      // `PhantomData`), so this needs no `unsafe` and nothing is structurally
      // pinned.
      match self.get_mut() {
        // Panics on a second poll, matching `AsyncBorrowFutureImpl`, which
        // unwraps its already-taken `cell` field. Polling a future after it
        // returned `Ready` is a contract violation either way.
        Self::Ready(borrow) => {
          Poll::Ready(borrow.take().expect("polled after completion"))
        }
        Self::Queued(fut) => Pin::new(fut).poll(cx),
      }
    }
  }

  impl<T: 'static, M: BorrowModeTrait> Future for AsyncBorrowFutureImpl<T, M> {
    type Output = AsyncBorrowImpl<T, M>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      ready!(self.cell.as_ref().unwrap().poll_waiter::<M>(self.id, cx));
      // TODO(piscisaureus): safety comment
      #[allow(
        clippy::undocumented_unsafe_blocks,
        reason = "safety comment on the containing block"
      )]
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
      // TODO(piscisaureus): safety comment
      #[allow(
        clippy::undocumented_unsafe_blocks,
        reason = "safety comment on the containing block"
      )]
      unsafe {
        &*self.cell.as_ptr()
      }
    }
  }

  impl<T, M: BorrowModeTrait> Borrow<T> for AsyncBorrowImpl<T, M> {
    fn borrow(&self) -> &T {
      self
    }
  }

  impl<T, M: BorrowModeTrait> AsRef<T> for AsyncBorrowImpl<T, M> {
    fn as_ref(&self) -> &T {
      self
    }
  }

  impl<T> DerefMut for AsyncBorrowImpl<T, Exclusive> {
    fn deref_mut(&mut self) -> &mut Self::Target {
      // TODO(piscisaureus): safety comment
      #[allow(
        clippy::undocumented_unsafe_blocks,
        reason = "safety comment on the containing block"
      )]
      unsafe {
        &mut *self.cell.as_ptr()
      }
    }
  }

  impl<T> BorrowMut<T> for AsyncBorrowImpl<T, Exclusive> {
    fn borrow_mut(&mut self) -> &mut T {
      self
    }
  }

  impl<T> AsMut<T> for AsyncBorrowImpl<T, Exclusive> {
    fn as_mut(&mut self) -> &mut T {
      self
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

  /// Borrow modes are zero-sized markers used only through `PhantomData`, so
  /// requiring `Unpin` costs nothing and lets the futures parameterized by them
  /// be `Unpin` too — which is what lets `FastAsyncBorrowFuture::poll()` avoid
  /// `unsafe` pin projection.
  pub trait BorrowModeTrait: Copy + Unpin {
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

    #[allow(dead_code, reason = "intentionally unused")]
    pub fn add(self, mode: BorrowMode) -> BorrowCount {
      match self.try_add(mode) {
        Some(value) => value,
        None => panic!("Can't add {mode:?} to {self:?}"),
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
        None => panic!("Can't remove {mode:?} from {self:?}"),
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

    pub fn set_waker(&mut self, new_waker: &Waker) {
      if self
        .waker
        .as_ref()
        .filter(|waker| waker.will_wake(new_waker))
        .is_none()
      {
        self.waker.replace(new_waker.clone());
      }
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

  #[test]
  fn async_ref_cell_borrow() {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();
    runtime.block_on(async {
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
    });
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

  // ---------------------------------------------------------------------
  // Fast-path (borrow granted at construction) regression tests.
  //
  // `AsyncRefCell::borrow{,_mut}()` take the borrow when the future is
  // *constructed* if the cell is uncontended at that moment, instead of at the
  // first poll. These tests pin down the consequences.
  // ---------------------------------------------------------------------

  fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap()
      .block_on(fut)
  }

  /// A waker that counts how many times it was woken, so tests can assert the
  /// fast path never needs one.
  #[derive(Default)]
  struct WakerProbe {
    woken: std::sync::atomic::AtomicUsize,
  }

  impl std::task::Wake for WakerProbe {
    fn wake(self: std::sync::Arc<Self>) {
      self.woken.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn wake_by_ref(self: &std::sync::Arc<Self>) {
      self.woken.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
  }

  /// Risk 1: a borrow future that is constructed and then dropped without ever
  /// being polled must release its borrow — exactly once, not zero times (leak,
  /// the cell would be permanently locked) and not twice (panic in
  /// `BorrowCount::remove`).
  #[test]
  fn fast_path_drop_without_poll_releases_borrow() {
    let cell = AsyncRefCell::<Thing>::default_rc();

    // Exclusive: construct, never poll, drop.
    for _ in 0..3 {
      let fut = cell.borrow_mut();
      // The borrow is held right now, so the cell is unavailable...
      assert!(cell.try_borrow().is_none());
      assert!(cell.try_borrow_mut().is_none());
      drop(fut);
      // ...and available again immediately after the drop.
      assert!(cell.try_borrow_mut().is_some());
    }

    // Shared: two concurrent constructed-but-unpolled shared borrows.
    let s1 = cell.borrow();
    let s2 = cell.borrow();
    assert!(cell.try_borrow_mut().is_none());
    assert!(cell.try_borrow().is_some(), "shared borrows should coexist");
    drop(s1);
    assert!(cell.try_borrow_mut().is_none(), "s2 still holds it");
    drop(s2);
    assert!(cell.try_borrow_mut().is_some());

    // And the cell is still fully functional afterwards.
    block_on(async {
      assert_eq!(cell.borrow_mut().await.touch(), 1);
    });
  }

  /// Risk 1b: a *queued* (contended) future dropped without a poll must also
  /// release its queue slot, and must not release a borrow it never got.
  #[test]
  fn fast_path_drop_queued_without_poll() {
    let cell = AsyncRefCell::<Thing>::default_rc();

    let held = cell.borrow_mut(); // fast path: borrow taken now
    let queued = cell.borrow_mut(); // contended: enqueued
    let queued2 = cell.borrow_mut(); // also enqueued

    drop(queued); // leaves a hole in the queue
    drop(held); // releases the borrow, wakes the queue
    // `queued2` inherited the borrow via wake_waiters(); the cell is busy.
    assert!(cell.try_borrow_mut().is_none());
    drop(queued2);
    assert!(cell.try_borrow_mut().is_some());
  }

  /// Risk 2: construct two borrow futures on the SAME cell before awaiting
  /// either, then await them in each order. The second construction must queue
  /// rather than be granted, and both orders must behave the way they did when
  /// the borrow was granted lazily.
  ///
  /// The `ext/websocket` `reserve_lock()` pattern is exactly this: reserve a
  /// place in line synchronously inside an op, then `.await` it on a spawned
  /// task.
  #[test]
  fn fast_path_construct_two_then_await_in_order() {
    block_on(async {
      let cell = AsyncRefCell::<Thing>::default_rc();
      let fut1 = cell.borrow_mut();
      let fut2 = cell.borrow_mut();
      // Constructing `fut2` must not have granted it anything.
      assert!(cell.try_borrow_mut().is_none());

      {
        let mut r1 = fut1.await;
        assert_eq!(r1.touch(), 1);
      }
      {
        let mut r2 = fut2.await;
        assert_eq!(r2.touch(), 2);
      }
    });
  }

  /// Same construction, awaited in the *reverse* order. `fut2` cannot complete
  /// until `fut1` releases, and `fut1` holds its borrow from construction, so
  /// awaiting `fut2` first deadlocks. That is not a regression: with a lazily
  /// granted borrow it deadlocked identically, because `create_waiter()` runs
  /// `wake_waiters()` at construction and reserves the borrow for `fut1` there.
  ///
  /// The test asserts the deadlock is a *pend*, not a panic or a spurious
  /// grant, and that dropping `fut1` unblocks `fut2`.
  #[test]
  fn fast_path_construct_two_then_await_reversed() {
    block_on(async {
      let cell = AsyncRefCell::<Thing>::default_rc();
      let fut1 = cell.borrow_mut();
      let fut2 = cell.borrow_mut();

      let probe = std::sync::Arc::new(WakerProbe::default());
      let waker = std::task::Waker::from(probe.clone());
      let mut cx = std::task::Context::from_waker(&waker);
      let mut fut2 = Box::pin(fut2);

      assert!(
        fut2.as_mut().poll(&mut cx).is_pending(),
        "fut2 must not be granted a borrow fut1 is holding"
      );
      assert_eq!(probe.woken.load(std::sync::atomic::Ordering::SeqCst), 0);

      // Releasing fut1 without ever polling it hands the borrow to fut2.
      drop(fut1);
      assert_eq!(
        probe.woken.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "dropping fut1 must wake the queued waiter"
      );
      let mut r2 = fut2.await;
      assert_eq!(r2.touch(), 1);
    });
  }

  /// Risk 2c: mixed shared/exclusive interleaving across the fast/queued
  /// boundary keeps FIFO order.
  #[test]
  fn fast_path_mixed_modes_keep_fifo() {
    block_on(async {
      let cell = AsyncRefCell::<Thing>::default_rc();
      let shared1 = cell.borrow(); // fast: Shared(1)
      let shared2 = cell.borrow(); // fast: Shared(2), still uncontended
      let excl = cell.borrow_mut(); // queued behind the shared borrows
      let shared3 = cell.borrow(); // queued behind `excl`, NOT granted early

      // `shared3` must not cut in line ahead of `excl` even though a shared
      // borrow is currently active and would admit it.
      assert!(cell.try_borrow().is_none(), "queue is non-empty");

      assert_eq!(shared1.await.look(), 0);
      assert_eq!(shared2.await.look(), 0);
      let mut e = excl.await;
      assert_eq!(e.touch(), 1);
      drop(e);
      assert_eq!(shared3.await.look(), 1);
    });
  }

  /// Risk 3: a fast-granted borrow wrapped in `try_or_cancel()` and cancelled
  /// before it is ever polled must still release the borrow when the wrapper is
  /// dropped.
  #[test]
  fn fast_path_cancelled_before_poll_releases_borrow() {
    use crate::CancelFuture;
    use crate::CancelHandle;

    block_on(async {
      let cell = AsyncRefCell::<Thing>::default_rc();
      let handle = CancelHandle::new_rc();

      {
        let guarded = cell.borrow_mut().or_cancel(&handle);
        assert!(cell.try_borrow_mut().is_none(), "borrow is held");
        handle.cancel();
        // Never polled; dropped while cancelled.
        drop(guarded);
      }
      assert!(
        cell.try_borrow_mut().is_some(),
        "cancelling before the first poll must not leak the borrow"
      );

      // And cancelling a borrow that *is* polled still yields Canceled without
      // leaking.
      let handle2 = CancelHandle::new_rc();
      handle2.cancel();
      let res = cell.borrow_mut().or_cancel(&handle2).await;
      assert!(
        res.is_err(),
        "already-cancelled handle should short-circuit"
      );
      assert!(cell.try_borrow_mut().is_some());
    });
  }

  /// Risk 4: the fast path must complete on its first poll without ever
  /// touching the `Waker` — no clone, no wake. Anything that requires a waker
  /// before the first poll would be a real semantic change.
  #[test]
  fn fast_path_needs_no_waker() {
    let cell = AsyncRefCell::<Thing>::default_rc();
    let probe = std::sync::Arc::new(WakerProbe::default());
    let waker = std::task::Waker::from(probe.clone());
    let mut cx = std::task::Context::from_waker(&waker);

    let mut fut = Box::pin(cell.borrow_mut());
    match fut.as_mut().poll(&mut cx) {
      std::task::Poll::Ready(mut b) => assert_eq!(b.touch(), 1),
      std::task::Poll::Pending => {
        panic!("uncontended borrow must be ready on first poll")
      }
    }
    assert_eq!(
      probe.woken.load(std::sync::atomic::Ordering::SeqCst),
      0,
      "fast path must not wake anything"
    );
  }

  /// The fast path must be *entered* when uncontended and *skipped* when not.
  /// Asserted through the observable proxy `try_borrow()`: the queued path
  /// leaves a waiter behind, which makes `try_borrow()` fail even for a mode
  /// the borrow count would admit.
  #[test]
  fn fast_path_engages_only_when_uncontended() {
    let cell = AsyncRefCell::<Thing>::default_rc();

    // Uncontended shared borrow => fast path => queue stays empty => another
    // shared `try_borrow()` succeeds.
    let fast = cell.borrow();
    assert!(
      cell.try_borrow().is_some(),
      "fast path must not enqueue a waiter"
    );

    // Exclusive borrow while a shared one is out => queued => the queue is now
    // non-empty, so even a shared `try_borrow()` must fail.
    let queued = cell.borrow_mut();
    assert!(
      cell.try_borrow().is_none(),
      "queued path must enqueue a waiter"
    );
    drop(queued);
    assert!(cell.try_borrow().is_some(), "queue drained again");
    drop(fast);
  }

  /// Polling the fast path a second time is a contract violation and panics,
  /// matching what the queued path does.
  #[test]
  #[should_panic(expected = "polled after completion")]
  fn fast_path_double_poll_panics() {
    let cell = AsyncRefCell::<Thing>::default_rc();
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut fut = Box::pin(cell.borrow_mut());
    assert!(fut.as_mut().poll(&mut cx).is_ready());
    let _ = fut.as_mut().poll(&mut cx);
  }

  #[derive(Default)]
  struct ThreeThings {
    pub thing1: AsyncRefCell<Thing>,
    pub thing2: AsyncRefCell<Thing>,
    pub thing3: AsyncRefCell<Thing>,
  }

  #[test]
  fn rc_ref_map() {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();
    runtime.block_on(async {
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
    });
  }
}
