// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

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

#[op2]
#[smi]
fn op_smi_option(#[smi] a: Option<Uint32>) -> Option<Uint32> {
  a
}
