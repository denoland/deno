// Copyright 2018-2025 the Deno authors. MIT license.

pub use inner::*;

#[cfg(feature = "sync")]
mod inner {
  #![allow(clippy::disallowed_types)]

  pub use core::marker::Send as MaybeSend;
  pub use core::marker::Sync as MaybeSync;
  pub use std::sync::Arc as MaybeArc;
}

#[cfg(not(feature = "sync"))]
mod inner {
  pub trait MaybeSync {}
  impl<T> MaybeSync for T where T: ?Sized {}
  pub trait MaybeSend {}
  impl<T> MaybeSend for T where T: ?Sized {}
  pub use std::rc::Rc as MaybeArc;
}
