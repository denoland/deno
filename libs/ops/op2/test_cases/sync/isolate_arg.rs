// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::v8;

// Regression test for https://github.com/denoland/deno/issues/32953
// `&v8::Isolate` / `&mut v8::Isolate` arguments on `#[op2(fast)]` ops must
// expand to the public `isolate_unchecked` / `isolate_unchecked_mut`
// accessors on `FastApiCallbackOptions`. The macro previously read the
// `pub(crate)` `.isolate` field directly, so any crate outside `v8`
// itself failed to compile with E0616.

#[op2(fast)]
pub fn op_with_isolate_ref(_iso: &v8::Isolate) {}

#[op2(fast)]
pub fn op_with_isolate_mut(_iso: &mut v8::Isolate) {}
