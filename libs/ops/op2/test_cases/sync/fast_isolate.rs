// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::cell::Cell;

use deno_core::cppgc::GarbageCollected;
use deno_core::v8;

#[op2(fast)]
fn op_isolate_ref(_isolate: &v8::Isolate) {}

#[op2(fast)]
fn op_isolate_mut(_isolate: &mut v8::Isolate) {}

pub struct Foo {
  value: Cell<u32>,
}

unsafe impl GarbageCollected for Foo {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Foo"
  }
}

#[op2]
impl Foo {
  #[constructor]
  #[cppgc]
  fn new(value: u32) -> Foo {
    Foo {
      value: Cell::new(value),
    }
  }

  #[fast]
  fn double_value(&self, _isolate: &v8::Isolate) -> u32 {
    self.value.get() * 2
  }
}
