// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::time::Duration;

use cooked_waker::IntoWaker;
use cooked_waker::ViaRawPointer;
use cooked_waker::Wake;
use cooked_waker::WakeRef;

use crate::reactor::Reactor;
use crate::reactor::ReactorInstant;
use crate::reactor::ReactorTimer;

/// Like a Box<T> but without Uniqueness semantics, which
/// cause issues with self-referential waker pointers under Stacked Borrows.
#[repr(transparent)]
struct OwnedPtr<T> {
  ptr: *mut T,
}

impl<T> OwnedPtr<T> {
  fn from_box(b: Box<T>) -> Self {
    Self {
      ptr: Box::into_raw(b),
    }
  }
}

impl<T> Deref for OwnedPtr<T> {
  type Target = T;
  fn deref(&self) -> &T {
    unsafe { &*self.ptr }
  }
}

impl<T> std::ops::DerefMut for OwnedPtr<T> {
  fn deref_mut(&mut self) -> &mut T {
    unsafe { &mut *self.ptr }
  }
}

impl<T> Drop for OwnedPtr<T> {
  fn drop(&mut self) {
    unsafe {
      let _ = Box::from_raw(self.ptr);
    }
  }
}

struct MutableSleep<Tmr: ReactorTimer> {
  sleep: UnsafeCell<Option<Tmr>>,
  ready: Cell<bool>,
  external_waker: UnsafeCell<Option<Waker>>,
  internal_waker: Waker,
}

impl<Tmr: ReactorTimer + 'static> MutableSleep<Tmr> {
  fn new() -> OwnedPtr<Self> {
    unsafe {
      let mut ptr = OwnedPtr::from_box(Box::new(MaybeUninit::<Self>::uninit()));
      let raw = ptr.as_ptr();
      ptr.write(MutableSleep {
        sleep: Default::default(),
        ready: Default::default(),
        external_waker: Default::default(),
        internal_waker: MutableSleepWaker::<Tmr> { inner: raw }.into_waker(),
      });
      std::mem::transmute(ptr)
    }
  }

  fn poll_ready(&self, cx: &mut Context) -> Poll<()> {
    if self.ready.take() {
      Poll::Ready(())
    } else {
      let external =
        unsafe { self.external_waker.get().as_mut().unwrap_unchecked() };
      if let Some(external) = external {
        // Already have this waker
        let waker = cx.waker();
        if !external.will_wake(waker) {
          external.clone_from(waker);
        }

        // We do a manual deadline check here. The timer wheel may not immediately check the deadline if the
        // executor was blocked.
        // Skip this check under Miri as it interferes with time simulation.
        #[cfg(not(miri))]
        {
          let sleep = unsafe { self.sleep.get().as_mut().unwrap_unchecked() };
          if let Some(sleep) = sleep
            && Tmr::Instant::now() >= sleep.deadline()
          {
            return Poll::Ready(());
          }
        }
        Poll::Pending
      } else {
        *external = Some(cx.waker().clone());
        Poll::Pending
      }
    }
  }

  fn clear(&self) {
    unsafe {
      *self.sleep.get() = None;
    }
    self.ready.set(false);
  }

  fn change(&self, timer: Tmr) {
    let pin = unsafe {
      // First replace the current timer
      *self.sleep.get() = Some(timer);

      // Then get ourselves a Pin to this
      Pin::new_unchecked(
        self
          .sleep
          .get()
          .as_mut()
          .unwrap_unchecked()
          .as_mut()
          .unwrap_unchecked(),
      )
    };

    // Register our waker
    let waker = &self.internal_waker;
    if pin.poll(&mut Context::from_waker(waker)).is_ready() {
      self.ready.set(true);
      self.internal_waker.wake_by_ref();
    }
  }
}

#[repr(transparent)]
struct MutableSleepWaker<Tmr: ReactorTimer> {
  inner: *const MutableSleep<Tmr>,
}

impl<Tmr: ReactorTimer> Clone for MutableSleepWaker<Tmr> {
  fn clone(&self) -> Self {
    MutableSleepWaker { inner: self.inner }
  }
}

unsafe impl<Tmr: ReactorTimer> Send for MutableSleepWaker<Tmr> {}
unsafe impl<Tmr: ReactorTimer> Sync for MutableSleepWaker<Tmr> {}

impl<Tmr: ReactorTimer> WakeRef for MutableSleepWaker<Tmr> {
  fn wake_by_ref(&self) {
    unsafe {
      let this = self.inner.as_ref().unwrap_unchecked();
      this.ready.set(true);
      let waker = this.external_waker.get().as_mut().unwrap_unchecked();
      if let Some(waker) = waker.as_ref() {
        waker.wake_by_ref();
      }
    }
  }
}

impl<Tmr: ReactorTimer> Wake for MutableSleepWaker<Tmr> {
  fn wake(self) {
    self.wake_by_ref()
  }
}

impl<Tmr: ReactorTimer> Drop for MutableSleepWaker<Tmr> {
  fn drop(&mut self) {}
}

unsafe impl<Tmr: ReactorTimer> ViaRawPointer for MutableSleepWaker<Tmr> {
  type Target = ();

  fn into_raw(self) -> *mut () {
    self.inner as _
  }

  unsafe fn from_raw(ptr: *mut ()) -> Self {
    MutableSleepWaker { inner: ptr as _ }
  }
}

/// A single-deadline timer for JS-managed user timers.
///
/// `UserTimer` is a simple "wake me at time T" mechanism. The JS side
/// manages timer bucketing, linked lists, and priority queues (matching
/// Node.js's architecture). Rust just needs to know when to wake up.
pub(crate) struct UserTimer<R: Reactor> {
  reactor: R,
  sleep: OwnedPtr<MutableSleep<R::Timer>>,
  base_instant: R::Instant,
  /// Whether the timer handle is "ref'd" (keeps event loop alive).
  refed: Cell<bool>,
}

impl<R: Reactor + Default> Default for UserTimer<R> {
  fn default() -> Self {
    Self::new(R::default())
  }
}

impl<R: Reactor> UserTimer<R> {
  pub fn new(reactor: R) -> Self {
    Self {
      base_instant: reactor.now(),
      sleep: MutableSleep::new(),
      reactor,
      refed: Cell::new(false),
    }
  }

  /// Schedule a wakeup after `delay` from now.
  pub fn schedule(&self, delay: Duration) {
    let deadline = self.reactor.now().checked_add(delay).unwrap();
    self.sleep.change(self.reactor.timer(deadline));
  }

  /// Cancel any pending wakeup.
  pub fn clear(&self) {
    self.sleep.clear();
  }

  /// Poll for the scheduled wakeup.
  pub fn poll_ready(&self, cx: &mut Context) -> Poll<()> {
    self.sleep.poll_ready(cx)
  }

  /// Get the current monotonic time in milliseconds since this timer
  /// was created (process start).
  pub fn now(&self) -> f64 {
    self.base_instant.elapsed().as_secs_f64() * 1000.0
  }

  /// Mark the timer handle as ref'd (keeps event loop alive).
  pub fn ref_timer(&self) {
    self.refed.set(true);
  }

  /// Mark the timer handle as unref'd (allows event loop to exit).
  pub fn unref_timer(&self) {
    self.refed.set(false);
  }

  /// Whether the timer handle is ref'd.
  pub fn is_refed(&self) -> bool {
    self.refed.get()
  }
}
