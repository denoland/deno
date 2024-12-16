// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use inner::*;

#[cfg(feature = "sync")]
mod inner {
  #![allow(clippy::disallowed_types)]

  pub use std::sync::Arc as MaybeArc;

  pub use dashmap::DashMap as MaybeDashMap;
}

#[cfg(not(feature = "sync"))]
mod inner {
  use std::hash::RandomState;
  pub use std::rc::Rc as MaybeArc;

  // Wrapper struct that exposes a subset of `DashMap` API.
  #[derive(Default)]
  struct MaybeDashMap<K, V, S = RandomState>(RefCell<HashMap<K, V, S>>);

  impl MaybeDashMap<K, V, S> {
    pub fn get(&'a self, key: &K) -> Option<&'a V> {
      let inner = self.0.borrow();
      inner.get(key)
    }

    pub fn insert(&self, key: K, value: V) -> Option<V> {
      let inner = self.0.borrow_mut();
      inner.insert(key, value)
    }
  }
}
