// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;

// Collect a few examples that we'll smoke test when not running on the CI.

#[op2(fast)]
pub fn op_fast(x: u32) -> u32 {
  x
}

#[op2(fast)]
fn op_buffers(#[buffer] _a: &[u8], #[buffer(copy)] _b: Vec<u8>) {}

struct Something {}

#[op2(fast)]
fn op_v8_1(_s: v8::Local<v8::String>) {}

#[op2(fast)]
fn op_v8_2(_s: &v8::String) {}

pub type Int16 = i16;
pub type Int32 = i32;
pub type Uint16 = u16;
pub type Uint32 = u32;

#[op2(fast)]
#[smi]
fn op_smi_unsigned_return(
  #[smi] a: Int16,
  #[smi] b: Int32,
  #[smi] c: Uint16,
  #[smi] d: Uint32,
) -> Uint32 {
  a as Uint32 + b as Uint32 + c as Uint32 + d as Uint32
}

#[op2(fast)]
#[smi]
fn op_smi_signed_return(
  #[smi] a: Int16,
  #[smi] b: Int32,
  #[smi] c: Uint16,
  #[smi] d: Uint32,
) -> Int32 {
  a as Int32 + b as Int32 + c as Int32 + d as Int32
}

#[op2(fast, stack_trace)]
fn op_stack_trace(#[string] _: String) {}
