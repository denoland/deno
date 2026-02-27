// Copyright 2018-2025 the Deno authors. MIT license.

use crate::reactor::Reactor;
use crate::reactor::ReactorInstant;
use crate::reactor::ReactorTimer;
use cooked_waker::IntoWaker;
use cooked_waker::ViaRawPointer;
use cooked_waker::Wake;
use cooked_waker::WakeRef;
use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_set;
use std::mem::MaybeUninit;
use std::num::NonZeroU64;
use std::ops::Deref;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::task::ready;
use std::time::Duration;

pub(crate) type WebTimerId = u64;

/// The minimum number of tombstones required to trigger compaction
const COMPACTION_MINIMUM: usize = 16;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TimerType {
  Repeat(NonZeroU64),
  Once,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct TimerKey<I: ReactorInstant>(I, u64, TimerType, bool);

struct TimerData<T> {
  data: T,
  unrefd: bool,
  #[cfg(any(windows, test))]
  high_res: bool,
  #[cfg(not(any(windows, test)))]
  high_res: (),
}

/// Implements much of the specification described by https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html.
///
/// To ensure that we perform well in the face of large numbers of timers, this implementation assumes
/// that timers may complete in batches that are properly ordered according to the spec. That is to say, timers
/// are executed in an order according to the following specification:
///
/// > Wait until any invocations of this algorithm that had the same global and orderingIdentifier,
/// > that started before this one, and whose milliseconds is equal to or less than this one's,
/// > have completed.
///
/// This complicates timer resolution because timers fire in an order based on _milliseconds_ (aka their original
/// timeout duration) rather than an exact moment in time.
///
/// While we respect the spirit of this paragraph, we make an assumption that all timer callback invocations are
/// instantaneous. This means that we can assume that no further timers have elapsed during the execution of a batch of
/// timers. This does not always hold up in reality (for example, a poorly-written timer in a worker may block on
/// `Atomics.wait` or a synchronous `XMLHttpRequest`), but it allows us to process timers in a simpler and less-racy way.
///
/// We also assume that our underlying timers are high-resolution, and that our underlying timer source resolves in
/// a proper start time + expiration time order.
///
/// Finally, we assume that the event loop time -- the time between which we are able to poll this set of timers -- is
/// non-zero, and that multiple timers may fire during this period and require re-ordering by their original millisecond
/// timeouts.
///
///
/// https://github.com/denoland/deno/pull/12953 -- Add refTimer, unrefTimer API
/// https://github.com/denoland/deno/pull/12862 -- Refactor timers to use one async op per timer
///
/// https://github.com/denoland/deno/issues/11398 -- Spurious assertion error when the callback to setInterval lasts longer than the interval
pub(crate) struct WebTimers<T, R: Reactor> {
  reactor: R,
  next_id: Cell<WebTimerId>,
  timers: RefCell<BTreeSet<TimerKey<R::Instant>>>,
  /// We choose a `BTreeMap` over `HashMap` because of memory performance.
  data_map: RefCell<BTreeMap<WebTimerId, TimerData<T>>>,
  /// How many unref'd timers exist?
  unrefd_count: Cell<usize>,
  /// A heap-allocated MutableSleep. Stored as a raw pointer (not Box) to avoid
  /// Box's Unique retag conflicting with the self-referential waker pointer.
  sleep: OwnedPtr<MutableSleep<R::Timer>>,
  /// The high-res timer lock. No-op on platforms other than Windows.
  high_res_timer_lock: HighResTimerLock,
}

impl<T, R: Reactor + Default> Default for WebTimers<T, R> {
  fn default() -> Self {
    Self::new(R::default())
  }
}

impl<T, R: Reactor> WebTimers<T, R> {
  pub fn new(reactor: R) -> Self {
    Self {
      reactor,
      next_id: Default::default(),
      timers: Default::default(),
      data_map: Default::default(),
      unrefd_count: Default::default(),
      sleep: MutableSleep::new(),
      high_res_timer_lock: Default::default(),
    }
  }

  #[allow(unused)]
  pub fn has_pending(&self) -> bool {
    !self.timers.borrow().is_empty()
  }
}

pub(crate) struct WebTimersIterator<'a, T, I: ReactorInstant> {
  data: Ref<'a, BTreeMap<WebTimerId, TimerData<T>>>,
  timers: Ref<'a, BTreeSet<TimerKey<I>>>,
}

