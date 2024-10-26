// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub use inner::*;

#[cfg(feature = "sync")]
mod inner {
  #![allow(clippy::disallowed_types)]

  pub use std::sync::Arc as MaybeArc;

  pub use core::marker::Send as MaybeSend;
  pub use core::marker::Sync as MaybeSync;
}

#[cfg(not(feature = "sync"))]
mod inner {
  pub use std::rc::Rc as MaybeArc;

  pub trait MaybeSync {}
  impl<T> MaybeSync for T where T: ?Sized {}
  pub trait MaybeSend {}
  impl<T> MaybeSend for T where T: ?Sized {}
}
