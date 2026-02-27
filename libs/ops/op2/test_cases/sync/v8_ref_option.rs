// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;

#[op2(fast)]
pub fn op_v8_lifetime<'s>(_s: Option<&v8::String>, _s2: Option<&v8::String>) {}
