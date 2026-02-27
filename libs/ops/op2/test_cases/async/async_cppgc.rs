// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();
use deno_core::GarbageCollected;
use deno_core::v8;

struct Wrap;

unsafe impl GarbageCollected for Wrap {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Wrap"
  }
}

#[op2]
#[cppgc]
async fn op_make_cppgc_object() -> Wrap {
  Wrap
}

#[op2]
async fn op_use_cppgc_object(#[cppgc] _wrap: &Wrap) {}

#[op2]
async fn op_use_optional_cppgc_object(#[cppgc] _wrap: Option<&Wrap>) {}
