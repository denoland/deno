use parking_lot::Mutex;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Allows for the lazy creation of an immutable value of type `T` where
/// only the initial creation requires a lock.
///
/// This is inspired by the `lazy-init` crate, but creation is done
/// via the constructor.
pub struct LazyInit<T> {
  creation_fn: Mutex<Option<Box<dyn FnOnce() -> T + Send + Sync>>>,
  initialized: AtomicBool,
  value: UnsafeCell<Option<T>>,
}

impl<T> std::fmt::Debug for LazyInit<T>
where
  T: std::fmt::Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&format!("{:?}", self.get()))
  }
}

impl<T> LazyInit<T> {
  pub fn new(creation_fn: impl FnOnce() -> T + Send + Sync + 'static) -> Self {
    LazyInit {
      creation_fn: Mutex::new(Some(Box::new(creation_fn))),
      initialized: AtomicBool::new(false),
      value: UnsafeCell::new(None),
    }
  }

  pub fn get(&self) -> &T {
    // create a barrier to ensure all writes to self.value are seen across threads
    if !self.initialized.load(Ordering::Acquire) {
      {
        let mut creation_fn = self.creation_fn.lock();
        if let Some(creation_fn) = creation_fn.take() {
          unsafe {
            *self.value.get() = Some(creation_fn());
          }
        } else {
          // another thread initialized the value
        }
      }
      self.initialized.store(true, Ordering::Release);
    }

    unsafe {
      (*self.value.get()).as_ref().unwrap()
    }
  }
}

// The code above ensures the synchronization between threads.
unsafe impl<T> Sync for LazyInit<T> where T: Send + Sync {}

#[cfg(test)]
mod test {
  use super::LazyInit;

  #[test]
  fn it_should_get_the_same_value() {
    let lazy1 = std::sync::Arc::new(LazyInit::new(|| 1));
    let lazy2 = std::sync::Arc::new(LazyInit::new(|| {
      // cause other threads to get the lock
      std::thread::sleep(std::time::Duration::from_millis(50));
      2
    }));

    let mut handles = Vec::new();
    for _ in 0..4 {
      let lazy1 = lazy1.clone();
      let lazy2 = lazy2.clone();
      handles.push(std::thread::spawn(move || {
        assert_eq!(*lazy1.get(), 1);
        assert_eq!(*lazy2.get(), 2);
      }));
    }

    for handle in handles {
      handle.join().unwrap();
    }
  }
}