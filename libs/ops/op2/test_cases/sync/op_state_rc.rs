// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::OpState;

#[op2(fast)]
fn op_state_rc(_state: Rc<RefCell<OpState>>) {}
