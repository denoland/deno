// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::marker::PhantomData;

pub struct CheckedSet<T: std::hash::Hash + ?Sized> {
  _kind: PhantomData<T>,
  checked: std::collections::HashSet<u64>,
}

impl<T: std::hash::Hash + ?Sized> Default for CheckedSet<T> {
  fn default() -> Self {
    Self {
      _kind: Default::default(),
      checked: Default::default(),
    }
  }
}

impl<T: std::hash::Hash + ?Sized> CheckedSet<T> {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      _kind: PhantomData,
      checked: std::collections::HashSet::with_capacity(capacity),
    }
  }

  pub fn insert(&mut self, value: &T) -> bool {
    self.checked.insert(self.get_hash(value))
  }

  fn get_hash(&self, value: &T) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
  }
}