impl<'a, T, I: ReactorInstant> IntoIterator
  for &'a WebTimersIterator<'a, T, I>
{
  type IntoIter = WebTimersIteratorImpl<'a, T, I>;
  type Item = (u64, bool, bool);

  fn into_iter(self) -> Self::IntoIter {
    WebTimersIteratorImpl {
      data: &self.data,
      timers: self.timers.iter(),
    }
  }
}

pub(crate) struct WebTimersIteratorImpl<'a, T, I: ReactorInstant> {
  data: &'a BTreeMap<WebTimerId, TimerData<T>>,
  timers: btree_set::Iter<'a, TimerKey<I>>,
}

impl<T, I: ReactorInstant> Iterator for WebTimersIteratorImpl<'_, T, I> {
  type Item = (u64, bool, bool);
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let item = self.timers.next()?;
      if self.data.contains_key(&item.1) {
        return Some((item.1, !matches!(item.2, TimerType::Once), item.3));
      }
    }
  }
}

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

impl<T: Clone, R: Reactor> WebTimers<T, R> {
  /// Returns an internal iterator that locks the internal data structures for the period
  /// of iteration. Calling other methods on this collection will cause a panic.
  pub(crate) fn iter(&self) -> WebTimersIterator<'_, T, R::Instant> {
    WebTimersIterator {
      data: self.data_map.borrow(),
      timers: self.timers.borrow(),
    }
  }

  /// Refs a timer by ID. Invalid IDs are ignored.
  pub fn ref_timer(&self, id: WebTimerId) {
    if let Some(TimerData { unrefd, .. }) =
      self.data_map.borrow_mut().get_mut(&id)
      && std::mem::replace(unrefd, false)
    {
      self.unrefd_count.set(self.unrefd_count.get() - 1);
    }
  }

  /// Unrefs a timer by ID. Invalid IDs are ignored.
  pub fn unref_timer(&self, id: WebTimerId) {
    if let Some(TimerData { unrefd, .. }) =
      self.data_map.borrow_mut().get_mut(&id)
      && !std::mem::replace(unrefd, true)
    {
      self.unrefd_count.set(self.unrefd_count.get() + 1);
    }
  }

  /// Queues a timer to be fired in order with the other timers in this set of timers.
  pub fn queue_timer(&self, timeout_ms: u64, data: T) -> WebTimerId {
    self.queue_timer_internal(false, timeout_ms, data, false)
  }

  /// Queues a timer to be fired in order with the other timers in this set of timers.
  pub fn queue_timer_repeat(&self, timeout_ms: u64, data: T) -> WebTimerId {
    self.queue_timer_internal(true, timeout_ms, data, false)
  }

  pub fn queue_system_timer(
    &self,
    repeat: bool,
    timeout_ms: u64,
    data: T,
  ) -> WebTimerId {
    self.queue_timer_internal(repeat, timeout_ms, data, true)
  }

  fn queue_timer_internal(
    &self,
    repeat: bool,
    timeout_ms: u64,
    data: T,
    is_system_timer: bool,
  ) -> WebTimerId {
    #[allow(clippy::let_unit_value)]
    let high_res = self.high_res_timer_lock.maybe_lock(timeout_ms);

    let id = self.next_id.get() + 1;
    self.next_id.set(id);

    let mut timers = self.timers.borrow_mut();
    let deadline = self
      .reactor
      .now()
      .checked_add(Duration::from_millis(timeout_ms))
      .unwrap();
    match timers.first() {
      Some(TimerKey(k, ..)) => {
        if &deadline < k {
          self.sleep.change(self.reactor.timer(deadline));
        }
      }
      _ => {
        self.sleep.change(self.reactor.timer(deadline));
      }
    }

    let timer_type = if repeat {
      TimerType::Repeat(
        NonZeroU64::new(timeout_ms).unwrap_or(NonZeroU64::new(1).unwrap()),
      )
    } else {
      TimerType::Once
    };
    timers.insert(TimerKey(deadline, id, timer_type, is_system_timer));

    let mut data_map = self.data_map.borrow_mut();
    data_map.insert(
      id,
      TimerData {
        data,
        unrefd: false,
        high_res,
      },
    );
    id
  }

  /// Cancels a pending timer in this set of timers, returning the associated data if a timer
  /// with the given ID was found.
  pub fn cancel_timer(&self, timer: u64) -> Option<T> {
    let mut data_map = self.data_map.borrow_mut();
    match data_map.remove(&timer) {
      Some(TimerData {
        data,
        unrefd,
        high_res,
      }) => {
        if data_map.is_empty() {
          // When the # of running timers hits zero, clear the timer tree.
          // When debug assertions are enabled, we do a consistency check.
          debug_assert_eq!(self.unrefd_count.get(), if unrefd { 1 } else { 0 });
          #[cfg(any(windows, test))]
          debug_assert_eq!(self.high_res_timer_lock.is_locked(), high_res);
          self.high_res_timer_lock.clear();
          self.unrefd_count.set(0);
          self.timers.borrow_mut().clear();
          self.sleep.clear();
        } else {
          self.high_res_timer_lock.maybe_unlock(high_res);
          if unrefd {
            self.unrefd_count.set(self.unrefd_count.get() - 1);
          }
        }
        Some(data)
      }
      _ => None,
    }
  }

  /// Poll for any timers that have completed.
  ///
  /// Returns the IDs and [`TimerType`]s of expired timers. The associated
  /// data must be retrieved per-timer via
  /// [`take_fired_timer`](Self::take_fired_timer), which allows
  /// `cancel_timer` to prevent dispatch of timers that expired in the
  /// same batch.
  pub fn poll_timers(&self, cx: &mut Context) -> Poll<Vec<(u64, TimerType)>> {
    ready!(self.sleep.poll_ready(cx));
    let now = R::Instant::now();
    let mut timers = self.timers.borrow_mut();
    let data = self.data_map.borrow();
    let mut output = vec![];
    let mut fired_once_count: usize = 0;

    let mut split = timers.split_off(&TimerKey(now, 0, TimerType::Once, false));
    std::mem::swap(&mut split, &mut timers);
    for TimerKey(_, id, timer_type, is_system_timer) in split {
      if !data.contains_key(&id) {
        continue; // tombstone
      }
      if let TimerType::Repeat(interval) = &timer_type {
        timers.insert(TimerKey(
          now
            .checked_add(Duration::from_millis((*interval).into()))
            .unwrap(),
          id,
          timer_type.clone(),
          is_system_timer,
        ));
      } else {
        fired_once_count += 1;
      }
      output.push((id, timer_type));
    }

    // In-effective poll, run a front-compaction and try again later
    if output.is_empty() {
      // We should never have an ineffective poll when the data map is empty, as we check
      // for this in cancel_timer.
      debug_assert!(!data.is_empty());
      while let Some(TimerKey(_, id, ..)) = timers.first() {
        if data.contains_key(id) {
          break;
        } else {
          timers.pop_first();
        }
      }
      if let Some(TimerKey(k, ..)) = timers.first() {
        self.sleep.change(self.reactor.timer(*k));
      }
      return Poll::Pending;
    }

    // Adjust for fired-once timers whose data is still in data_map
    // (it will be removed by take_fired_timer).
    let pending_data_count = data.len() - fired_once_count;

    if pending_data_count == 0 {
      // No more pending timers; clear the tree and sleep.
      if !timers.is_empty() {
        timers.clear();
      }
      self.sleep.clear();
    } else {
      // Run compaction when there are enough tombstones to justify cleanup.
      let tombstone_count = timers.len() - pending_data_count;
      if tombstone_count > COMPACTION_MINIMUM {
        timers.retain(|k| data.contains_key(&k.1));
      }
      if let Some(TimerKey(k, ..)) = timers.first() {
        self.sleep.change(self.reactor.timer(*k));
      }
    }

    Poll::Ready(output)
  }

  /// Extracts the data for a previously-fired timer. Returns `None` if
  /// the timer was cancelled between [`poll_timers`](Self::poll_timers)
  /// and this call.
  pub fn take_fired_timer(&self, id: u64, timer_type: &TimerType) -> Option<T> {
    match timer_type {
      TimerType::Repeat(_) => {
        self.data_map.borrow().get(&id).map(|td| td.data.clone())
      }
      TimerType::Once => {
        let mut data = self.data_map.borrow_mut();
        let TimerData {
          data: d,
          unrefd,
          high_res,
        } = data.remove(&id)?;
        if data.is_empty() {
          self.high_res_timer_lock.clear();
          self.unrefd_count.set(0);
          self.timers.borrow_mut().clear();
          self.sleep.clear();
        } else {
          self.high_res_timer_lock.maybe_unlock(high_res);
          if unrefd {
            self.unrefd_count.set(self.unrefd_count.get() - 1);
          }
        }
        Some(d)
      }
    }
  }

  /// Is this set of timers empty?
  pub fn is_empty(&self) -> bool {
    self.data_map.borrow().is_empty()
  }

  /// The total number of timers in this collection.
  pub fn len(&self) -> usize {
    self.data_map.borrow().len()
  }

  /// The number of unref'd timers in this collection.
  pub fn unref_len(&self) -> usize {
    self.unrefd_count.get()
  }

  #[cfg(test)]
  pub fn assert_consistent(&self) {
    if self.data_map.borrow().is_empty() {
      // If the data map is empty, we should have no timers, no unref'd count, no high-res lock
      assert_eq!(self.timers.borrow().len(), 0);
      assert_eq!(self.unrefd_count.get(), 0);
      assert!(!self.high_res_timer_lock.is_locked());
    } else {
      assert!(self.unrefd_count.get() <= self.data_map.borrow().len());
      // The high-res lock count must be <= the number of remaining timers
      assert!(self.high_res_timer_lock.lock_count.get() <= self.len());
    }
  }

  pub fn has_pending_timers(&self) -> bool {
    self.len() > self.unref_len()
  }
}

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "C" {
  fn timeBeginPeriod(n: u32);
  fn timeEndPeriod(n: u32);
}

