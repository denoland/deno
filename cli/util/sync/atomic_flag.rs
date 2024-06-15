// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// Simplifies the use of an atomic boolean as a flag.
#[derive(Debug, Default)]
pub struct AtomicFlag(AtomicBool);

impl AtomicFlag {
  /// Raises the flag returning if the raise was successful.
  pub fn raise(&self) -> bool {
    !self.0.swap(true, Ordering::SeqCst)
  }

  /// Gets if the flag is raised.
  pub fn is_raised(&self) -> bool {
    self.0.load(Ordering::SeqCst)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn atomic_flag_raises() {
    let flag = AtomicFlag::default();
    assert!(!flag.is_raised()); // false by default
    assert!(flag.raise());
    assert!(flag.is_raised());
    assert!(!flag.raise());
    assert!(flag.is_raised());
  }
}
