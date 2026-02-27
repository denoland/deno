// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2]
#[string]
pub fn op_string_return(#[string] s: Option<String>) -> Option<String> {
  s
}
