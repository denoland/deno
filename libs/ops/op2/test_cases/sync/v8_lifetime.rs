// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;

#[op2]
pub fn op_v8_lifetime<'s>(
  _s: v8::Local<'s, v8::String>,
) -> v8::Local<'s, v8::String> {
  unimplemented!()
}
