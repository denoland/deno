// Copyright 2018-2026 the Deno authors. MIT license.

#[cfg(all(feature = "v8", feature = "quickjs"))]
compile_error!("features `v8` and `quickjs` are mutually exclusive");

#[cfg(not(any(feature = "v8", feature = "quickjs")))]
compile_error!("either feature `v8` or `quickjs` must be enabled");

#[cfg(all(feature = "v8", not(feature = "quickjs")))]
pub use rusty_v8::*;
#[cfg(all(feature = "quickjs", not(feature = "v8")))]
pub use v8x_backend::*;
