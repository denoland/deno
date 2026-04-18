// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

#[derive(WebIDL)]
#[webidl(enum)]
pub enum Enumeration {
  FooBar,
  Baz,
  #[webidl(rename = "hello")]
  World,
}