#[derive(Default)]
struct HighResTimerLock {
  #[cfg(any(windows, test))]
  lock_count: Cell<usize>,
}

impl HighResTimerLock {
  /// If a timer is requested with <=100ms resolution, request the high-res timer. Since the default
  /// Windows timer period is 15ms, this means a 100ms timer could fire at 115ms (15% late). We assume that
  /// timers longer than 100ms are a reasonable cutoff here.
  ///
  /// The high-res timers on Windows are still limited. Unfortunately this means that our shortest duration 4ms timers
  /// can still be 25% late, but without a more complex timer system or spinning on the clock itself, we're somewhat
  /// bounded by the OS' scheduler itself.
  #[cfg(any(windows, test))]
  const LOW_RES_TIMER_RESOLUTION: u64 = 100;

  #[cfg(any(windows, test))]
  #[inline(always)]
  fn maybe_unlock(&self, high_res: bool) {
    if high_res {
      let old = self.lock_count.get();
      debug_assert!(old > 0);
      let new = old - 1;
      self.lock_count.set(new);
      #[cfg(windows)]
      if new == 0 {
        // SAFETY: Windows API
        unsafe {
          timeEndPeriod(1);
        }
      }
    }
  }

  #[cfg(not(any(windows, test)))]
  #[inline(always)]
  fn maybe_unlock(&self, _high_res: ()) {}

