// Copyright 2018-2025 the Deno authors. MIT license.

use parking_lot::Condvar;
use parking_lot::Mutex;

struct Permits {
  max: usize,
  used: usize,
}

pub struct Permit<'a>(&'a Semaphore);

impl<'a> Drop for Permit<'a> {
  fn drop(&mut self) {
    self.0.release();
  }
}

pub struct Semaphore {
  permits: Mutex<Permits>,
  condvar: Condvar,
}

impl Semaphore {
  pub fn new(max_permits: usize) -> Self {
    Semaphore {
      permits: Mutex::new(Permits {
        max: max_permits,
        used: 0,
      }),
      condvar: Condvar::new(),
    }
  }

  pub fn acquire(&self) -> Permit<'_> {
    {
      let mut permits = self.permits.lock();
      while permits.used >= permits.max {
        self.condvar.wait(&mut permits);
      }
      permits.used += 1;
    }
    Permit(self)
  }

  fn release(&self) {
    let mut permits = self.permits.lock();
    if permits.used == 0 {
      return;
    }
    permits.used -= 1;
    if permits.used < permits.max {
      drop(permits);
      self.condvar.notify_one();
    }
  }

  pub fn set_max(&self, n: usize) {
    let mut permits = self.permits.lock();
    let is_greater = n > permits.max;
    permits.max = n;
    drop(permits);
    if is_greater {
      self.condvar.notify_all(); // Wake up waiting threads
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;
  use std::thread;
  use std::time::Duration;

  use super::*;

  #[test]
  fn test_multiple_acquire_release() {
    let sem = Semaphore::new(3);
    sem.acquire();
    sem.acquire();
    sem.acquire();
    sem.release();
    sem.release();
    sem.release();
  }

  #[test]
  fn test_concurrent_access() {
    let sem = Arc::new(Semaphore::new(2));
    let mut handles = vec![];

    for _ in 0..5 {
      let sem_clone = Arc::clone(&sem);
      #[allow(clippy::disallowed_methods)]
      let handle = thread::spawn(move || {
        sem_clone.acquire();
        thread::sleep(Duration::from_millis(10));
        sem_clone.release();
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }

  #[test]
  fn test_blocking_behavior() {
    let sem = Arc::new(Semaphore::new(1));
    let sem_clone = Arc::clone(&sem);

    sem.acquire();

    #[allow(clippy::disallowed_methods)]
    let handle = thread::spawn(move || {
      let start = std::time::Instant::now();
      sem_clone.acquire();
      let elapsed = start.elapsed();
      sem_clone.release();
      elapsed
    });

    thread::sleep(Duration::from_millis(50));
    sem.release();

    let elapsed = handle.join().unwrap();
    assert!(elapsed >= Duration::from_millis(40));
  }

  #[test]
  fn test_set_max_increase() {
    let sem = Arc::new(Semaphore::new(1));
    let sem_clone = Arc::clone(&sem);

    sem.acquire();

    #[allow(clippy::disallowed_methods)]
    let handle = thread::spawn(move || {
      sem_clone.acquire();
      sem_clone.release();
    });

    thread::sleep(Duration::from_millis(10));
    sem.set_max(2);

    handle.join().unwrap();
    sem.release();
  }

  #[test]
  fn test_set_max_decrease() {
    let sem = Semaphore::new(3);
    sem.acquire();
    sem.acquire();

    sem.set_max(1);

    sem.release();
    sem.release();
  }

  #[test]
  fn test_zero_permits_with_set_max() {
    let sem = Arc::new(Semaphore::new(0));
    let sem_clone = Arc::clone(&sem);

    #[allow(clippy::disallowed_methods)]
    let handle = thread::spawn(move || {
      sem_clone.acquire();
      sem_clone.release();
    });

    thread::sleep(Duration::from_millis(10));
    sem.set_max(1);

    handle.join().unwrap();
  }

  #[test]
  fn test_multiple_threads_wait_and_proceed() {
    let sem = Arc::new(Semaphore::new(1));
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
      let sem_clone = Arc::clone(&sem);
      let counter_clone = Arc::clone(&counter);
      #[allow(clippy::disallowed_methods)]
      let handle = thread::spawn(move || {
        sem_clone.acquire();
        let mut count = counter_clone.lock();
        *count += 1;
        thread::sleep(Duration::from_millis(5));
        drop(count);
        sem_clone.release();
      });
      handles.push(handle);
    }

    for handle in handles {
      handle.join().unwrap();
    }

    let final_count = *counter.lock();
    assert_eq!(final_count, 10);
  }
}
