// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(ToV8)]
pub struct Tuple(#[to_v8(serde)] u8, u8);
