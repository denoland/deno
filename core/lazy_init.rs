use std::cell::UnsafeCell;
use parking_lot::Mutex;

/// Allows for the lazy creation of an immutable value of type `T` where
/// only the initial creation requires a lock.
pub struct LazyInit<T> {
    creation_fn: Mutex<Option<Box<dyn FnOnce() -> T + Send + Sync>>>,
    value: UnsafeCell<Option<T>>,
}

impl<T> std::fmt::Debug for LazyInit<T> where T : std::fmt::Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.get()))
    }
}

impl<T> LazyInit<T> {
    pub fn new(creation_fn: Box<dyn FnOnce() -> T + Send + Sync>) -> LazyInit<T> {
        LazyInit {
            creation_fn: Mutex::new(Some(creation_fn)),
            value: UnsafeCell::new(None),
        }
    }

    pub fn get(&self) -> &T {
        unsafe {
            if let Some(value) = (*self.value.get()).as_ref() {
                value
            } else {
                {
                    let mut creation_fn = self.creation_fn.lock();
                    if let Some(creation_fn) = creation_fn.take() {
                        *self.value.get() = Some(creation_fn());
                    } else {
                        // another thread initialized the value
                    }
                }

                (*self.value.get()).as_ref().unwrap()
            }
        }
    }
}

unsafe impl<T> Sync for LazyInit<T> where T: Send + Sync {
}
