// Copyright 2018-2026 the Deno authors. MIT license.

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

#[derive(ToV8)]
pub struct LifetimeStruct<'a> {
  a: deno_core::v8::Local<'a, deno_core::v8::Value>,
  b: u8,
}

#[derive(ToV8)]
pub struct SkipIfStruct {
  a: u8,
  #[to_v8(skip_if = Option::is_none)]
  b: Option<u32>,
  #[to_v8(rename = "cc", skip_if = Option::is_none)]
  c: Option<String>,
  #[to_v8(skip_if = Vec::is_empty)]
  e: Vec<u32>,
  d: bool,
}
