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
  pub use std::rc::Rc as MaybeArc;

  pub use std::collections::HashMap as MaybeDashMap;
}
