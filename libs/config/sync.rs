// Copyright 2018-2024 the Deno authors. MIT license.

pub use inner::*;

#[cfg(feature = "sync")]
mod inner {
  #![allow(clippy::disallowed_types)]
  pub use std::sync::Arc as MaybeArc;
}

#[cfg(not(feature = "sync"))]
mod inner {
  pub use std::rc::Rc as MaybeArc;
}

// ok for constructing
#[allow(clippy::disallowed_types)]
pub fn new_rc<T>(value: T) -> MaybeArc<T> {
  MaybeArc::new(value)
}
