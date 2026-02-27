// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(ToV8)]
pub struct Struct {
  a: u8,
  #[to_v8(serde)]
  c: Vec<u32>,
  #[to_v8(rename = "e")]
  d: u16,
  f: Option<u32>,
}
