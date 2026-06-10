// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct Input {}
#[derive(Serialize, Deserialize)]
pub struct Output {}

#[op2]
#[serde]
pub fn op_serde_v8(#[serde] _input: Input) -> Output {
  Output {}
}
