// Copyright 2018-2025 the Deno authors. MIT license.

pub use inner::*;

#[cfg(feature = "sync_fs")]
mod inner {
  #![allow(clippy::disallowed_types)]

  pub use core::marker::Send as MaybeSend;
  pub use core::marker::Sync as MaybeSync;
  pub use std::sync::Arc as MaybeArc;
}

#[cfg(not(feature = "sync_fs"))]
mod inner {
  pub use std::rc::Rc as MaybeArc;

  pub trait MaybeSync {}
  impl<T> MaybeSync for T where T: ?Sized {}
  pub trait MaybeSend {}
  impl<T> MaybeSend for T where T: ?Sized {}
}

#[allow(clippy::disallowed_types)]
#[inline]
pub fn new_rc<T>(value: T) -> MaybeArc<T> {
  MaybeArc::new(value)
}
