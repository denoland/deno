// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#[op2(core)]
pub fn op_test_add_option(a: u32, b: Option<u32>) -> u32 {
  a + b.unwrap_or(100)
}
