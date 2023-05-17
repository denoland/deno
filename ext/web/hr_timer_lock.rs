// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
#[cfg(target_os = "windows")]
mod windows {
  use std::marker::PhantomData;
  use std::sync::atomic::AtomicU32;

  pub(crate) struct HrTimerLock {
    pub(super) _unconstructable: PhantomData<()>,
  }

  /// Decrease the reference count of the HR timer on drop.
  impl Drop for HrTimerLock {
    fn drop(&mut self) {
      dec_ref();
    }
  }

  /// Maintains the HR timer refcount. Times should not be nested more than 5 deep so this is
  /// more than sufficient.
  static TIMER_REFCOUNT: AtomicU32 = AtomicU32::new(0);

  pub(super) fn inc_ref() {
    let old = TIMER_REFCOUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // Overflow/underflow sanity check in debug mode
    debug_assert!(old != u32::MAX);
    if old == 0 {
      lock_hr();
    }
  }

  fn dec_ref() {
    let old = TIMER_REFCOUNT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    // Overflow/underflow sanity check in debug mode
    debug_assert!(old != 0);
    if old == 1 {
      unlock_hr();
    }
  }

  fn lock_hr() {
    // SAFETY: We just want to set the timer period here
    unsafe { winmm::timeBeginPeriod(1) };
  }

  fn unlock_hr() {
    // SAFETY: We just want to set the timer period here
    unsafe { winmm::timeEndPeriod(1) };
  }
}

#[cfg(target_os = "windows")]
pub(crate) fn hr_timer_lock() -> windows::HrTimerLock {
  windows::inc_ref();
  windows::HrTimerLock {
    _unconstructable: Default::default(),
  }
}

/// No-op on other platforms.
#[cfg(not(target_os = "windows"))]
pub(crate) fn hr_timer_lock() -> () {
  ()
}
