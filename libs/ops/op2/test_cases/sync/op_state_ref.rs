// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

use deno_core::OpState;
use deno_core::v8;

// Test w/ import pollution
#[allow(unused)]
use std::borrow::Borrow;
#[allow(unused)]
use std::borrow::BorrowMut;

#[op2(fast)]
fn op_state_ref(_state: &OpState) {}

#[op2(fast)]
fn op_state_mut(_state: &mut OpState) {}

#[op2(fast)]
fn op_state_and_v8_local(
  _state: &mut OpState,
  _callback: v8::Local<v8::Function>,
) {
}
