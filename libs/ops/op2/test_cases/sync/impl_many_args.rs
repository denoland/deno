// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::cppgc::GarbageCollected;
use deno_core::v8;

pub struct ManyArgs;

unsafe impl GarbageCollected for ManyArgs {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ManyArgs"
  }
}

#[op2]
impl ManyArgs {
  #[fast]
  pub fn add8(
    &self,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
  ) -> u32 {
    a + b + c + d + e + f + g + h
  }
}