  #[cfg(any(windows, test))]
  #[inline(always)]
  fn maybe_lock(&self, timeout_ms: u64) -> bool {
    if timeout_ms <= Self::LOW_RES_TIMER_RESOLUTION {
      let old = self.lock_count.get();
      #[cfg(windows)]
      if old == 0 {
        // SAFETY: Windows API
        unsafe {
          timeBeginPeriod(1);
        }
      }
      self.lock_count.set(old + 1);
      true
    } else {
      false
    }
  }

  #[cfg(not(any(windows, test)))]
  #[inline(always)]
  fn maybe_lock(&self, _timeout_ms: u64) {}

  #[cfg(any(windows, test))]
  #[inline(always)]
  fn clear(&self) {
    #[cfg(windows)]
    if self.lock_count.get() > 0 {
      // SAFETY: Windows API
      unsafe {
        timeEndPeriod(1);
      }
    }

    self.lock_count.set(0);
  }

  #[cfg(not(any(windows, test)))]
  #[inline(always)]
  fn clear(&self) {}

  #[cfg(any(windows, test))]
  fn is_locked(&self) -> bool {
    self.lock_count.get() > 0
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reactor_tokio::TokioReactor;
  use rstest::rstest;
  use std::future::Future;
  use std::future::poll_fn;

  type TestTimers = WebTimers<(), TokioReactor>;

  /// Miri is way too slow here on some of the larger tests.
  const TEN_THOUSAND: u64 = if cfg!(miri) { 100 } else { 10_000 };

  /// Helper function to support miri + rstest. We cannot use I/O in a miri test.
  fn async_test<F: Future<Output = T>, T>(f: F) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_time()
      .build()
      .unwrap();
    runtime.block_on(f)
  }

