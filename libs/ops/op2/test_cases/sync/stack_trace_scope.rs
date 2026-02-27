// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(fast, stack_trace)]
fn op_stack_trace(#[string] _: String) {}
