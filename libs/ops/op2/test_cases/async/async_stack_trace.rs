// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[op2(stack_trace)]
pub async fn op_async_stack_trace() {}
