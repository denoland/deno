// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::future::Future;

#[op2]
pub fn op_async_impl_use(
  x: i32,
) -> impl Future<Output = std::io::Result<i32>> + use<> {
  async move { Ok(x) }
}
