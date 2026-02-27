// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();
use std::borrow::Cow;

#[op2]
#[string]
pub fn op_string_return() -> String {
  "".into()
}

#[op2]
#[string]
pub fn op_string_return_ref() -> &'static str {
  ""
}

#[op2]
#[string]
pub fn op_string_return_cow<'a>() -> Cow<'a, str> {
  "".into()
}
