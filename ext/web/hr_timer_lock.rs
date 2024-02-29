// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
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

  /// Maintains the HR timer refcount. This should be more than sufficient as 2^32 timers would be
  /// an impossible situation, and if it does somehow happen, the worst case is that we'll disable
  /// the high-res timer when we shouldn't (and things would eventually return to proper operation).
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

  /// If the refcount is > 0, we ask Windows for a lower timer period once. While the underlying
  /// Windows timeBeginPeriod/timeEndPeriod API can manage its own reference counts, we choose to
  /// use it once per process and avoid nesting these calls.
  fn lock_hr() {
    // SAFETY: We just want to set the timer period here
    unsafe { windows_sys::Win32::Media::timeBeginPeriod(1) };
  }

  fn unlock_hr() {
    // SAFETY: We just want to set the timer period here
    unsafe { windows_sys::Win32::Media::timeEndPeriod(1) };
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
pub(crate) fn hr_timer_lock() -> (std::marker::PhantomData<()>,) {
  Default::default()
}