  async fn poll_all(timers: &TestTimers) -> Vec<u64> {
    timers.assert_consistent();
    let len = timers.len();
    let mut v = vec![];
    while !timers.is_empty() {
      let batch = poll_fn(|cx| {
        timers.assert_consistent();
        timers.poll_timers(cx)
      })
      .await;
      for (id, timer_type) in &batch {
        timers.take_fired_timer(*id, timer_type);
      }
      v.extend(batch.into_iter().map(|(id, _)| id));
      #[allow(clippy::print_stderr)]
      {
        eprintln!(
          "{} ({} {})",
          v.len(),
          timers.len(),
          timers.data_map.borrow().len(),
        );
      }
      timers.assert_consistent();
    }
    assert_eq!(v.len(), len);
    v
  }

  /// This test attempts to mimic a memory leak fix in the timer compaction logic.
  /// See https://github.com/denoland/deno/issues/27925
  ///
  /// The leak happens when there are enough tombstones to justify cleanup
  /// (tombstone_count > COMPACTION_MINIMUM) but there are also more active timers
  /// than tombstones (tombstone_count <= data.len()). In this scenario, the original
  /// condition won't trigger compaction, allowing tombstones to accumulate.
  #[test]
  fn test_timer_tombstone_memory_leak() {
    const ACTIVE_TIMERS: usize = 100;
    const TOMBSTONES: usize = 30; // > COMPACTION_MINIMUM but < ACTIVE_TIMERS
    const CLEANUP_THRESHOLD: usize = 5; // Threshold to determine if compaction happened
    async_test(async {
      let timers = TestTimers::default();

      // Create mostly long-lived timers, with a few immediate ones
      // The immediate timers ensure poll_timers returns non-empty output
      // which prevents the front-compaction mechanism from cleaning up tombstones
      let mut active_timer_ids = Vec::with_capacity(ACTIVE_TIMERS);
      for i in 0..ACTIVE_TIMERS {
        let timeout = if i < CLEANUP_THRESHOLD { 1 } else { 10000 };
        active_timer_ids.push(timers.queue_timer(timeout, ()));
      }

      // Create and immediately cancel timers to generate tombstones
      for _ in 0..TOMBSTONES {
        let id = timers.queue_timer(10000, ());
        timers.cancel_timer(id);
      }

      let count_tombstones =
        || timers.timers.borrow().len() - timers.data_map.borrow().len();
      let initial_tombstones = count_tombstones();

      // Verify test setup is correct
      assert!(
        initial_tombstones > COMPACTION_MINIMUM,
        "Test requires tombstones > COMPACTION_MINIMUM"
      );
      assert!(
        initial_tombstones <= ACTIVE_TIMERS,
        "Test requires tombstones <= active_timers"
      );

      // Poll timers to trigger potential compaction
      let fired = poll_fn(|cx| timers.poll_timers(cx)).await;
      for (id, timer_type) in &fired {
        timers.take_fired_timer(*id, timer_type);
      }

      let remaining_tombstones = count_tombstones();

      for id in active_timer_ids {
        timers.cancel_timer(id);
      }

      assert!(
        remaining_tombstones < CLEANUP_THRESHOLD,
        "Memory leak: Tombstones not cleaned up"
      );
    });
  }

