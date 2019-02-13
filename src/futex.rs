extern crate integer_atomics;

use cfg_if;
use integer_atomics::AtomicI32;
use std::convert::From;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

// A Futex is a wrapper around AtomicI32 that adds Futex wait/wake
// capabilities.
#[derive(Default)]
#[repr(transparent)]
pub struct Futex {
  inner: AtomicI32,
}

// These values match the proposed WebAssembly shared memory spec.
// TODO: find a link to the spec.
// TODO: consider removing the NotEqual case as IMO it's unnecessary and
// the required check is inherently racy on Windows.
#[repr(i32)]
pub enum WaitResult {
  Ok = 0,
  NotEqual = 1,
  TimedOut = 2,
}

impl Futex {
  #[allow(dead_code)]
  pub fn new(value: i32) -> Self {
    Self {
      inner: AtomicI32::new(value),
    }
  }

  fn _assert_is_same_size_as_i32() {
    // This 'useless' transmute serves as a static assertion that a Futex has
    // the same size as an i32 and doesn't (indirectly) embed any other fields.
    unsafe { transmute::<i32, Self>(0) };
  }
}

impl From<AtomicI32> for Futex {
  fn from(inner: AtomicI32) -> Self {
    Self { inner }
  }
}

impl Into<AtomicI32> for Futex {
  fn into(self) -> AtomicI32 {
    self.inner
  }
}

impl Deref for Futex {
  type Target = AtomicI32;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for Futex {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

cfg_if! {
  if #[cfg(windows)] {
    use std::mem::size_of_val;
    use std::sync::atomic::Ordering;
    use winapi::shared::minwindef::{DWORD, TRUE};
    use winapi::shared::ntdef::VOID;
    use winapi::shared::winerror::ERROR_TIMEOUT;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::synchapi::WaitOnAddress;
    use winapi::um::synchapi::{WakeByAddressAll, WakeByAddressSingle};
    use winapi::um::winbase::INFINITE;

    impl Futex {
      const MILLIS_PER_SEC: DWORD = 1_000;

      #[inline]
      fn as_void_ptr(&self) -> *mut VOID {
        // The WaitOnAddress and WakeByAddress functions operate on mutable
        // void pointers, but neither of them actually change the value.
        &self.inner as *const _ as *mut VOID
      }

      pub fn wait(
        &self,
        value: i32,
        timeout: Option<Duration>
      ) -> WaitResult {
        // Check for a non-matching value. As this check is inherently racy
        // anyway, use relaxed memory ordering for performance.
        if self.inner.load(Ordering::Relaxed) != value {
          return WaitResult::NotEqual;
        }
        let timeout_ms = match timeout {
          Some(dur) => dur.as_secs() as DWORD * Self::MILLIS_PER_SEC +
                       dur.subsec_millis(),
          None => INFINITE,
        };
        let success = unsafe {
          WaitOnAddress(
            self.as_void_ptr(),
            &value as *const _ as *mut VOID,
            size_of_val(&value),
            timeout_ms,
          )
        };
        if success == TRUE {
          WaitResult::Ok
        } else if unsafe { GetLastError() } == ERROR_TIMEOUT {
          WaitResult::TimedOut
        } else {
          panic!("WaitOnAddress failed unexpectedly.")
        }
      }

      pub fn notify_one(&self) {
        unsafe { WakeByAddressSingle(self.as_void_ptr()) }
      }

      pub fn notify_all(&self) {
        unsafe { WakeByAddressAll(self.as_void_ptr()) }
      }
    }
  } else {
    // Futex emulation for other platforms.
    // TODO: implement native futexes for linux.
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};

    impl Futex {
      #[inline]
      fn lock_table() -> MutexGuard<'static, HashMap<usize, Arc<Condvar>>> {
        lazy_static! {
          static ref TABLE: Mutex<HashMap<usize, Arc<Condvar>>> =
            Mutex::new(HashMap::new());
        }
        TABLE.lock().unwrap()
      }

      #[inline]
      fn key(&self) -> usize {
        &self.inner as *const _ as usize
      }

      pub fn wait(
        &self,
        value: i32,
        timeout: Option<Duration>
      ) -> WaitResult {
        let mut table = Self::lock_table();
        // Comparing the futex value must be done with the global lock held.
        if self.inner.load(Ordering::Acquire) != value {
          return WaitResult::NotEqual;
        }
        let condvar = table
          .entry(self.key())
          .or_insert_with(|| Arc::new(Condvar::new()))
          .clone();
        let (mut table, result) = match timeout {
          None => (condvar.wait(table).unwrap(), WaitResult::Ok),
          Some(dur) => {
            let (table, lock_result) = condvar
              .wait_timeout(table, dur)
              .unwrap();
            (
              table,
              if lock_result.timed_out() {
                WaitResult::TimedOut
              } else {
                WaitResult::Ok
              },
            )
          }
        };
        // Drop the condition variable from the address table if there are
        // *two* references left: one reference originates from the HashMap
        // entry, the other is on this function's stack frame.
        if Arc::strong_count(&condvar) == 2 {
          table.remove(&self.key());
        }
        result
      }

      pub fn notify_one(&self) {
        let table = Self::lock_table();
        if let Some(condvar) = table.get(&self.key()) {
          condvar.notify_one();
        }
      }

      pub fn notify_all(&self) {
        let table = Self::lock_table();
        if let Some(condvar) = table.get(&self.key()) {
          condvar.notify_all();
        }
      }
    }
  }
}
