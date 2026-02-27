// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;

// Unused scope would normally make this a slow-only op
#[op2(fast(op_fast))]
fn op_slow(_scope: &v8::PinScope<'_, '_>, a: u32, b: u32) -> u32 {
  a + b
}

#[op2(fast)]
fn op_fast(a: u32, b: u32) -> u32 {
  a + b
}

pub trait Trait {}

// Unused scope would normally make this a slow-only op
#[op2(fast(op_fast_generic::<T>))]
fn op_slow_generic<T: Trait>(
  _scope: &v8::PinScope<'_, '_>,
  a: u32,
  b: u32,
) -> u32 {
  a + b
}

#[op2(fast)]
fn op_fast_generic<T: Trait>(a: u32, b: u32) -> u32 {
  a + b
}