  #[test]
  fn test_timer() {
    async_test(async {
      let timers = TestTimers::default();
      let _a = timers.queue_timer(1, ());

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 1);
    });
  }

  #[test]
  fn test_high_res_lock() {
    async_test(async {
      let timers = TestTimers::default();
      assert!(!timers.high_res_timer_lock.is_locked());
      let _a = timers.queue_timer(1, ());
      assert!(timers.high_res_timer_lock.is_locked());

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 1);
      assert!(!timers.high_res_timer_lock.is_locked());
    });
  }

  #[rstest]
  #[test]
  fn test_timer_cancel_1(#[values(0, 1, 2, 3)] which: u64) {
    async_test(async {
      let timers = TestTimers::default();
      for i in 0..4 {
        let id = timers.queue_timer(i * 25, ());
        if i == which {
          assert!(timers.cancel_timer(id).is_some());
        }
      }
      assert_eq!(timers.len(), 3);

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 3);
    })
  }

  #[rstest]
  #[test]
  fn test_timer_cancel_2(#[values(0, 1, 2)] which: u64) {
    async_test(async {
      let timers = TestTimers::default();
      for i in 0..4 {
        let id = timers.queue_timer(i * 25, ());
        if i == which || i == which + 1 {
          assert!(timers.cancel_timer(id).is_some());
        }
      }
      assert_eq!(timers.len(), 2);

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 2);
    })
  }

  #[test]
  fn test_timers_10_random() {
    async_test(async {
      let timers = TestTimers::default();
      for i in 0..10 {
        timers.queue_timer((i % 3) * 10, ());
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 10);
    })
  }

  #[test]
  fn test_timers_10_random_cancel() {
    async_test(async {
      let timers = TestTimers::default();
      for i in 0..10 {
        let id = timers.queue_timer((i % 3) * 10, ());
        timers.cancel_timer(id);
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 0);
    });
  }

  #[rstest]
  #[test]
  fn test_timers_10_random_cancel_after(#[values(true, false)] reverse: bool) {
    async_test(async {
      let timers = TestTimers::default();
      let mut ids = vec![];
      for i in 0..2 {
        ids.push(timers.queue_timer((i % 3) * 10, ()));
      }
      if reverse {
        ids.reverse();
      }
      for id in ids {
        timers.cancel_timer(id);
        timers.assert_consistent();
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 0);
    });
  }

  #[test]
  fn test_timers_10() {
    async_test(async {
      let timers = TestTimers::default();
      for _i in 0..10 {
        timers.queue_timer(1, ());
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 10);
    });
  }

  #[test]
  fn test_timers_10_000_random() {
    async_test(async {
      let timers = TestTimers::default();
      for i in 0..TEN_THOUSAND {
        timers.queue_timer(i % 10, ());
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), TEN_THOUSAND as usize);
    });
  }

  /// Cancel a large number of timers at the head of the queue to trigger
  /// a front compaction.
  #[test]
  fn test_timers_cancel_first() {
    async_test(async {
      let timers = TestTimers::default();
      let mut ids = vec![];
      for _ in 0..TEN_THOUSAND {
        ids.push(timers.queue_timer(1, ()));
      }
      for i in 0..10 {
        timers.queue_timer(i * 25, ());
      }
      for id in ids {
        timers.cancel_timer(id);
      }
      let v = poll_all(&timers).await;
      assert_eq!(v.len(), 10);
    });
  }

  #[test]
  fn test_timers_10_000_cancel_most() {
    async_test(async {
      let timers = TestTimers::default();
      let mut ids = vec![];
      for i in 0..TEN_THOUSAND {
        ids.push(timers.queue_timer(i % 100, ()));
      }

      // This should trigger a compaction
      fastrand::seed(42);
      ids.retain(|_| fastrand::u8(0..10) > 0);
      for id in ids.iter() {
        timers.cancel_timer(*id);
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), TEN_THOUSAND as usize - ids.len());
    });
  }

  #[rstest]
  #[test]
  fn test_chaos(#[values(42, 99, 1000)] seed: u64) {
    async_test(async {
      let timers = TestTimers::default();
      fastrand::seed(seed);

      let mut count = 0;
      let mut ref_count = 0;

      for _ in 0..TEN_THOUSAND {
        let mut cancelled = false;
        let mut unrefed = false;
        let id = timers.queue_timer(fastrand::u64(0..10), ());
        for _ in 0..fastrand::u64(0..10) {
          if fastrand::u8(0..10) == 0 {
            timers.cancel_timer(id);
            cancelled = true;
          }
          if fastrand::u8(0..10) == 0 {
            timers.ref_timer(id);
            unrefed = false;
          }
          if fastrand::u8(0..10) == 0 {
            timers.unref_timer(id);
            unrefed = true;
          }
        }

        if !cancelled {
          count += 1;
        }
        if !unrefed {
          ref_count += 1;
        }

        timers.assert_consistent();
      }

      #[allow(clippy::print_stderr)]
      {
        eprintln!("count={count} ref_count={ref_count}");
      }

      let v = poll_all(&timers).await;
      assert_eq!(v.len(), count);

      assert!(timers.is_empty());
      assert!(!timers.has_pending_timers());
    });
  }
}
