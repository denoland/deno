// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(FromV8)]
pub struct Struct {
  a: u8,
  #[from_v8(default = Some(3))]
  c: Option<u32>,
  #[from_v8(rename = "e")]
  d: u16,
  f: Option<u32>,
}
