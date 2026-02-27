// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::OpState;

#[op2]
pub async fn op_async_opstate(
  state: Rc<RefCell<OpState>>,
) -> std::io::Result<i32> {
  Ok(*state.borrow().borrow::<i32>())
}
