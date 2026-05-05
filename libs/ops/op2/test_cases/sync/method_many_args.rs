// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

// Regression test for https://github.com/denoland/deno/issues/32718
// `#[op2]` impl methods with more than the clippy default of 7 arguments
// must compile cleanly under `#![deny(clippy::too_many_arguments)]`. The
// generator already silenced the lint on the `impl Callable` block but
// not on the `trait Callable` declaration, so any external crate that
// denied the lint hit the warning at the trait method signature.

use deno_core::cppgc::GarbageCollected;
use deno_core::v8;

pub struct Big;

unsafe impl GarbageCollected for Big {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Big"
  }
}

#[op2]
impl Big {
  #[fast]
  pub fn many(
    &self,
    _a: f64,
    _b: f64,
    _c: f64,
    _d: f64,
    _e: f64,
    _f: f64,
    _g: f64,
    _h: f64,
  ) {
  }
}
