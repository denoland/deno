// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(fast)]
#[bigint]
pub fn op_bigint() -> u64 {
  0
}
