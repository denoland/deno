// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::time::Duration;

use futures::task::AtomicWaker;

use crate::reactor::Reactor;
use crate::reactor::ReactorInstant;
use crate::reactor::ReactorTimer;

struct MutableSleep<Tmr: ReactorTimer> {
  sleep: UnsafeCell<Option<Tmr>>,
  wake_state: Arc<MutableSleepWaker>,
  internal_waker: Waker,
}

impl<Tmr: ReactorTimer + 'static> MutableSleep<Tmr> {
  fn new() -> Self {
    let wake_state = Arc::new(MutableSleepWaker {
      ready: AtomicBool::new(false),
      external_waker: AtomicWaker::new(),
    });
    Self {
      sleep: Default::default(),
      internal_waker: Waker::from(Arc::clone(&wake_state)),
      wake_state,
    }
  }

  fn poll_ready(&self, cx: &mut Context) -> Poll<()> {
    if self.wake_state.ready.swap(false, Ordering::AcqRel) {
      return Poll::Ready(());
    }

    self.wake_state.external_waker.register(cx.waker());

    // Check again after registering so a wake racing with registration cannot
    // be lost.
    if self.wake_state.ready.swap(false, Ordering::AcqRel) {
      return Poll::Ready(());
    }

    // We do a manual deadline check here. The timer wheel may not immediately
    // check the deadline if the executor was blocked. Skip this check under
    // Miri as it interferes with time simulation.
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
  }

  fn clear(&self) {
    unsafe {
      *self.sleep.get() = None;
    }
    self.wake_state.ready.store(false, Ordering::Release);
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
      self.internal_waker.wake_by_ref();
    }
  }
}

struct MutableSleepWaker {
  ready: AtomicBool,
  external_waker: AtomicWaker,
}

impl Wake for MutableSleepWaker {
  fn wake(self: Arc<Self>) {
    self.wake_by_ref();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.ready.store(true, Ordering::Release);
    self.external_waker.wake();
  }
}

/// A single-deadline timer for JS-managed user timers.
///
/// `UserTimer` is a simple "wake me at time T" mechanism. The JS side
/// manages timer bucketing, linked lists, and priority queues (matching
/// Node.js's architecture). Rust just needs to know when to wake up.
pub(crate) struct UserTimer<R: Reactor> {
  reactor: R,
  sleep: MutableSleep<R::Timer>,
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

#[cfg(all(test, feature = "reactor-tokio"))]
mod tests {
  use std::sync::Arc;
  use std::sync::mpsc;
  use std::task::Wake;

  use super::*;

  struct NotifyWaker(mpsc::Sender<()>);

  impl Wake for NotifyWaker {
    fn wake(self: Arc<Self>) {
      self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
      self.0.send(()).unwrap();
    }
  }

  #[test]
  fn timer_wake_from_another_thread() {
    let sleep = MutableSleep::<crate::reactor_tokio::TokioTimer>::new();

    // A wake before the external waker is registered must remain observable.
    let timer_waker = sleep.internal_waker.clone();
    std::thread::spawn(move || timer_waker.wake())
      .join()
      .unwrap();

    let (notify_tx, notify_rx) = mpsc::channel();
    let external_waker = Waker::from(Arc::new(NotifyWaker(notify_tx)));
    let mut cx = Context::from_waker(&external_waker);
    assert_eq!(sleep.poll_ready(&mut cx), Poll::Ready(()));

    // Once registered, an off-thread wake must notify the external task and
    // make the timer ready.
    let timer_waker = sleep.internal_waker.clone();
    assert_eq!(sleep.poll_ready(&mut cx), Poll::Pending);

    std::thread::spawn(move || timer_waker.wake())
      .join()
      .unwrap();

    notify_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(sleep.poll_ready(&mut cx), Poll::Ready(()));
  }
}
