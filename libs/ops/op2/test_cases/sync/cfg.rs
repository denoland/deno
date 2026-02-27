// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

/// This is a doc comment.
#[op2(fast)]
#[cfg(windows)]
pub fn op_maybe_windows() -> () {}

/// This is a doc comment.
#[op2(fast)]
#[cfg(not(windows))]
pub fn op_maybe_windows() -> () {}
