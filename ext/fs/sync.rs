// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub use inner::*;

#[cfg(feature = "sync_fs")]
mod inner {
  #![allow(clippy::disallowed_types)]

  use std::ops::Deref;
  use std::ops::DerefMut;
  pub use std::sync::Arc as MaybeArc;

  pub use core::marker::Send as MaybeSend;
  pub use core::marker::Sync as MaybeSync;

  pub struct MaybeArcMutexGuard<'lock, T>(std::sync::MutexGuard<'lock, T>);

  impl<'lock, T> Deref for MaybeArcMutexGuard<'lock, T> {
    type Target = std::sync::MutexGuard<'lock, T>;
    fn deref(&self) -> &std::sync::MutexGuard<'lock, T> {
      &self.0
    }
  }

  impl<'lock, T> DerefMut for MaybeArcMutexGuard<'lock, T> {
    fn deref_mut(&mut self) -> &mut std::sync::MutexGuard<'lock, T> {
      &mut self.0
    }
  }

  #[derive(Debug)]
  pub struct MaybeArcMutex<T>(std::sync::Arc<std::sync::Mutex<T>>);
  impl<T> MaybeArcMutex<T> {
    pub fn new(val: T) -> Self {
      Self(std::sync::Arc::new(std::sync::Mutex::new(val)))
    }
  }

  impl<'lock, T> MaybeArcMutex<T> {
    pub fn lock(&'lock self) -> MaybeArcMutexGuard<'lock, T> {
      MaybeArcMutexGuard(self.0.lock().unwrap())
    }
  }
}

#[cfg(not(feature = "sync_fs"))]
mod inner {
  use std::ops::Deref;
  use std::ops::DerefMut;
  pub use std::rc::Rc as MaybeArc;

  pub trait MaybeSync {}
  impl<T> MaybeSync for T where T: ?Sized {}
  pub trait MaybeSend {}
  impl<T> MaybeSend for T where T: ?Sized {}

  pub struct MaybeArcMutexGuard<'lock, T>(std::cell::RefMut<'lock, T>);

  impl<'lock, T> Deref for MaybeArcMutexGuard<'lock, T> {
    type Target = std::cell::RefMut<'lock, T>;
    fn deref(&self) -> &std::cell::RefMut<'lock, T> {
      &self.0
    }
  }

  impl<'lock, T> DerefMut for MaybeArcMutexGuard<'lock, T> {
    fn deref_mut(&mut self) -> &mut std::cell::RefMut<'lock, T> {
      &mut self.0
    }
  }

  #[derive(Debug)]
  pub struct MaybeArcMutex<T>(std::rc::Rc<std::cell::RefCell<T>>);
  impl<T> MaybeArcMutex<T> {
    pub fn new(val: T) -> Self {
      Self(std::rc::Rc::new(std::cell::RefCell::new(val)))
    }
  }

  impl<'lock, T> MaybeArcMutex<T> {
    pub fn lock(&'lock self) -> MaybeArcMutexGuard<'lock, T> {
      MaybeArcMutexGuard(self.0.borrow_mut())
    }
  }
}
