// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(FromV8)]
pub struct Tuple(#[from_v8(serde)] u8, u8);
